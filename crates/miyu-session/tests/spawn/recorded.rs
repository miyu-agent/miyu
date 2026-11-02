//! 载入交回的「要不要认派到一半的空子会话」（施工 V-37 上，`agents.md` 第一条第 8 条）：真会话派子代理，载入以后交回的
//! [`miyu_session::Placed::recorded`]，和原来收空子会话时在载入以后另把父会话的整份日志读一遍攒出来的（照抄留在这里的
//! [`old`]）一模一样。几种情形：没派过、派成了、派不成、一次派两个只派成一个、派完没来得及记下结果就崩了、改名以前的
//! `agent`。

use std::collections::BTreeSet;

use miyu_kernel::event::Event;
use miyu_session::Workplace;

use super::*;

/// 原来的认法（施工 7-8 起，V-37 上以前在 `miyu-endpoint` 的 `sessions/orphans.rs`），照抄攒三样的那一段：整份日志里派子代理
/// 的调用都在带着子会话的 `job.started` 的调用里的不用认，不然交回记下了的子会话。
fn old(events: &[Event]) -> Option<BTreeSet<SessionId>> {
    let mut calls = BTreeSet::new();
    let mut started_calls = BTreeSet::new();
    let mut started = BTreeSet::new();
    for event in events {
        match &event.body {
            Body::MessageAssistant(reply) => {
                calls.extend(reply.blocks.iter().filter_map(|block| match block {
                    Block::ToolCall(call) if miyu_tool::is_subagent(&call.name) => {
                        Some(call.call_id)
                    }
                    _ => None,
                }))
            }
            Body::ToolResult(result) => {
                for effect in &result.effects {
                    if let Effect::JobStarted(job) = effect
                        && let Some(session) = &job.session
                    {
                        started_calls.insert(result.call_id);
                        started.insert(session.clone());
                    }
                }
            }
            _ => {}
        }
    }
    if calls.is_subset(&started_calls) {
        return None;
    }
    Some(started)
}

/// 父会话照 `script` 说一轮，会话表是 `table`；日志照 `crash` 改过（崩了的样子）以后重新载入：交回载入交回的，和原来的认法
/// 照载入以后的整份日志认的。
async fn both(
    script: Script,
    table: &Arc<Table>,
    crash: impl FnOnce(&str) -> String,
) -> (Option<BTreeSet<SessionId>>, Option<BTreeSet<SessionId>>) {
    let home = Home::new();
    let handle = parent(&home, &script, table).await;
    one_turn(&home, &handle, 1).await;
    let id = handle.id().clone();
    stop(&handle).await;
    let dir = home.root.session_dir(&id);
    let segment = std::fs::read_dir(&dir)
        .expect("读得了")
        .map(|entry| entry.expect("读得了").path())
        .find(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .expect("有一段日志");
    let text = std::fs::read_to_string(&segment).expect("读得回来");
    std::fs::write(&segment, crash(&text)).expect("写得进");
    let port = Some(Arc::clone(table) as Arc<dyn SessionPort>);
    let (loaded, placed) = home
        .placing(
            &id,
            &Script::new([Play::Says("嗯。")]),
            &basesystem(&home),
            Workplace::Given(environment()),
            port,
        )
        .await;
    // 原来是载入以后在后台读：载入时补上的收尾（崩了的那一轮）也在里面。
    let whole = old(&home.log(&id));
    stop(&loaded).await;
    (placed.recorded, whole)
}

/// 日志原样，没崩。
fn intact(text: &str) -> String {
    text.to_string()
}

/// 截在第一条工具结果前面：派完了、没来得及记下结果就崩了。
fn before_the_result(text: &str) -> String {
    text.split_inclusive('\n')
        .take_while(|line| !line.contains(r#""kind":"tool.result""#))
        .collect()
}

/// 一种情形：说什么、会话表、日志怎么改（崩了的样子）、该交回什么。
struct Case {
    what: &'static str,
    script: Script,
    table: Arc<Table>,
    crash: fn(&str) -> String,
    expected: Option<BTreeSet<SessionId>>,
}

/// 改名以前造的会话：调用叫 `agent`。
fn renamed(text: &str) -> String {
    text.replace(r#""name":"subagent""#, r#""name":"agent""#)
}

#[tokio::test]
async fn what_loading_hands_back_is_what_reading_the_whole_log_found() {
    let ids = |ns: &[usize]| Some(ns.iter().map(|n| child_id(*n)).collect::<BTreeSet<_>>());
    let one = || Script::new([calls(&[subagent("查", "Look.")]), Play::Says("好。")]);
    let two = || {
        Script::new([
            calls(&[subagent("甲", "Look."), subagent("乙", "Look.")]),
            Play::Says("好。"),
        ])
    };
    let fine = || Arc::new(Table::default());
    let cases = [
        Case {
            what: "没派过",
            script: Script::new([Play::Says("好。")]),
            table: fine(),
            crash: intact,
            expected: None,
        },
        Case {
            what: "派成了",
            script: one(),
            table: fine(),
            crash: intact,
            expected: None,
        },
        Case {
            what: "派不成",
            script: one(),
            table: Table::failing(&[1]),
            crash: intact,
            expected: ids(&[]),
        },
        Case {
            what: "两个只派成一个",
            script: two(),
            table: Table::failing(&[2]),
            crash: intact,
            expected: ids(&[1]),
        },
        Case {
            what: "没记下结果就崩了",
            script: one(),
            table: fine(),
            crash: before_the_result,
            expected: ids(&[]),
        },
        Case {
            what: "改名以前的 agent，派不成",
            script: one(),
            table: Table::failing(&[1]),
            crash: renamed,
            expected: ids(&[]),
        },
        Case {
            what: "改名以前的 agent，派成了",
            script: one(),
            table: fine(),
            crash: renamed,
            expected: None,
        },
    ];
    for case in cases {
        let (placed, whole) = both(case.script, &case.table, case.crash).await;
        assert_eq!(placed, whole, "{}：载入交回的和整份读的一样", case.what);
        assert_eq!(placed, case.expected, "{}", case.what);
    }
}
