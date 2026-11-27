//! 量尺（施工 LW-10 下，`#[ignore]`）：一个约 1 MB 上下文的会话，内核收成槽以后这一份占多少堆。照 glibc 的 `mallinfo2`
//! （`miyu_heap::stats` 的 `used`）量载入以前、载入以后、收成槽以后三刻，断言收成槽以后比载入以前多的不超过 150 kB（施工单
//! LW-10「验收」第 3 条）。只断言结果，不断言耗时。
//!
//! 另一把（施工 LW-36）：一个记过 1500 句旁听的话的群会话，收成槽以后照桥的样子接着灌两回、每回 1000 句（每句一个
//! `session.send`、一次落盘），内核这一份不随条数长；同样灌进一直不收的那一份，对照长了多少。头一回长的大头是最近 1024 个命令
//! 编号的指纹（载入时认回来的编号没有指纹，灌进去的有，满了就丢最早的），第二回只剩账本里每句旁听一个序号
//! （`kernel/history.md`「对外的样子」）。
//!
//! 量整个进程的堆，自己一个程序：同一个进程里别的测试并着分配的会算进来。只有 glibc 上量得出，别的平台跳过。
//!
//! `cargo test -p miyu-session --release --test shelved_bytes -- --ignored --nocapture`

use std::path::Path;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Event, Level, Permission, VenueMessage};
use miyu_kernel::facts::Environment;
use miyu_kernel::id::{CommandId, ExternalId, SessionId, VenueId};
use miyu_kernel::origin::{By, External};
use miyu_kernel::session::{Action, Command, Input, Received, Session};
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_policy::compose;
use miyu_store::resources::ResourceRoot;

/// 多少轮、每轮人说的和她回的各多少字节：200 轮 × 2 × 2600 字节，上下文约 1 MB。
const TURNS: u64 = 200;
const CHARS: usize = 2600;

/// 每一行日志都写这一刻：账本不看时刻的先后。
const AT: &str = "2026-11-22T00:00:00.000Z";

/// 一轮：人说一句（`seq`），回合开始，她回一句，回合结束。占四个序号。
fn turn(seq: u64, words: &str) -> Vec<String> {
    let started = seq + 1;
    vec![
        format!(
            r#"{{"seq":{seq},"at":"{AT}","kind":"message.user","by":{{"kind":"person"}},"cause":"cmd-{seq}","body":{{"blocks":[{{"type":"text","text":"{words}"}}]}}}}"#
        ),
        format!(
            r#"{{"seq":{started},"at":"{AT}","kind":"turn.started","turn":{started},"by":{{"kind":"kernel"}},"cause":"cmd-{seq}","body":{{"trigger":{seq}}}}}"#
        ),
        format!(
            r#"{{"seq":{},"at":"{AT}","kind":"message.assistant","turn":{started},"by":{{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}},"body":{{"blocks":[{{"type":"text","text":"{words}"}}],"seen":{started}}}}}"#,
            seq + 2
        ),
        format!(
            r#"{{"seq":{},"at":"{AT}","kind":"turn.ended","turn":{started},"by":{{"kind":"kernel"}},"body":{{"reason":"completed"}}}}"#,
            seq + 3
        ),
    ]
}

#[test]
#[ignore = "量尺：cargo test -p miyu-session --release --test shelved_bytes -- --ignored --nocapture"]
fn a_shelved_session_holds_little() {
    let Some(_) = miyu_heap::stats() else {
        return;
    };
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let snapshot = compose(
        Some("engineer"),
        resources
            .sources_with(miyu_session::testkit::sample_persona_texts())
            .expect("核心的字读得出来"),
        true,
    );
    let created = snapshot.session_created(
        VenueId::parse("local").expect("场所合写法"),
        Permission {
            level: Level::Full,
            read_only: false,
        },
    );
    let first = format!(
        r#"{{"seq":1,"at":"{AT}","kind":"session.created","by":{{"kind":"kernel"}},"cause":"cmd-0","body":{}}}"#,
        serde_json::to_string(&created).expect("写得出")
    );
    let words = "字".repeat(CHARS / 3);
    let environment = Environment {
        offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
        cwd: "~/src/miyu".to_string(),
        dirs: Vec::new(),
    };
    let policy = snapshot.policy().expect("出厂的快照造得出策略");
    let id = SessionId::parse("01a0f233-cfec-7023-8ed5-2a037a1d5ec8").expect("会话编号合写法");
    let at = Timestamp::from_unix_millis(1_800_000_000_000).expect("在范围里");
    miyu_heap::trim();
    // 从这一刻起算：日志一行行现造现解，不留整份原文，交给内核的就是解出来的事件。
    let before = used();
    let lines = std::iter::once(first).chain((0..TURNS).flat_map(|k| turn(2 + 4 * k, &words)));
    let events: Vec<Event> = lines
        .map(|line| Event::from_line(&line).expect("读得成"))
        .collect();
    let (mut session, _) = Session::load(id, events, at, policy, environment).expect("载入得了");
    miyu_heap::trim();
    let loaded = used();
    let taken = session.shelve().expect("歇着，收得了");
    let shelved_events = taken.len();
    drop(taken);
    miyu_heap::trim();
    let shelved = used();
    let request = session.shelved().expect("收着");
    let kb = |bytes: i64| bytes / 1024;
    println!(
        "上下文约 {} kB；载入以后内核占 {} kB；收成槽（{shelved_events} 条，从第 {} 条读回）以后 {} kB",
        TURNS * 2 * CHARS as u64 / 1024,
        kb(loaded - before),
        request.get(),
        kb(shelved - before)
    );
    assert!(
        shelved - before <= 150 * 1024,
        "收成槽以后内核这一份还占 {} kB",
        kb(shelved - before)
    );
    drop(session);
}

/// 群会话载入时记过几句旁听的话、收成槽以后再灌几句，一句多少字节（施工 LW-36）。
const HEARD: u64 = 1500;
const MORE: u64 = 1000;
const SAID: usize = 600;

/// 旁听的第 `seq` 条，照日志的一行。
fn heard_line(seq: u64, words: &str) -> String {
    format!(
        r#"{{"seq":{seq},"at":"{AT}","kind":"message.user","by":{{"kind":"external","venue":"qq:group:1","id":"qq:20017"}},"cause":"heard-{seq}","body":{{"blocks":[{{"type":"text","text":"{words}"}}],"venue":{{"msg":"m{seq}","name":"小林","ambient":true}}}}}}"#
    )
}

/// 桥交进来的第 `k` 句旁听的话。
fn heard(k: u64, words: &str, at: Timestamp) -> Input {
    Input::Command(Received {
        id: CommandId::parse(&format!("more-{k}")).expect("命令编号合写法"),
        by: By::External(External {
            venue: VenueId::parse("qq:group:1").expect("合写法"),
            id: ExternalId::parse("qq:20017").expect("合写法"),
            owner: false,
            role: None,
        }),
        at,
        command: Command::Send {
            blocks: vec![Block::Text(Text {
                text: words.to_string(),
            })],
            urgent: false,
            venue: Some(Box::new(VenueMessage {
                msg: format!("n{k}"),
                name: Some("小林".to_string()),
                ambient: true,
                ..VenueMessage::default()
            })),
        },
    })
}

/// 照执行器的样子灌 [`MORE`] 句，命令编号从 `from` 起：每句送进去，追加的落了盘再送「落了盘」，交出的动作放掉。收着的会话
/// 每一条都不用先放回。
fn pour(session: &mut Session, words: &str, at: Timestamp, from: u64) {
    for k in from..from + MORE {
        let input = heard(k, words, at);
        assert!(
            session.shelved().is_none() || !session.needs_history(&input),
            "收着时旁听的话要先放回"
        );
        let actions = session.handle(input);
        let upto = actions
            .iter()
            .find_map(|action| match action {
                Action::Append(events) => events.last().map(|event| event.seq),
                _ => None,
            })
            .expect("旁听的话记下了");
        let stored = Input::Stored { at, upto };
        assert!(session.shelved().is_none() || !session.needs_history(&stored));
        drop(actions);
        drop(session.handle(stored));
    }
}

#[test]
#[ignore = "量尺：cargo test -p miyu-session --release --test shelved_bytes -- --ignored --nocapture"]
fn a_shelved_group_does_not_grow_with_what_it_overhears() {
    let Some(_) = miyu_heap::stats() else {
        return;
    };
    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let snapshot = compose(
        Some("engineer"),
        resources
            .sources_with(miyu_session::testkit::sample_persona_texts())
            .expect("核心的字读得出来"),
        true,
    );
    let created = snapshot.session_created(
        VenueId::parse("qq:group:1").expect("场所合写法"),
        Permission {
            level: Level::Full,
            read_only: false,
        },
    );
    let first = format!(
        r#"{{"seq":1,"at":"{AT}","kind":"session.created","by":{{"kind":"kernel"}},"cause":"cmd-0","body":{}}}"#,
        serde_json::to_string(&created).expect("写得出")
    );
    let words = "字".repeat(SAID / 3);
    let environment = Environment {
        offset: UtcOffset::from_minutes(540).expect("东九区在范围里"),
        cwd: "~/src/miyu".to_string(),
        dirs: Vec::new(),
    };
    let at = Timestamp::from_unix_millis(1_800_000_000_000).expect("在范围里");
    let load = |id: &str| {
        let policy = snapshot.policy().expect("出厂的快照造得出策略");
        let lines =
            std::iter::once(first.clone()).chain((2..2 + HEARD).map(|seq| heard_line(seq, &words)));
        let events: Vec<Event> = lines
            .map(|line| Event::from_line(&line).expect("读得成"))
            .collect();
        let id = SessionId::parse(id).expect("会话编号合写法");
        let (session, _) =
            Session::load(id, events, at, policy, environment.clone()).expect("载入得了");
        session
    };
    let kb = |bytes: i64| bytes / 1024;
    // 一直不收的那一份：灌进去长了多少，对照用。
    miyu_heap::trim();
    let mut held = load("01a0f233-cfec-7023-8ed5-2a037a1d5ec9");
    miyu_heap::trim();
    let held_before = used();
    pour(&mut held, &words, at, 0);
    pour(&mut held, &words, at, MORE);
    miyu_heap::trim();
    let held_grew = used() - held_before;
    drop(held);
    // 收着的那一份。
    miyu_heap::trim();
    let mut session = load("01a0f233-cfec-7023-8ed5-2a037a1d5eca");
    let taken = session.shelve().expect("歇着，收得了");
    let shelved_events = taken.len();
    drop(taken);
    miyu_heap::trim();
    let shelved = used();
    pour(&mut session, &words, at, 0);
    miyu_heap::trim();
    let once = used();
    pour(&mut session, &words, at, MORE);
    assert!(session.shelved().is_some(), "灌完了还收着");
    miyu_heap::trim();
    let (first, second) = (once - shelved, used() - once);
    println!(
        "群会话记过 {HEARD} 句、收成槽（{shelved_events} 条）以后灌两回、每回 {MORE} 句（一句 {SAID} 字节）：内核这一份头一回长了 {first} 字节（{} kB），第二回长了 {second} 字节（{} kB）；一直不收的那一份两回共长了 {held_grew} 字节（{} kB）",
        kb(first),
        kb(second),
        kb(held_grew)
    );
    assert!(
        second <= 32 * 1024,
        "收着再灌 {MORE} 句，内核这一份长了 {} kB",
        kb(second)
    );
    assert!(
        first <= 128 * 1024,
        "收着灌头 {MORE} 句，内核这一份长了 {} kB",
        kb(first)
    );
    drop(session);
}

/// 堆里这时在用的字节。
fn used() -> i64 {
    let stats = miyu_heap::stats().expect("glibc 上量得出");
    i64::try_from(stats.used + stats.mmapped).expect("放得下")
}
