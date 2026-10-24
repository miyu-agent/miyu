//! fork 子代理，执行器这一头（施工 A-F 中，`docs/blueprint/agents.md`「fork 子代理」）：`subagent` 写 `fork` 的，经会话表从
//! 这次调用那一步分叉（假的会话表真去分叉），交代前面接上外壳；子会话的快照就是父会话那一份、回合开始不换，请求前缀照父会话的
//! 接着往下，向上回报；写了池、人格的，fork 出来的再 fork 的，拒。

use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event, ToolResult};
use miyu_kernel::id::{CallId, CommandId, SessionId};
use miyu_kernel::origin::{By, Session};
use miyu_kernel::session::{Command, Outcome};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Child, Fork, ForkChild, Handle, Lineage, Pending, SessionPort, Within, new_id};
use miyu_store::root::DataRoot;
use miyu_tool::Catalog;

use crate::support::*;

/// 记忆、深度上限：拆出去守 500 行上限。
#[path = "fork_subagent/limits.rs"]
mod limits;

const PLACEHOLDER: &str = include_str!("../../../resources/core/tool-results/forked.txt");
const SHELL: &str = include_str!("../../../resources/core/jobs/fork-directive.txt");
const NO_FORK: &str = include_str!("../../../resources/software/basesystem/agent/no-fork.txt");

/// 假的会话表：fork 真去分叉（照数据根 `root`），别的不造；记下要它 fork 的、发的命令。
struct Forks {
    root: DataRoot,
    /// 要它造的（不 fork 的）子会话：只记下，造得成。
    made: Mutex<Vec<Child>>,
    forked: Mutex<Vec<(ForkChild, SessionId)>>,
    sent: Mutex<Vec<(SessionId, CommandId, By, Command)>>,
}

impl Forks {
    fn new(home: &Home) -> Arc<Forks> {
        Arc::new(Forks {
            root: home.root.clone(),
            made: Mutex::new(Vec::new()),
            forked: Mutex::new(Vec::new()),
            sent: Mutex::new(Vec::new()),
        })
    }

    fn forked(&self) -> Vec<(ForkChild, SessionId)> {
        self.forked
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn sent(&self) -> Vec<(SessionId, CommandId, By, Command)> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl SessionPort for Forks {
    fn create(&self, child: Child) -> Pending<'_, Result<SessionId, String>> {
        self.made
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(child);
        Box::pin(async { Ok(new_id(now())) })
    }

    fn fork(&self, child: ForkChild) -> Pending<'_, Result<SessionId, String>> {
        Box::pin(async move {
            let id = new_id(now());
            miyu_session::fork(Fork {
                root: &self.root,
                owner: &child.owner,
                parent: &child.lineage.parent,
                id: id.clone(),
                cause: child.command.clone(),
                by: By::Session(Session {
                    id: child.lineage.parent.clone(),
                }),
                at: now(),
                index: None,
                seal_after: None,
                within: Some(Within {
                    call: child.call,
                    depth: child.lineage.depth,
                    placeholder: child.placeholder.clone(),
                }),
            })
            .await
            .map_err(|error| error.to_string())?;
            self.forked
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push((child, id.clone()));
            Ok(id)
        })
    }

    fn open(&self, _session: SessionId) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn command(
        &self,
        session: SessionId,
        id: CommandId,
        by: By,
        command: Command,
    ) -> Pending<'_, Result<Outcome, String>> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((session, id, by, command));
        Box::pin(async { Ok(Outcome::Accepted { events: Vec::new() }) })
    }

    fn stop(
        &self,
        _session: SessionId,
        _id: CommandId,
        _by: By,
    ) -> Pending<'_, Result<(), String>> {
        Box::pin(async { Ok(()) })
    }

    fn peek(&self, _session: SessionId) -> Pending<'_, Result<miyu_session::Peek, String>> {
        Box::pin(async { Ok(miyu_session::Peek::default()) })
    }

    fn sessions(
        &self,
        _owner: miyu_kernel::id::AccountId,
        _stop: miyu_tool::Stop,
    ) -> Pending<'_, Result<Vec<miyu_tool::MainSession>, String>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn read_log(&self, _session: SessionId) -> Pending<'_, Result<miyu_tool::Log, String>> {
        Box::pin(async { Err("no logs here".to_string()) })
    }

    fn held(&self, _session: SessionId) -> Pending<'_, bool> {
        Box::pin(async { false })
    }

    fn watch(
        &self,
        _session: SessionId,
        _watcher: SessionId,
        _since: miyu_kernel::time::Timestamp,
    ) -> Pending<'_, Result<(), miyu_session::NotWatched>> {
        Box::pin(async {
            Err(miyu_session::NotWatched::Failed(
                "no watches here".to_string(),
            ))
        })
    }
}

/// 真的基础系统：`subagent` 在里面。
fn basesystem(home: &Home) -> Catalog {
    Catalog::new(miyu_basesystem::tools(home.resources.path()).expect("读得出")).expect("合写法")
}

/// 调一次 `subagent`，参数照 `args` 再加标题、交代。
fn subagent(args: serde_json::Value) -> Play {
    let mut all =
        serde_json::json!({"description": "查导出", "prompt": "List what src/lib.rs exports."});
    for (key, value) in args.as_object().expect("是对象") {
        all[key] = value.clone();
    }
    Play::calls(&[("subagent", all.to_string().as_str())])
}

/// 说一句，等到磁盘上第 `turns` 轮结束。
async fn turn(home: &Home, handle: &Handle, turns: usize) -> Vec<Event> {
    ask(handle, &format!("cmd-{turns}"), say("去查一下"))
        .await
        .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= turns
    })
    .await
}

fn results(log: &[Event]) -> Vec<&ToolResult> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

fn text(result: &ToolResult) -> &str {
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => text,
        other => panic!("一段字：{other:?}"),
    }
}

/// 父会话那一轮里调 `subagent` 的那一次。
fn subagent_call(log: &[Event]) -> CallId {
    results(log).first().expect("有结果").call_id
}

/// 造会话时的快照哈希，和之后换过的几份。
fn snapshots(log: &[Event]) -> Vec<String> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::SessionCreated(created) => Some(created.policy.as_str().to_string()),
            Body::PolicyChanged(changed) => changed
                .policy
                .as_ref()
                .map(|hash| hash.as_str().to_string()),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_fork_goes_on_from_the_step_that_made_it_with_the_parents_snapshot() {
    let home = Home::new();
    let forks = Forks::new(&home);
    let script = Script::new([
        subagent(serde_json::json!({"fork": true})),
        Play::Says("派出去了。"),
        Play::Says("导出了 run 和 Config。"),
    ]);
    let tools = basesystem(&home);
    let lines = Lines {
        sessions: Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let parent = home
        .create_full(&script, &tools, Opening::default(), lines)
        .await;
    let log = turn(&home, &parent, 1).await;
    let call = subagent_call(&log);
    assert_eq!(text(results(&log)[0]), "Started subagent j1: \"查导出\".\n");
    let forked = forks.forked();
    let (asked, child) = forked.first().expect("要会话表 fork 了一个");
    assert_eq!(asked.call, call, "从派它的那次调用分叉");
    assert_eq!(asked.lineage.parent, *parent.id());
    assert_eq!(asked.lineage.depth, 1);
    assert_eq!(
        asked.command.as_str(),
        format!("{}/j1", parent.id().as_str())
    );
    assert_eq!(asked.placeholder, PLACEHOLDER);
    // 交代前面接上外壳，作为父会话发来的话送进子会话。
    let sent = forks.sent();
    let (to, id, by, command) = sent.first().expect("交代送进去了");
    assert_eq!(to, child);
    assert_eq!(id.as_str(), format!("{}/j1/prompt", parent.id().as_str()));
    let directive = format!("{}\n\nList what src/lib.rs exports.", SHELL.trim_end());
    match command {
        Command::Send { blocks, .. } => assert_eq!(
            blocks,
            &vec![Block::Text(Text {
                text: directive.clone()
            })]
        ),
        other => panic!("发一条消息：{other:?}"),
    }
    // 子会话载入，收到交代说一轮：快照就是父会话那一份，回合开始不换；请求前缀照父会话的接着往下。
    let branch = home
        .load_full(
            child,
            &script,
            &tools,
            &environment().cwd,
            Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        )
        .await;
    within(
        "交代的回应",
        branch.command(id.clone(), by.clone(), command.clone()),
    )
    .await
    .expect("子会话在跑");
    let child_log = until_logged(&home, child, |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= 2
    })
    .await;
    assert_eq!(
        snapshots(&child_log),
        snapshots(&log)[..1],
        "子会话一直用父会话那一份快照"
    );
    let requests = script.requests();
    let (parent_second, child_first) = (&requests[1].1, &requests[2].1);
    assert_eq!(child_first.system, parent_second.system);
    assert_eq!(child_first.tools, parent_second.tools);
    assert_eq!(
        child_first.messages[..2],
        parent_second.messages[..2],
        "人那一句、派它的那条回复一样"
    );
    let rest = format!("{:?}", &child_first.messages[2..]);
    assert!(
        rest.contains("Forked; the parent handles this call."),
        "{rest}"
    );
    assert!(
        rest.contains("You are a fork of the agent above."),
        "{rest}"
    );
    // 子代理照常向上回报。
    let reported = until_sent(&forks, parent.id()).await;
    assert!(matches!(reported, Command::Report(_)), "{reported:?}");
    stop(&branch).await;
    stop(&parent).await;
}

/// 等到会话表收到发给 `to` 的一个命令。
async fn until_sent(forks: &Forks, to: &SessionId) -> Command {
    within("向上回报", async {
        loop {
            if let Some((_, _, _, command)) = forks.sent().into_iter().find(|(at, ..)| at == to) {
                return command;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
}

#[tokio::test]
async fn a_fork_takes_no_pool_or_persona() {
    let home = Home::new();
    let forks = Forks::new(&home);
    let script = Script::new([
        subagent(serde_json::json!({"fork": true, "persona": "engineer"})),
        Play::Says("好。"),
    ]);
    let lines = Lines {
        sessions: Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let parent = home
        .create_full(&script, &basesystem(&home), Opening::default(), lines)
        .await;
    let log = turn(&home, &parent, 1).await;
    let said = text(results(&log)[0]);
    assert!(
        said.contains("`fork` keeps your model and persona"),
        "{said}"
    );
    assert!(forks.forked().is_empty(), "不 fork");
}

#[tokio::test]
async fn a_fork_cannot_fork_again() {
    let home = Home::new();
    let forks = Forks::new(&home);
    let script = Script::new([
        subagent(serde_json::json!({"fork": true})),
        Play::Says("派出去了。"),
        subagent(serde_json::json!({"fork": true})),
        Play::Says("自己做。"),
    ]);
    let tools = basesystem(&home);
    let lines = Lines {
        sessions: Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        ..Lines::default()
    };
    let parent = home
        .create_full(&script, &tools, Opening::default(), lines)
        .await;
    turn(&home, &parent, 1).await;
    let (_, child) = forks.forked().remove(0);
    let (_, id, by, command) = forks.sent().remove(0);
    let branch = home
        .load_full(
            &child,
            &script,
            &tools,
            &environment().cwd,
            Some(Arc::clone(&forks) as Arc<dyn SessionPort>),
        )
        .await;
    within("交代的回应", branch.command(id, by, command))
        .await
        .expect("子会话在跑");
    let child_log = until_logged(&home, &child, |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= 2
    })
    .await;
    assert_eq!(text(results(&child_log).last().expect("有结果")), NO_FORK);
    assert_eq!(forks.forked().len(), 1, "没再 fork");
    stop(&branch).await;
    stop(&parent).await;
}
