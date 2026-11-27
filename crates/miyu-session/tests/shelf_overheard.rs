//! 群里旁听的话不把收起来的历史读回来，也不推迟收起（施工 LW-36，`internal/blueprint/session/actor.md`「闲下来收成槽」）：照
//! tokio 暂停的时钟拨，不真等。一个群会话，照一句旁听的话开过一轮以后，群里每隔一阵有人说话：歇够 3 分钟照样收；收着时又接连
//! 来几句，一次都不读日志；再照最后一句开一轮，才读回一次，请求（连同里面的群聊近况）和一直不收的那个群会话逐字节一样。看收没收、
//! 读没读照运行日志的 `history shelved`、`history unshelved` 那几行。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听。

mod support;

use std::time::Duration;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::VenueMessage;
use miyu_kernel::id::{ExternalId, Seq, VenueId};
use miyu_kernel::origin::{By, External, Role};
use miyu_kernel::session::{Command, Outcome};
use miyu_log::{LevelFilter, Memory};
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;
use support::{Home, Lines, Opening, ask, id, until_turn_ends, watch, within};

/// 歇着满这么久收：同 `actor/shelf.rs` 的 `KEEP`。
const KEEP: Duration = Duration::from_secs(180);

/// 收着时接连来几句旁听的话。
const WHILE_SHELVED: usize = 6;

/// 拨钟 `by`，再让醒了的任务走完。
async fn pass(by: Duration) {
    tokio::time::advance(by).await;
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

/// 读推送读到标题起好了（`session.meta_changed` 推出来），再让会话办完那一批：第一轮结束以后起标题是另起的任务回的，回来的那一批
/// 要看历史，从它办完那一刻起算（同 `shelf_log.rs`）。
async fn until_titled(pushes: &mut miyu_session::Subscription) {
    loop {
        let next = pushes.next().await.expect("订阅没断");
        if matches!(&*next, miyu_session::Pushed::Events(events)
            if events.iter().any(|event| matches!(event.body, miyu_kernel::event::Body::MetaChanged(_))))
        {
            break;
        }
    }
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

/// 运行日志里有几行是 `what` 的。
fn count(memory: &Memory, what: &str) -> usize {
    memory
        .lines()
        .iter()
        .filter(|line| line.contains(what))
        .count()
}

/// 每一次请求照内核的写法，会话编号换成 `<id>`：两个会话的编号不一样，别的一字不差才算一样。
fn asked(script: &Script, id: &str) -> Vec<String> {
    script
        .requests()
        .iter()
        .map(|(_, request)| {
            String::from_utf8(request.canonical_bytes())
                .expect("是字")
                .replace(id, "<id>")
        })
        .collect()
}

/// 两轮回复的剧本。起标题的也排上：起标题在路上的会话不算歇着。
fn script() -> Script {
    Script::new([Play::Says("在。"), Play::Says("又在。")])
        .titles([Play::Says("群聊"), Play::Says("群聊")])
}

/// 一个群会话。
async fn group(home: &Home, script: &Script) -> Handle {
    let lines = Lines {
        venue: VenueId::parse("qq:group:1").expect("合写法"),
        group: true,
        ..Lines::default()
    };
    home.create_full(script, &Catalog::default(), Opening::default(), lines)
        .await
}

/// 群里的小林（`qq:20017`）。
fn member() -> By {
    By::External(External {
        venue: VenueId::parse("qq:group:1").expect("合写法"),
        id: ExternalId::parse("qq:20017").expect("合写法"),
        owner: false,
        role: Some(Role::Member),
    })
}

/// 群里小林说的第 `k` 句，桥照旁听交进来（`venue.ambient`）：交回它的序号。
async fn hear(handle: &Handle, k: usize) -> Seq {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: format!("群里的第 {k} 句"),
        })],
        urgent: false,
        venue: Some(Box::new(VenueMessage {
            msg: format!("88{k:02}"),
            name: Some("小林".to_string()),
            ambient: true,
            ..VenueMessage::default()
        })),
    };
    let outcome = within(
        "旁听的回应",
        handle.command(id(&format!("heard-{k}")), member(), said),
    )
    .await
    .expect("会话在跑");
    match outcome {
        Outcome::Accepted { events } => events[0],
        other => panic!("旁听的话没收下：{other:?}"),
    }
}

/// 照旁听的第 `to` 条开一轮（`session.respond`），等这一轮说完。
async fn respond(handle: &Handle, pushes: &mut miyu_session::Subscription, k: &str, to: Seq) {
    let command = Command::Respond {
        to: vec![to],
        facts: Vec::new(),
        tail: None,
    };
    ask(handle, k, command).await.expect("会话在跑");
    until_turn_ends(pushes).await;
}

#[tokio::test(start_paused = true)]
async fn overheard_messages_neither_read_the_history_back_nor_put_off_shelving() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::DEBUG,
        None,
    ));
    // 不收的那一份：一直不拨钟，从头到尾不收；说的话、开的轮和下面一样。
    let held = script();
    let control = {
        let home = Home::new();
        let handle = group(&home, &held).await;
        let mut pushes = watch(&handle).await;
        let first = hear(&handle, 1).await;
        respond(&handle, &mut pushes, "respond-1", first).await;
        let mut last = first;
        for k in 2..=3 + WHILE_SHELVED {
            last = hear(&handle, k).await;
        }
        respond(&handle, &mut pushes, "respond-2", last).await;
        asked(&held, handle.id().as_str())
    };
    assert_eq!(count(&memory, "history shelved"), 0, "没歇够的不收");

    let shelving = script();
    let home = Home::new();
    let handle = group(&home, &shelving).await;
    let mut pushes = watch(&handle).await;
    let first = hear(&handle, 1).await;
    respond(&handle, &mut pushes, "respond-1", first).await;
    until_titled(&mut pushes).await;
    // 这一轮（连同起标题）办完以后群里每隔一阵有人说话：旁听不往后推，从这一轮办完起歇够 3 分钟照样收。
    pass(Duration::from_secs(60)).await;
    hear(&handle, 2).await;
    pass(Duration::from_secs(60)).await;
    hear(&handle, 3).await;
    pass(KEEP - Duration::from_secs(121)).await;
    assert_eq!(count(&memory, "history shelved"), 0, "不到 3 分钟不收");
    pass(Duration::from_secs(2)).await;
    assert_eq!(
        count(&memory, "history shelved"),
        1,
        "一直有人说话也照这一轮办完的时刻收：{:#?}",
        memory.lines()
    );
    // 收着时接连来几句：一次都不读日志，也不再收一遍。
    let mut last = first;
    for k in 4..=3 + WHILE_SHELVED {
        last = hear(&handle, k).await;
        pass(Duration::from_secs(30)).await;
    }
    assert_eq!(count(&memory, "history unshelved"), 0, "旁听的话不读日志");
    assert_eq!(count(&memory, "history shelved"), 1, "一直收着");
    // 照最后一句开一轮：先读回一次，请求和一直不收的一样。
    respond(&handle, &mut pushes, "respond-2", last).await;
    assert_eq!(count(&memory, "history unshelved"), 1, "开一轮以前读回一次");
    let shelved = asked(&shelving, handle.id().as_str());
    assert_eq!(shelved.len(), 2, "两轮各请求一次");
    assert!(
        shelved[1].contains("群里的第 4 句") && shelved[1].contains("群里的第 9 句"),
        "第二轮的请求里有收着时来的旁听的话：{}",
        shelved[1]
    );
    assert!(shelved == control, "收着时来过旁听的请求和一直不收的不一样");
}
