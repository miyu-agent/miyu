//! 回合索引在后台写（施工 V-9）：接上时自己读日志，后来交来的一批里已经铺过的不再看。

use std::path::PathBuf;
use std::sync::Arc;

use miyu_kernel::event::Event;
use miyu_kernel::id::{AccountId, SessionId, VenueId};
use miyu_store::env::{Env, Platform};
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_store::recall::{RecallIndexes, Room};
use miyu_store::root::DataRoot;

use super::{Laid, Lanes, Opening, Source};
use crate::agents::LOCAL;

const ALICE: &str = r#"{"kind":"person","account":"admin"}"#;
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
    let dir = root.session_dir(&AccountId::parse("admin").expect("账号写对了"), &session);
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
    let room = Room::session(&AccountId::parse("admin").expect("账号写对了"), &session);
    let lanes = Lanes::default();
    let mut lane = lanes
        .open(Opening {
            recall: Arc::clone(&recall),
            room: room.clone(),
            session: session.clone(),
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
    let account = AccountId::parse("admin").expect("账号写对了");
    let mut log = SessionLog::create(&root.session_dir(&account, &session), SEGMENT_LIMIT)
        .expect("建得了日志");
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
    let room = Room::session(&account, &session);
    let lanes = Lanes::default();
    let lane = lanes
        .open(Opening {
            recall: Arc::clone(&recall),
            room: room.clone(),
            session: session.clone(),
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
