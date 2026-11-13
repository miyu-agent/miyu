//! 封存了的支线判完抽取就报会话表来问（施工 V-51，`docs/blueprint/session/actor.md`「退下」）：没事了（照问退不退的同一本账）
//! 当场报一声，不等会话表隔一阵问；有头订阅着、抽取在路上的不报，放下了、抽完了再报；报过以后放下一个把手也再报（会话表上次
//! 问时它还拿着）；没封存的从不报。抽取失败的照原来的规矩：不当场重试，抽取的账放下了就报。假的会话表记下每一次报
//! （`support/table.rs`）。

use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_kernel::event::Body;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Fork, Handle, Hold, Retire, SessionPort, Stopped, new_id};
use tokio::sync::Barrier;

use crate::support::extracting::{mark, organizer_after, stream};
use crate::support::meaning::catalog;
use crate::support::table::{Table, session};
use crate::support::*;

/// 问退不退时说的「闲多久」、抽取的闹钟：长得这次测试里不会到。
const LONG: Duration = Duration::from_secs(3600);

/// 照 `plays` 回，起标题的请求回一个标题（没排的一直在路上，会话一直有事）。
fn script(plays: impl IntoIterator<Item = Play>) -> Script {
    Script::new(plays).titles([Play::Says("测试")])
}

/// 说一句，等这一轮结束，放下订阅。
async fn talk(handle: &Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 封存了没有。
fn sealed(home: &Home, session: &SessionId) -> bool {
    home.log(session)
        .iter()
        .any(|event| matches!(event.body, Body::SessionSealed(_)))
}

/// 开着记忆的主会话说过一轮，从它分出一条跑满 `seal_after` 轮就封存的支线，载入（会话表是 `table`），交回主会话和支线。
async fn branch(
    home: &Home,
    models: &Script,
    table: &Arc<Table>,
    seal_after: u32,
) -> (Handle, Handle) {
    let tools = catalog(home);
    let parent = home
        .create_full(models, &tools, Opening::default(), Lines::default())
        .await;
    talk(&parent, "m1", "在吗").await;
    let lane = fork_lane(home, &parent, models, table, seal_after).await;
    (parent, lane)
}

/// 从 `parent` 分出一条跑满 `seal_after` 轮就封存的支线，载入（会话表是 `table`）。
async fn fork_lane(
    home: &Home,
    parent: &Handle,
    models: &Script,
    table: &Arc<Table>,
    seal_after: u32,
) -> Handle {
    let child = new_id(now());
    miyu_session::fork(Fork {
        root: &home.root,
        parent: parent.id(),
        id: child.clone(),
        cause: CommandId::parse("f1").expect("合写法"),
        by: alice(),
        at: now(),
        index: None,
        seal_after: NonZeroU32::new(seal_after),
        within: None,
    })
    .await
    .expect("分叉得了");
    let port = Arc::clone(table) as Arc<dyn SessionPort>;
    home.load_full(
        &child,
        models,
        &catalog(home),
        &environment().cwd,
        Some(port),
    )
    .await
}

/// 等到会话 `session` 报过 `count` 次退得下。
async fn until_rung(table: &Table, session: &SessionId, count: usize) {
    within("报退得下", async {
        while table.rings(session) < count {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await;
}

/// 一轮就封存、一轮不够抽的：判完（不抽）当场报，没人来问也报；问了就退，之后再交命令是「会话停了」。
#[tokio::test]
async fn a_sealed_lane_too_short_to_extract_rings_at_once() {
    let server = Server::start(Vec::new()).await;
    let mut home = Home::new();
    organizer_after(&mut home, &server, "", LONG);
    let table = Arc::new(Table::default());
    let models = script([Play::Says("在。"), Play::Says("支线。")]);
    let (_parent, lane) = branch(&home, &models, &table, 1).await;
    talk(&lane, "m2", "再来").await;
    assert!(sealed(&home, lane.id()), "跑完一轮封存了");
    until_rung(&table, lane.id(), 1).await;
    assert_eq!(lane.retire(LONG).await, Ok(Retire::Retired));
    assert_eq!(
        lane.command(id("m3"), alice(), say("还在吗")).await,
        Err(Stopped),
        "退下了"
    );
    assert!(server.received().is_empty(), "一轮不够抽，不发请求");
}

/// 有头订阅着的不报：封存那一批送完、再办完一封，还是一次都没报；放下订阅以后报。
#[tokio::test]
async fn a_watched_sealed_lane_rings_only_after_the_head_goes() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let models = script([Play::Says("在。"), Play::Says("支线。")]);
    let (_parent, lane) = branch(&home, &models, &table, 1).await;
    let mut pushes = watch(&lane).await;
    ask(&lane, "m2", say("再来")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert!(sealed(&home, lane.id()));
    // 封存那一批以后 actor 又办完了一封：要报早就报了。
    lane.current().await.expect("会话在跑");
    assert_eq!(table.rings(lane.id()), 0, "订阅着，不报");
    drop(pushes);
    until_rung(&table, lane.id(), 1).await;
    assert_eq!(lane.retire(LONG).await, Ok(Retire::Retired));
}

/// 报过以后放下一个把手再报一次：会话表上次问时它还拿着，「除了表里那一个没有别的把手」不成。问过（没退）以后放下的不报。
#[tokio::test]
async fn letting_go_of_a_handle_rings_again() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let models = script([Play::Says("在。"), Play::Says("支线。")]);
    let (_parent, lane) = branch(&home, &models, &table, 1).await;
    talk(&lane, "m2", "再来").await;
    until_rung(&table, lane.id(), 1).await;
    let held = lane.clone();
    assert_eq!(table.rings(lane.id()), 1);
    drop(held);
    assert_eq!(table.rings(lane.id()), 2, "放下的那一刻报");
    assert_eq!(lane.retire(LONG).await, Ok(Retire::Retired));
    let id = lane.id().clone();
    drop(lane);
    assert_eq!(table.rings(&id), 2, "退下以后放下把手不再报");
}

/// 没封存的不报：说完一轮、闲着、再办完一封，一次都没报（闲够了才退，照会话表隔一阵问）。
#[tokio::test]
async fn an_unsealed_session_never_rings() {
    let home = Home::new();
    let table = Arc::new(Table::default());
    let handle = session(&home, &script([Play::Says("在。")]), &table).await;
    talk(&handle, "m1", "在吗").await;
    handle.current().await.expect("会话在跑");
    let held = handle.clone();
    drop(held);
    assert_eq!(table.rings(handle.id()), 0);
}

/// 两轮封存、够抽的：抽取在路上不报，问了也答还有事；抽取期间支线又收一封、主会话又分出一条支线跑完；抽完（记进记忆日志）
/// 当场报、退下。新的那一条照它自己的账。
#[tokio::test]
async fn a_lane_extracting_rings_once_the_extraction_is_done() {
    let gate = Arc::new(Barrier::new(2));
    let mut held = stream(r#"{"memories": [{"class": "user", "text": "养了一只猫", "turn": 2}]}"#);
    held.body.insert(0, Piece::Gate(Arc::clone(&gate)));
    let server = Server::start(vec![held]).await;
    let mut home = Home::new();
    organizer_after(&mut home, &server, "", LONG);
    let table = Arc::new(Table::default());
    let models = script([
        Play::Says("在。"),
        Play::Says("一。"),
        Play::Says("二。"),
        Play::Says("另一条。"),
    ]);
    let (parent, lane) = branch(&home, &models, &table, 2).await;
    talk(&lane, "m2", "我养了一只猫").await;
    talk(&lane, "m3", "它叫团子").await;
    assert!(sealed(&home, lane.id()), "跑满两轮封存了");
    within("抽取的请求", async {
        while server.received().is_empty() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    })
    .await;
    assert_eq!(
        lane.retire(LONG).await,
        Ok(Retire::Kept(Hold::Memory)),
        "抽取在路上"
    );
    lane.current().await.expect("会话在跑");
    let other = fork_lane(&home, &parent, &models, &table, 1).await;
    talk(&other, "m4", "另一个人").await;
    assert_eq!(table.rings(lane.id()), 0, "抽完以前不报");
    assert_eq!(mark(&home, lane.id()), None);
    gate.wait().await;
    until_rung(&table, lane.id(), 1).await;
    assert!(
        mark(&home, lane.id()).is_some(),
        "报以前抽到哪已经记进记忆日志"
    );
    assert_eq!(lane.retire(LONG).await, Ok(Retire::Retired));
    assert_eq!(server.received().len(), 1, "只抽一次");
    until_rung(&table, other.id(), 1).await;
    assert_eq!(other.retire(LONG).await, Ok(Retire::Retired));
}

/// 抽取失败（模型出错）：照原来的规矩不当场重试，抽取的账放下了就报、退下；只发了一次请求。
#[tokio::test]
async fn a_failed_extraction_does_not_retry_and_rings() {
    let server = Server::start(vec![Reply::error(500, &[], "boom")]).await;
    let mut home = Home::new();
    organizer_after(&mut home, &server, "", LONG);
    let table = Arc::new(Table::default());
    let models = script([Play::Says("在。"), Play::Says("一。"), Play::Says("二。")]);
    let (_parent, lane) = branch(&home, &models, &table, 2).await;
    talk(&lane, "m2", "我养了一只猫").await;
    talk(&lane, "m3", "它叫团子").await;
    until_rung(&table, lane.id(), 1).await;
    assert_eq!(server.received().len(), 1, "失败了不当场重试");
    assert_eq!(mark(&home, lane.id()), None, "第一次失败不记抽到哪");
    assert_eq!(lane.retire(LONG).await, Ok(Retire::Retired));
}
