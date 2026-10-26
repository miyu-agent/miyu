//! 合并跑在整理会话里（施工 D-2，设计 `41-整理会话.md`）：真记忆日志、假的会话表（记下开的、每一批的字，照剧本交回）。
//!
//! 接了会话表的：真有要合的才开、一趟一个会话，一批一轮（`<会话>/dream/<n>`），请求的字遮过 key；答了的照旧读、核对、落，
//! `merging` 带会话，回执带会话；会话出错结束的数一次失败、连着三次跳过；会话自己停了、人叫停了（打断那一轮）的放弃这一批、
//! 不数；会话那一路断了的这一趟停、不数。没接的照旧走一次性入口（别的测试）。

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use miyu_http::testkit::Server;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_kernel::time::UtcOffset;
use miyu_recall::merge::{Revised, Seen};
use miyu_recall::{MemoryEvent, MergeFailed, STOPPED, from_event};
use miyu_session::testkit::{Play, Script};
use miyu_session::{
    DreamBatch, DreamRoom, DreamSessions, DreamSpec, Dreamed, Keeper, Listener, NotDreamed,
    Pending, Turn, Watched,
};
use tokio::sync::Notify;

use crate::support::extracting::*;
use crate::support::meaning::{catalog, chat, persona, save};
use crate::support::*;

/// 会话表那一头照剧本怎么交回一批。
#[derive(Debug, Clone)]
enum Step {
    Answer(String),
    Fail(&'static str),
    /// 会话自己停了（例如有人直接打断了整理会话）。
    Stopped,
    /// 会话那一路断了：核心在停。
    Gone,
    /// 一直等到叫停。
    Hold,
    /// 边流边交（`Watched`），交完了答这一整段：`Gate` 那里等测试放行。
    Stream(Vec<Piece>),
}

/// 流里的一样。
#[derive(Debug, Clone)]
enum Piece {
    Text(&'static str),
    Restart,
    /// 等测试放行（`Fake::gate`）。
    Gate,
}

/// 假的会话表：记下开的、每一批、每一次叫停。
#[derive(Debug, Default)]
struct Fake {
    opened: Mutex<Vec<DreamSpec>>,
    batches: Mutex<Vec<(SessionId, CommandId, String)>>,
    stops: Mutex<Vec<CommandId>>,
    steps: Mutex<VecDeque<Step>>,
    stopped: Notify,
    holding: Notify,
    gate: Notify,
    closed: Mutex<Vec<(SessionId, usize)>>,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn session_id() -> SessionId {
    SessionId::parse("0192f3a0-3333-7abc-8def-001122334455").expect("合写法")
}

impl DreamSessions for Fake {
    fn open(&self, spec: DreamSpec) -> Pending<'_, Result<SessionId, String>> {
        lock(&self.opened).push(spec);
        Box::pin(async { Ok(session_id()) })
    }

    fn batch<'a>(
        &'a self,
        session: SessionId,
        id: CommandId,
        text: String,
        watch: &'a mut (dyn FnMut(Watched<'_>) + Send),
    ) -> Pending<'a, Result<DreamBatch, String>> {
        lock(&self.batches).push((session, id, text));
        let step = lock(&self.steps).pop_front().expect("剧本里还有");
        Box::pin(async move {
            match step {
                Step::Stream(pieces) => {
                    let mut answer = String::new();
                    for piece in pieces {
                        match piece {
                            Piece::Text(text) => {
                                answer.push_str(text);
                                watch(Watched::Text(text));
                            }
                            Piece::Restart => {
                                answer.clear();
                                watch(Watched::Restart);
                            }
                            Piece::Gate => self.gate.notified().await,
                        }
                    }
                    Ok(DreamBatch::Answered(answer))
                }
                Step::Answer(text) => Ok(DreamBatch::Answered(text)),
                Step::Fail(why) => Ok(DreamBatch::Failed(why.to_string())),
                Step::Stopped => Ok(DreamBatch::Stopped),
                Step::Gone => Err("core stopping".to_string()),
                Step::Hold => {
                    self.holding.notify_one();
                    self.stopped.notified().await;
                    Ok(DreamBatch::Stopped)
                }
            }
        })
    }

    fn close(&self, session: SessionId, keep: usize) -> Pending<'_, Result<usize, String>> {
        lock(&self.closed).push((session, keep));
        Box::pin(async { Ok(0) })
    }

    fn stop(&self, _session: SessionId, id: CommandId) -> Pending<'_, Result<(), String>> {
        lock(&self.stops).push(id);
        self.stopped.notify_one();
        Box::pin(async { Ok(()) })
    }
}

/// 场地接上抽取（合并的一次性入口指着一台什么都不交的服务器：接了会话表的不该去问它），装上照 `steps` 交回的会话表。
async fn world(steps: Vec<Step>) -> (Home, Arc<Fake>, Server) {
    let server = Server::start(Vec::new()).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 5\n");
    let fake = Arc::new(Fake {
        steps: Mutex::new(steps.into()),
        ..Fake::default()
    });
    assert!(home.memory.give_dream_sessions(fake.clone()), "头一次接上");
    (home, fake, server)
}

fn keeper(home: &Home) -> Keeper {
    Keeper::new(&home.memory, persona(), Listener::Owner(alice()))
}

/// 不经会话的那一趟：照场地这时的配置。
async fn dream(home: &Home) -> Result<Dreamed, NotDreamed> {
    let source = Arc::clone(&*home.configs.borrow());
    let config = Arc::new(Turn::new(source.with_project("~"), source));
    keeper(home)
        .dream(
            config,
            alice_account(),
            UtcOffset::from_minutes(540).expect("合写法"),
        )
        .await
}

/// 这一间记过的整理事件。
fn merges(home: &Home) -> Vec<MemoryEvent> {
    keeper(home)
        .follow(Some(0), 4096)
        .expect("跟得上")
        .filled
        .iter()
        .filter_map(|event| from_event(event)?.ok())
        .filter(|event| {
            matches!(
                event,
                MemoryEvent::Merging(_) | MemoryEvent::Merged(_) | MemoryEvent::MergeFailed(_)
            )
        })
        .collect()
}

fn revise_m1() -> Step {
    Step::Answer(
        serde_json::json!({
            "revised": [{"id": "m1", "text": "用户养了一只橘猫"}],
            "summary": "用户养了一只橘猫。"
        })
        .to_string(),
    )
}

fn failures(home: &Home) -> Vec<MergeFailed> {
    merges(home)
        .into_iter()
        .filter_map(|event| match event {
            MemoryEvent::MergeFailed(failed) => Some(failed),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_dream_runs_its_batches_in_one_dream_session() {
    let (home, fake, server) = world(vec![revise_m1()]).await;
    save(&home, "用户养了一只猫，key 是 sk-org-0123456789abcdef");
    let dreamed = dream(&home).await.expect("整理了");
    assert_eq!(
        dreamed,
        Dreamed {
            given: 1,
            revised: 1,
            summary: true,
            session: Some(session_id()),
            ..Dreamed::default()
        }
    );
    assert!(server.received().is_empty(), "没走一次性入口");
    let opened = lock(&fake.opened).clone();
    assert_eq!(opened.len(), 1, "一趟一个会话");
    assert_eq!(opened[0].owner, alice_account(), "那一间的主人");
    assert_eq!(opened[0].room, DreamRoom::Persona("engineer".to_string()));
    assert!(opened[0].manual, "人叫的");
    assert_eq!(opened[0].from, None, "不经会话的没有");
    assert_eq!(
        opened[0].model.as_deref(),
        Some("@normal"),
        "整理记忆登记的档位池（施工 Q-2）"
    );
    let batches = lock(&fake.batches).clone();
    assert_eq!(batches.len(), 1);
    let (session, id, text) = &batches[0];
    assert_eq!(session, &session_id());
    assert_eq!(id.as_str(), format!("{}/dream/1", session_id()));
    assert!(text.starts_with("Below are your memories"), "{text}");
    assert!(
        !text.contains("sk-org-0123456789abcdef"),
        "遮过 key：{text}"
    );
    let merging = merges(&home)
        .into_iter()
        .find_map(|event| match event {
            MemoryEvent::Merging(merging) => Some(merging),
            _ => None,
        })
        .expect("记了开始合");
    assert_eq!(merging.session, Some(session_id()), "merging 带会话");
    assert!(
        merges(&home)
            .iter()
            .any(|event| matches!(event, MemoryEvent::Merged(merged) if merged.revised == 1)),
        "照旧落"
    );
    assert_eq!(
        lock(&fake.closed).clone(),
        vec![(session_id(), 20)],
        "跑完收这一趟，照 memory.dream_keep 留（出厂 20）"
    );
}

#[tokio::test]
async fn nothing_to_dream_opens_no_session() {
    let (home, fake, _server) = world(Vec::new()).await;
    assert_eq!(dream(&home).await.expect("交回了"), Dreamed::default());
    assert!(lock(&fake.opened).is_empty(), "没东西可整理的不开空会话");
    assert!(lock(&fake.closed).is_empty(), "没开的不收");
}

#[tokio::test]
async fn a_failed_turn_counts_and_the_third_skips_the_batch() {
    let (home, _fake, _server) = world(vec![
        Step::Fail("error: model_failed"),
        Step::Fail("error: model_failed"),
        Step::Fail("error: model_failed"),
    ])
    .await;
    save(&home, "用户养了一只猫");
    for _ in 0..3 {
        assert!(matches!(dream(&home).await, Err(NotDreamed::Failed(_))));
    }
    let tries: Vec<_> = failures(&home)
        .iter()
        .map(|failed| (failed.why.clone(), failed.tries))
        .collect();
    assert_eq!(
        tries,
        vec![
            ("error: model_failed".to_string(), Some(1)),
            ("error: model_failed".to_string(), Some(2)),
            ("error: model_failed".to_string(), Some(3)),
        ]
    );
    assert!(
        merges(&home)
            .iter()
            .any(|event| matches!(event, MemoryEvent::Merged(merged) if merged.failed)),
        "连着第三次跳过这一批"
    );
}

#[tokio::test]
async fn a_lost_session_stops_the_trip_without_counting() {
    let (home, fake, _server) = world(vec![Step::Gone, revise_m1()]).await;
    save(&home, "用户养了一只猫");
    assert!(matches!(dream(&home).await, Err(NotDreamed::Failed(_))));
    assert!(failures(&home).is_empty(), "不记没合成、不数");
    let dreamed = dream(&home).await.expect("下次接着合");
    assert_eq!(dreamed.revised, 1);
    assert_eq!(lock(&fake.opened).len(), 2, "一趟开一个");
}

#[tokio::test]
async fn a_session_stopped_on_its_own_gives_up_the_batch() {
    let (home, _fake, _server) = world(vec![Step::Stopped, revise_m1()]).await;
    save(&home, "用户养了一只猫");
    let dreamed = dream(&home).await.expect("交回了");
    assert!(dreamed.stopped, "{dreamed:?}");
    assert_eq!(
        failures(&home)
            .iter()
            .map(|failed| (failed.why.as_str(), failed.tries))
            .collect::<Vec<_>>(),
        vec![(STOPPED, None)],
        "记 stopped、不数"
    );
    assert_eq!(dream(&home).await.expect("下次接着合").revised, 1);
}

#[tokio::test]
async fn stopping_the_dream_interrupts_its_turn() {
    let (home, fake, _server) = world(vec![Step::Hold]).await;
    save(&home, "用户养了一只猫");
    let stopping = async {
        within("等到那一批", fake.holding.notified()).await;
        assert!(keeper(&home).stop_dream(), "这一间有人叫的整理");
    };
    let (dreamed, ()) = within("停下", async { tokio::join!(dream(&home), stopping) }).await;
    let dreamed = dreamed.expect("交回了");
    assert!(dreamed.stopped);
    assert_eq!(dreamed.session, Some(session_id()));
    assert_eq!(
        lock(&fake.stops)
            .iter()
            .map(CommandId::as_str)
            .collect::<Vec<_>>(),
        vec![format!("{}/dream/1/stop", session_id()).as_str()],
        "打断那一轮，另用一个编号"
    );
    assert_eq!(
        failures(&home)
            .iter()
            .map(|failed| failed.why.as_str())
            .collect::<Vec<_>>(),
        vec![STOPPED]
    );
}

#[tokio::test]
async fn a_dream_asked_in_a_session_names_it() {
    let server = Server::start(vec![every_turn("用户养了一只猫")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "extract_turns = 5\n");
    let fake = Arc::new(Fake {
        steps: Mutex::new(vec![revise_m1()].into()),
        ..Fake::default()
    });
    assert!(home.memory.give_dream_sessions(fake.clone()));
    let handle = home
        .create_full(
            &Script::new([Play::Says("好。")]),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    let dreamed = handle.dream().await.expect("会话在跑").expect("整理了");
    assert_eq!(dreamed.session, Some(session_id()));
    let opened = lock(&fake.opened).clone();
    assert_eq!(opened.len(), 1);
    assert_eq!(
        opened[0].from.as_ref(),
        Some(handle.id()),
        "人在哪个会话里叫的"
    );
    assert!(opened[0].manual);
    assert_eq!(server.received().len(), 1, "只有抽取走一次性入口");
    tokio::time::sleep(Duration::from_millis(10)).await;
    stop(&handle).await;
}

/// 后台自己合的也开整理会话（设计 41 DS6）：不是人叫的，不记哪个会话叫的。
#[tokio::test]
async fn a_merge_on_its_own_runs_in_a_dream_session_too() {
    let server = Server::start(vec![every_turn("用户养了一只猫")]).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "merge_sessions = 1\n");
    let fake = Arc::new(Fake {
        steps: Mutex::new(vec![revise_m1()].into()),
        ..Fake::default()
    });
    assert!(home.memory.give_dream_sessions(fake.clone()));
    let handle = home
        .create_full(
            &Script::new([Play::Says("好。"), Play::Says("好。")]),
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    chat(&home, &handle, 1, "我养了一只猫").await;
    chat(&home, &handle, 2, "是只橘猫").await;
    let mut opened = Vec::new();
    for _ in 0..200 {
        opened = lock(&fake.opened).clone();
        if !opened.is_empty() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert_eq!(opened.len(), 1, "抽完够了就在后台合");
    assert!(!opened[0].manual, "不是人叫的");
    assert_eq!(opened[0].from, None, "不记哪个会话");
    stop(&handle).await;
}

/// 一批放不下的分几批合：同一趟只开一个会话，一批一轮，编号接着数。
#[tokio::test]
async fn batches_of_one_dream_share_its_session() {
    let answers = (0..4).map(|_| Step::Answer("{}".to_string())).collect();
    let (home, fake, _server) = world(answers).await;
    for n in 0..120 {
        save(
            &home,
            &format!("用户第 {n} 件事：{}", "很长的一段记忆".repeat(14)),
        );
    }
    dream(&home).await.expect("整理了");
    assert_eq!(lock(&fake.opened).len(), 1, "一趟一个会话");
    let ids: Vec<String> = lock(&fake.batches)
        .iter()
        .map(|(_, id, _)| id.as_str().to_string())
        .collect();
    assert!(ids.len() >= 2, "分了几批：{ids:?}");
    for (n, id) in ids.iter().enumerate() {
        assert_eq!(id, &format!("{}/dream/{}", session_id(), n + 1));
    }
}

/// 她正在写什么（施工 R-21）在整理会话里也边收边推（D-1 下的 `watch`）：写完一条就推，不等这一轮答完；换了一次请求从头再来的
/// 之前那几段作废。
#[tokio::test]
async fn what_she_writes_comes_out_while_the_turn_runs() {
    let answer = r#"{"revised": [{"id": "m1", "text": "用户养了一只橘猫"}], "summary": "用户养了一只橘猫。"}"#;
    let (head, tail) = answer.split_at(answer.find(", \"summary\"").expect("有摘要"));
    let (home, fake, _server) = world(vec![Step::Stream(vec![
        Piece::Text(r#"{"revised": [{"id": "m1", "text": "作废的"#),
        Piece::Restart,
        Piece::Text(head),
        Piece::Gate,
        Piece::Text(tail),
    ])])
    .await;
    save(&home, "用户养了一只猫");
    let mut feed = keeper(&home)
        .follow(Some(0), 1024)
        .expect("跟得上")
        .dreaming;
    let watching = async {
        let first = within("头一条", feed.next()).await.expect("还在");
        // 这一轮还停在放行那里：收得到就是边收边推的。
        fake.gate.notify_one();
        first
    };
    let (dreamed, first) = tokio::join!(dream(&home), watching);
    assert_eq!(dreamed.expect("整理了").revised, 1);
    assert_eq!(
        first.seen,
        Seen::Revised(Revised {
            id: miyu_recall::MemoryId::new(miyu_kernel::id::Seq::new(1).expect("从 1 起")),
            text: "用户养了一只橘猫".into()
        }),
        "重来以前那一段作废了"
    );
}

/// `memory.dream_keep` 照配置交给 `close`（设计 41 DS9）。
#[tokio::test]
async fn closing_keeps_as_many_as_configured() {
    let server = Server::start(Vec::new()).await;
    let mut home = Home::new();
    organizer(&mut home, &server, "dream_keep = 3\n");
    let fake = Arc::new(Fake {
        steps: Mutex::new(vec![revise_m1()].into()),
        ..Fake::default()
    });
    assert!(home.memory.give_dream_sessions(fake.clone()));
    save(&home, "用户养了一只猫");
    dream(&home).await.expect("整理了");
    assert_eq!(lock(&fake.closed).clone(), vec![(session_id(), 3)]);
}
