//! 闲下来收成槽（施工 LW-10 下，`internal/blueprint/session/actor.md`「闲下来收成槽」）：照 tokio 暂停的时钟拨，不真等。歇着
//! 满 3 分钟才收，其间来了回合的从那一轮重新算；收着时订阅、取消订阅不读日志；再说话先从日志读回，请求和一直不收的逐字节
//! 一样；闲够了退下、内存紧时退下照常，再载入照常。日志坏了读不回来：这一轮一个请求都不发、如实报错，会话不坏，日志好了接着
//! 用。看收没收、读没读照运行日志的 `history shelved`、`history unshelved`、`history unreadable` 那几行。
//!
//! 只有这一个测试，自己一个进程：`tracing` 的调用点第一次被碰到时记下谁在听，别的测试同时碰到，这里装的订阅者可能漏听。

mod support;

use std::time::Duration;

use miyu_kernel::event::Body;
use miyu_log::{LevelFilter, Memory};
use miyu_session::testkit::{Play, Script};
use miyu_session::{Pushed, Retire, Retiring, Subscription};
use support::{Home, ask, say, until_turn_ends, watch};

/// 歇着满这么久收：同 `actor/shelf.rs` 的 `KEEP`。
const KEEP: Duration = Duration::from_secs(180);

/// 拨钟 `by`，再让醒了的任务走完。
async fn pass(by: Duration) {
    tokio::time::advance(by).await;
    for _ in 0..16 {
        tokio::task::yield_now().await;
    }
}

/// 读推送读到标题起好了（`session.meta_changed` 推出来），再让会话办完那一批：第一轮结束以后起标题是另起的任务回的，起标题
/// 在路上的会话不算歇着，从它办完那一刻起算。
async fn until_titled(pushes: &mut Subscription) {
    loop {
        let next = pushes.next().await.expect("订阅没断");
        if matches!(&*next, Pushed::Events(events)
            if events.iter().any(|event| matches!(event.body, Body::MetaChanged(_))))
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

/// 四句话的剧本。起标题的请求也排上：起标题在路上的会话不算歇着，不收。
fn script() -> Script {
    Script::new([
        Play::Says("你好。"),
        Play::Says("嗯。"),
        Play::Says("在。"),
        Play::Says("又在。"),
    ])
    .titles([
        Play::Says("打招呼"),
        Play::Says("打招呼"),
        Play::Says("打招呼"),
    ])
}

#[tokio::test(start_paused = true)]
async fn an_idle_session_shelves_its_history_and_reads_it_back() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::DEBUG,
        None,
    ));
    // 不收的那一份：一直不拨钟，从头到尾不收。
    let held = script();
    let control = {
        let home = Home::new();
        let handle = home.create(&held).await;
        let mut pushes = watch(&handle).await;
        for (id, words) in [("cmd-1", "hi"), ("cmd-2", "再说一句"), ("cmd-3", "第三句")] {
            ask(&handle, id, say(words)).await.expect("会话在跑");
            until_turn_ends(&mut pushes).await;
        }
        asked(&held, handle.id().as_str())
    };
    assert_eq!(count(&memory, "history shelved"), 0, "没歇够的不收");

    let shelving = script();
    let home = Home::new();
    let handle = home.create(&shelving).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    until_titled(&mut pushes).await;
    // 不到 3 分钟不收；其间来了一轮的，从那一轮办完重新算。
    pass(KEEP - Duration::from_secs(1)).await;
    assert_eq!(count(&memory, "history shelved"), 0, "不到 3 分钟不收");
    ask(&handle, "cmd-2", say("再说一句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(count(&memory, "history unshelved"), 0, "没收过的不读");
    pass(KEEP - Duration::from_secs(1)).await;
    assert_eq!(count(&memory, "history shelved"), 0, "又来了一轮，重新算");
    pass(Duration::from_secs(2)).await;
    assert_eq!(count(&memory, "history shelved"), 1, "歇够了收");
    // 收着时订阅、取消订阅：不读日志。
    let more = watch(&handle).await;
    drop(more);
    pass(Duration::from_secs(1)).await;
    assert_eq!(count(&memory, "history unshelved"), 0, "订阅不读日志");
    // 收着的时候说话：先读回，请求和一直不收的一样。
    ask(&handle, "cmd-3", say("第三句"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(count(&memory, "history unshelved"), 1, "说话以前读回");
    assert!(
        asked(&shelving, handle.id().as_str()) == control,
        "收过再放回的请求和一直不收的不一样"
    );
    // 上下文的明细：先读回再答。
    pass(KEEP + Duration::from_secs(1)).await;
    assert_eq!(count(&memory, "history shelved"), 2);
    let context = handle.context().await.expect("会话在跑");
    assert_eq!(count(&memory, "history unshelved"), 2, "要明细以前读回");
    assert!(
        context.breakdown.said.iter().any(|(_, tokens)| *tokens > 0),
        "{context:?}"
    );
    // 收着时闲够了退下、内存紧时退下：照常退。再载入，说话照常。
    pass(KEEP + Duration::from_secs(1)).await;
    assert_eq!(count(&memory, "history shelved"), 3);
    drop(pushes);
    pass(Duration::from_secs(1)).await;
    assert_eq!(handle.retire(Retiring::Tight).await, Ok(Retire::Retired));
    let id = handle.id().clone();
    let handle = home.load(&id, &shelving).await;
    // 订阅一下再放下：载入吐出的那一批办完了才轮到它。
    drop(watch(&handle).await);
    pass(Duration::from_secs(1)).await;
    pass(KEEP + Duration::from_secs(1)).await;
    assert_eq!(
        count(&memory, "history shelved"),
        4,
        "载入以后只被订阅着的也照这个时刻收：{:#?}",
        memory
            .lines()
            .iter()
            .filter(|line| line.contains("history")
                || line.contains("input")
                || line.contains("load"))
            .collect::<Vec<_>>()
    );
    assert_eq!(handle.retire(Duration::ZERO).await, Ok(Retire::Retired));
}

#[tokio::test(start_paused = true)]
async fn a_log_that_cannot_be_read_back_sends_nothing() {
    let memory = Memory::new();
    let _listening = tracing::subscriber::set_default(miyu_log::subscriber(
        memory.clone(),
        LevelFilter::DEBUG,
        None,
    ));
    let script = script();
    let home = Home::new();
    let handle = home.create(&script).await;
    let mut pushes = watch(&handle).await;
    ask(&handle, "cmd-1", say("hi")).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    until_titled(&mut pushes).await;
    pass(KEEP + Duration::from_secs(1)).await;
    assert_eq!(count(&memory, "history shelved"), 1);
    // 日志第二行换成一样长的乱码：读不回来。
    let segment = home
        .root
        .session_dir(handle.id())
        .join("000000000001.jsonl");
    let text = std::fs::read_to_string(&segment).expect("读得出");
    let second = text.lines().nth(1).expect("有第二行").to_string();
    std::fs::write(
        &segment,
        text.replacen(&second, &"x".repeat(second.len()), 1),
    )
    .expect("写得进");
    let sent = script.requests().len();
    ask(&handle, "cmd-2", say("读不回来的这一句"))
        .await
        .expect("会话照旧收下话");
    until_turn_ends(&mut pushes).await;
    assert_eq!(
        count(&memory, "history unreadable"),
        1,
        "{:#?}",
        memory.lines()
    );
    assert_eq!(script.requests().len(), sent, "一个请求都不发");
    // 日志修好了：下一句照常，读不回的那一轮人说的话也在请求里。
    let now = std::fs::read_to_string(&segment).expect("读得出");
    std::fs::write(
        &segment,
        now.replacen(&"x".repeat(second.len()), &second, 1),
    )
    .expect("写得进");
    // 修好了再读日志：读不回的那一轮记了一条没发出去的请求，原话写明为什么。
    let log = home.log(handle.id());
    let called = log.iter().rev().find_map(|event| match &event.body {
        miyu_kernel::event::Body::ModelCalled(called) => Some(called.clone()),
        _ => None,
    });
    let called = called.expect("记了这一次请求");
    let error = called.error.as_ref().expect("没发出去").message.clone();
    assert!(
        error.contains("这个会话的历史从日志读不回来，请求没有发出去：")
            && error.contains("line 2"),
        "{error}"
    );
    ask(&handle, "cmd-3", say("修好了"))
        .await
        .expect("会话在跑");
    until_turn_ends(&mut pushes).await;
    assert_eq!(script.requests().len(), sent + 1, "修好了照常发");
    let (_, last) = script.requests().pop().expect("发了");
    let text = String::from_utf8(last.canonical_bytes()).expect("是字");
    for words in ["hi", "读不回来的这一句", "修好了"] {
        assert!(text.contains(words), "请求里少了「{words}」");
    }
}
