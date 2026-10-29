//! 回合索引在后台写（施工 V-9）：接上时自己读日志，后来交来的一批里已经铺过的不再看。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::Event;
use miyu_kernel::id::{SessionId, VenueId};
use miyu_store::env::{Env, Platform};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_store::recall::{Origin, RecallIndexes, Room};
use miyu_store::root::DataRoot;

use super::{Laid, Lanes, Opening, Source};
use crate::agents::LOCAL;

const ALICE: &str = r#"{"kind":"person"}"#;
const MODEL: &str = r#"{"kind":"model","endpoint":"deepseek","model":"deepseek-v4"}"#;
const KERNEL: &str = r#"{"kind":"kernel"}"#;

/// 临时目录：用完删掉。
struct Scratch(PathBuf);

impl Drop for Scratch {
    fn drop(&mut self) {
        drop(std::fs::remove_dir_all(&self.0));
    }
}

fn root(name: &str) -> (Scratch, DataRoot) {
    let scratch =
        Scratch(std::env::temp_dir().join(format!("miyu-lane-{name}-{}", std::process::id())));
    let env = Env {
        platform: Platform::current(),
        miyu_home: Some(scratch.0.join("data").into_os_string()),
        home: None,
        xdg_cache_home: None,
        local_app_data: None,
        miyu_resources: None,
        exe: None,
    };
    let root = DataRoot::locate(&env).expect("MIYU_HOME 是绝对路径");
    root.prepare().expect("临时目录里建得了骨架");
    (scratch, root)
}

fn line(seq: u64, kind: &str, turn: Option<u64>, by: &str, body: &str) -> Event {
    let turn = turn.map_or(String::new(), |turn| format!(r#","turn":{turn}"#));
    Event::from_line(&format!(
        r#"{{"seq":{seq},"at":"2026-10-20T08:00:{:02}.000Z","kind":"{kind}"{turn},"by":{by},"body":{body}}}"#,
        seq % 60
    ))
    .expect("写对了")
}

#[test]
fn what_was_laid_from_the_log_is_not_seen_again() {
    let (_scratch, root) = root("again");
    let session = SessionId::parse("01a0f233-cfec-7023-8ed5-2a037a1d5ec9").expect("编号写对了");
    let dir = root.session_dir(&session);
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).expect("建得了日志");
    let said = line(
        1,
        "message.user",
        None,
        ALICE,
        r#"{"blocks":[{"type":"text","text":"周末去爬山"}]}"#,
    );
    let started = line(2, "turn.started", Some(2), KERNEL, r#"{"trigger":1}"#);
    let replied = line(
        3,
        "message.assistant",
        Some(2),
        MODEL,
        r#"{"blocks":[{"type":"text","text":"好。"}],"seen":2}"#,
    );
    let ended = line(
        4,
        "turn.ended",
        Some(2),
        KERNEL,
        r#"{"reason":"completed"}"#,
    );
    // 载入以后、后台读日志以前又落了一条：后台读到的已经有它，交来的这一批里又有它。
    log.append(&[said, started.clone()]).expect("写得进");
    let recall = Arc::new(RecallIndexes::new(&root));
    let room = Room::session(&session);
    let lanes = Lanes::default();
    let mut lane = lanes
        .open(Opening {
            recall: Arc::clone(&recall),
            room: room.clone(),
            session: session.clone(),
            origin: origin(),
            venue: VenueId::parse(LOCAL).expect("场所写对了"),
            floor: None,
            laid: Laid::Read(&[]),
            source: Source::of(&log, None),
        })
        .expect("起得了后台");
    lane.advance(&[started]);
    lane.advance(&[replied, ended]);
    lanes.settle();
    let (index, _) = recall.turns(&room);
    assert_eq!(
        index.keys().expect("列得出"),
        vec![format!("{session}/2")],
        "这一轮照样记上"
    );
}

#[test]
fn nothing_is_opened_until_the_lane_is_started() {
    let (scratch, root) = root("later");
    let session = SessionId::parse("01a0f233-cfec-7023-8ed5-2a037a1d5eca").expect("编号写对了");
    let mut log =
        SessionLog::create(&root.session_dir(&session), SEGMENT_LIMIT).expect("建得了日志");
    log.append(&[
        line(
            1,
            "message.user",
            None,
            ALICE,
            r#"{"blocks":[{"type":"text","text":"周末去爬山"}]}"#,
        ),
        line(2, "turn.started", Some(2), KERNEL, r#"{"trigger":1}"#),
        line(
            3,
            "message.assistant",
            Some(2),
            MODEL,
            r#"{"blocks":[{"type":"text","text":"好。"}],"seen":2}"#,
        ),
        line(
            4,
            "turn.ended",
            Some(2),
            KERNEL,
            r#"{"reason":"completed"}"#,
        ),
    ])
    .expect("写得进");
    let recall = Arc::new(RecallIndexes::new(&root));
    let room = Room::session(&session);
    let lanes = Lanes::default();
    let lane = lanes
        .open(Opening {
            recall: Arc::clone(&recall),
            room: room.clone(),
            session: session.clone(),
            origin: origin(),
            venue: VenueId::parse(LOCAL).expect("场所写对了"),
            floor: None,
            laid: Laid::Read(&[]),
            source: Source::of(&log, None),
        })
        .expect("起得了后台");
    lanes.settle();
    assert!(
        !built(&scratch.0, "turns.db"),
        "载入时还没接上：后台不读日志、不开库"
    );
    let _lane = lane.started();
    lanes.settle();
    let (index, _) = recall.turns(&room);
    assert_eq!(
        index.keys().expect("列得出"),
        vec![format!("{session}/2")],
        "接上以后照日志补上"
    );
}

/// `dir` 下面有没有叫 `name` 的文件。
fn built(dir: &std::path::Path, name: &str) -> bool {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .any(|entry| {
            let path = entry.path();
            if path.is_dir() {
                built(&path, name)
            } else {
                entry.file_name() == name
            }
        })
}

/// 一轮四条：人说的、开始、她答的、结束，第 `seq` 条起，回合编号是开始那一条。
fn turn(seq: u64, said: &str) -> Vec<Event> {
    let turn = Some(seq + 1);
    vec![
        line(
            seq,
            "message.user",
            None,
            ALICE,
            &format!(r#"{{"blocks":[{{"type":"text","text":"{said}"}}]}}"#),
        ),
        line(
            seq + 1,
            "turn.started",
            turn,
            KERNEL,
            &format!(r#"{{"trigger":{seq}}}"#),
        ),
        line(
            seq + 2,
            "message.assistant",
            turn,
            MODEL,
            &format!(
                r#"{{"blocks":[{{"type":"text","text":"好。"}}],"seen":{}}}"#,
                seq + 1
            ),
        ),
        line(
            seq + 3,
            "turn.ended",
            turn,
            KERNEL,
            r#"{"reason":"completed"}"#,
        ),
    ]
}

/// 撤销了一轮，落盘回来时回合库里已经埋上（施工 R-32）：后台正忙着接一个大会话，撤销照样等它写完才回，回来马上照出处判
/// 那一轮已经不活了。原来不等，Windows CI 上刚撤了马上搜，那一轮记下的还搜得到。
#[test]
fn an_undone_turn_is_buried_by_the_time_the_batch_lands() {
    let (_scratch, root) = root("undo");
    let recall = Arc::new(RecallIndexes::new(&root));
    let lanes = Lanes::default();
    let venue = VenueId::parse(LOCAL).expect("场所写对了");
    let opening = |session: &SessionId, log: &SessionLog| Opening {
        recall: Arc::clone(&recall),
        room: Room::session(session),
        session: session.clone(),
        origin: origin(),
        venue: venue.clone(),
        floor: None,
        laid: Laid::Read(&[]),
        source: Source::of(log, None),
    };

    let small = SessionId::parse("01a0f233-cfec-7023-8ed5-2a037a1d5ecb").expect("编号写对了");
    let mut log = SessionLog::create(&root.session_dir(&small), SEGMENT_LIMIT).expect("建得了日志");
    log.append(&turn(1, "我养了一只猫")).expect("写得进");
    let lane = lanes
        .open(opening(&small, &log))
        .expect("起得了后台")
        .started();
    lanes.settle();
    let source: miyu_recall::Source =
        serde_json::from_str(&format!(r#"{{"session":"{small}","turn":2}}"#)).expect("写对了");
    let room = Room::session(&small);
    assert!(
        recall.alive(&room, &source).expect("读得了"),
        "撤销以前那一轮活着"
    );

    // 后台先去接一个大会话：照日志整份铺，要一会儿。
    let big = SessionId::parse("01a0f233-cfec-7023-8ed5-2a037a1d5ecc").expect("编号写对了");
    let mut other = SessionLog::create(&root.session_dir(&big), SEGMENT_LIMIT).expect("建得了日志");
    let lines: Vec<Event> = (0..4000u64)
        .flat_map(|n| turn(1 + n * 4, &format!("第 {n} 件事：周末去爬山，带上水和伞")))
        .collect();
    other.append(&lines).expect("写得进");
    let _busy = lanes
        .open(opening(&big, &other))
        .expect("起得了后台")
        .started();

    let mut store = crate::store::Indexed::new(log, &small, None, None, Some(lane));
    crate::store::Store::append(
        &mut store,
        &[line(5, "turn.reverted", None, ALICE, r#"{"turns":[2]}"#)],
    )
    .expect("写得进");
    assert!(
        !recall.alive(&room, &source).expect("读得了"),
        "撤销落盘回来，那一轮已经埋上"
    );
}

/// 本机的主会话（施工 E-6）。
fn origin() -> Origin {
    Origin { main: true }
}
