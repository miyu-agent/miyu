//! 她自己定节奏的循环没定下一次就结束（施工 LO-3，项目主人 2026-11-16 定，照 Claude Code「不调就停」）：真核心、手拨的钟。
//! `/loop <话>` 交进去开的那一轮里她没调 `schedule_next`，那一轮结束时删掉这一行、推 `removed`；调了的留着、下一次是她定的；
//! 人插的一轮不算。`/loop clear`、`/loop stop` 同 `/loop off`。以前那一行停着等她定，头上的图标在、循环其实停了。核心在她那一轮
//! 里停了的，起来时删掉（施工 LO-3补）。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_endpoint::schedules::{Creator, Loop, Schedule, Target, Wanted, When};
use miyu_kernel::id::SessionId;
use miyu_kernel::time::Timestamp;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::clock::{Hand, core_with_tools};
use crate::support::*;

/// 钟从这一刻起：2026-10-26T00:00:00Z。
const START: i64 = 1_792_972_800_000;

fn looping() -> Catalog {
    let tools = miyu_schedule::loop_tools(&default_resources()).expect("出厂的字");
    Catalog::in_packages([(miyu_schedule::LOOP, tools)]).expect("合写法")
}

async fn typed(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    let params = json!({"session": session, "text": text});
    client.call(id, "command.run", params).await
}

fn rows(core: &Core) -> Vec<Schedule> {
    core.schedules().list().expect("读得出")
}

/// 起好唤醒表的核心、握好手的连接、开好的会话。
async fn started(home: &Home, script: &Script, hand: &Arc<Hand>) -> (Arc<Core>, Client, String) {
    let core = core_with_tools(home, script, hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    core.start_schedules();
    (core, client, session)
}

/// 订着 `schedules` 流的另一个连接。
async fn watching(core: &Arc<Core>) -> Client {
    let mut watcher = Client::connect(Arc::clone(core));
    watcher.hello().await;
    watcher
        .call("w", "subscribe", json!({"stream": "schedules"}))
        .await;
    watcher
}

/// 一直读 `schedules.changed`，读到任务 `id` 删了的那一条。
async fn until_removed(watcher: &mut Client, id: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        let left = deadline.saturating_duration_since(tokio::time::Instant::now());
        let message = watcher
            .next_within(left)
            .await
            .unwrap_or_else(|| panic!("等不到 {id} 删了的推送"));
        if message["method"] == "schedules.changed"
            && message["params"] == json!({"id": id, "removed": true})
        {
            return;
        }
    }
}

/// 她这一轮里没调 `schedule_next`：那一轮结束时这一行删了，推了 `removed`。
#[tokio::test]
async fn a_dynamic_loop_ends_when_its_turn_sets_no_next() {
    let home = Home::new();
    let script = Script::new([Play::Says("现在是早上八点。")]);
    let hand = Hand::at(START);
    let (core, mut client, session) = started(&home, &script, &hand).await;
    let mut watcher = watching(&core).await;
    typed(&mut client, "k1", &session, "/loop 报一下时间").await;
    home.until_turns(&session, 1).await;
    until_removed(&mut watcher, "t1").await;
    assert!(rows(&core).is_empty(), "{:?}", rows(&core));
}

/// 她这一轮里调了 `schedule_next`：这一行留着，下一次是她定的，到点再跑一轮。
#[tokio::test]
async fn a_dynamic_loop_whose_turn_sets_the_next_stays() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("schedule_next", r#"{"delay_seconds":120}"#)]),
        Play::Says("两分钟后再报。"),
        Play::Says("又报了一次。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client, session) = started(&home, &script, &hand).await;
    typed(&mut client, "k1", &session, "/loop 报一下时间").await;
    home.until_turns(&session, 1).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let row = rows(&core).pop().expect("留着");
    assert_eq!(row.looping, Some(Loop::Dynamic));
    assert_eq!(
        row.next.map(|next| next.unix_millis()),
        Some(START + 120_000)
    );
    hand.advance(120_000);
    home.until_turns(&session, 2).await;
}

/// 循环等着下一次时人插了一轮，她这一轮没调 `schedule_next`：不是循环自己那一轮，不删，下一次照她上次定的。
#[tokio::test]
async fn a_turn_a_person_starts_while_the_loop_waits_does_not_end_it() {
    let home = Home::new();
    let script = Script::new([
        Play::calls(&[("schedule_next", r#"{"delay_seconds":600}"#)]),
        Play::Says("十分钟后再看。"),
        Play::Says("你好。"),
    ]);
    let hand = Hand::at(START);
    let (core, mut client, session) = started(&home, &script, &hand).await;
    typed(&mut client, "k1", &session, "/loop 盯着部署").await;
    home.until_turns(&session, 1).await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 2).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let row = rows(&core).pop().expect("留着");
    assert_eq!(
        row.next.map(|next| next.unix_millis()),
        Some(START + 600_000)
    );
}

/// `/loop clear`、`/loop STOP`、`/loop stop ` 同 `/loop off`；`/loop clear the cache` 是要说的话，开一个循环。唤醒表不起：
/// 只看命令建了、删了哪一行，不跑。
#[tokio::test]
async fn clear_and_stop_alone_turn_the_loop_off() {
    let home = Home::new();
    let hand = Hand::at(START);
    let core = core_with_tools(&home, &Script::new([]), &hand, looping());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    for (n, off) in ["/loop clear", "/loop STOP", "/loop stop "]
        .into_iter()
        .enumerate()
    {
        typed(
            &mut client,
            &format!("on{n}"),
            &session,
            "/loop 10m 看看 CI",
        )
        .await;
        assert_eq!(rows(&core).len(), 1);
        let reply = typed(&mut client, &format!("off{n}"), &session, off).await;
        assert_eq!(
            reply["result"]["said"], "已停止这个会话的循环。",
            "{off:?}：{reply}"
        );
        assert!(rows(&core).is_empty(), "{off:?}：{:?}", rows(&core));
    }
    let reply = typed(&mut client, "words", &session, "/loop clear the cache").await;
    assert_eq!(
        reply["result"]["said"], "已开始循环：由 AI 决定下一次运行的时间 · clear the cache",
        "{reply}"
    );
    let row = rows(&core).pop().expect("开了一个");
    assert_eq!(row.prompt, "clear the cache");
}

/// 核心没来得及看完循环那一轮就没了（施工 LO-3补：被强杀、断电，那一行还是交过以后没定下一次的样子），起来时会话空着，删掉；
/// 人停着的那一行留着给人续，定了下一次的照旧。
#[tokio::test]
async fn a_loop_left_without_a_next_ends_on_restart() {
    let home = Home::new();
    let hand = Hand::at(START);
    let ids = {
        let first = core_with_tools(&home, &Script::new([]), &hand, looping());
        let mut client = Client::connect(Arc::clone(&first));
        client.hello().await;
        let session = client.create("c1", "~").await;
        let schedules = first.schedules();
        let dynamic = |prompt: &str| Wanted {
            by: Creator::Person,
            when: When::At {
                at: Timestamp::from_unix_millis(START + 600_000).expect("时刻"),
            },
            prompt: prompt.to_string(),
            target: Target::Session {
                session: SessionId::parse(&session).expect("编号"),
            },
            looping: Some(Loop::Dynamic),
            first: None,
        };
        let left = schedules.add(dynamic("交过、没定")).expect("建得了");
        schedules.set_next(&left.id, None, None).expect("清得掉");
        let paused = schedules.add(dynamic("人停着的")).expect("建得了");
        schedules.pause(&paused.id).expect("停得了");
        let set = schedules.add(dynamic("定了的")).expect("建得了");
        (left.id, paused.id, set.id)
    };
    let core = core_with_tools(&home, &Script::new([]), &hand, looping());
    core.start_schedules();
    until("没定下一次的那一行删了", || {
        rows(&core).iter().all(|row| row.id != ids.0)
    })
    .await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let left: Vec<String> = rows(&core).into_iter().map(|row| row.id).collect();
    assert!(left.contains(&ids.1) && left.contains(&ids.2), "{left:?}");
}

/// 有计划地停（施工 LO-3补，修 LO-3 带进来的）：循环那一轮停在请求上时核心停下，那一轮照「重启了」结束，那一行留着、下一次
/// 还空着。再起来那一轮接着干，她在接着的一轮里定了下一次：那一行留着、下一次是她定的。
async fn stopped_in_its_turn(home: &Home, hand: &Arc<Hand>) -> String {
    let script = Script::new([Play::Holds]);
    let (first, mut client, session) = started(home, &script, hand).await;
    typed(&mut client, "k1", &session, "/loop 报一下时间").await;
    until("循环那一轮请求了", || script.requests().len() == 1).await;
    first.stop_sessions().await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let kept = rows(&first);
    assert_eq!(kept.len(), 1, "停的时候不删：{kept:?}");
    assert_eq!(kept[0].next, None);
    session
}

#[tokio::test]
async fn a_planned_stop_lets_the_resumed_turn_set_the_next() {
    let home = Home::new();
    let hand = Hand::at(START);
    let session = stopped_in_its_turn(&home, &hand).await;
    let script = Script::new([
        Play::calls(&[("schedule_next", r#"{"delay_seconds":120}"#)]),
        Play::Says("两分钟后再报。"),
    ]);
    let core = core_with_tools(&home, &script, &hand, looping());
    core.start_schedules();
    let set = || rows(&core).first().and_then(|row| row.next);
    until("接着的那一轮定了下一次", || set().is_some()).await;
    assert_eq!(set().map(|next| next.unix_millis()), Some(START + 120_000));
    home.until_turns(&session, 2).await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(rows(&core).len(), 1, "接着的那一轮结束以后留着");
}

/// 有计划地停以后接着干的那一轮里她没定下一次：那一轮结束时删掉。
#[tokio::test]
async fn a_resumed_turn_that_sets_no_next_ends_the_loop() {
    let home = Home::new();
    let hand = Hand::at(START);
    let session = stopped_in_its_turn(&home, &hand).await;
    let script = Script::new([Play::Says("现在是早上八点。")]);
    let core = core_with_tools(&home, &script, &hand, looping());
    core.start_schedules();
    until("接着的那一轮请求了", || {
        script.requests().len() == 1
    })
    .await;
    home.until_turns(&session, 2).await;
    until("那一行删了", || rows(&core).is_empty()).await;
}
