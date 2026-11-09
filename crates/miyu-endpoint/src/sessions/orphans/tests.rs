//! 认派到一半的空子会话：顾问会话（施工 ADV-1）不是任务，父会话的日志里本来就没有它的 `job.started`，不算；子代理照算。

use std::collections::BTreeSet;

use miyu_kernel::event::Event;
use miyu_kernel::id::SessionId;
use miyu_store::log::{SEGMENT_LIMIT, SessionLog};
use miyu_store::root::DataRoot;

use super::orphans;
use crate::test_support::temp_root;

const PARENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a10";
const AGENT: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a11";
const ADVISOR: &str = "01a0d75d-2180-7a3c-9e41-5b7d2c8f6a12";

/// 在数据根里放一个子会话 `id`：第一条是父会话造的 `session.created`，`origin` 照给的写。
fn child(root: &DataRoot, id: &str, origin: Option<&str>) {
    let origin = origin.map_or(String::new(), |origin| format!(",\"origin\":\"{origin}\""));
    let line = format!(
        "{{\"seq\":1,\"at\":\"2026-11-10T07:00:00.000Z\",\"kind\":\"session.created\",\"by\":{{\"kind\":\"session\",\"id\":\"{PARENT}\"}},\"cause\":\"{PARENT}/x\",\"body\":{{\"venue\":\"local\",\"policy\":\"sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894\",\"permission\":{{\"level\":\"full\",\"read_only\":false}},\"parent\":\"{PARENT}\",\"depth\":1{origin}}}}}"
    );
    let event = Event::from_line(&line).expect("合写法");
    let dir = root.session_dir(&SessionId::parse(id).expect("合写法"));
    let mut log = SessionLog::create(&dir, SEGMENT_LIMIT).expect("建得了");
    log.append(&[event]).expect("写得进");
}

#[test]
fn an_advisor_session_is_not_an_orphan() {
    let (root, _dir) = temp_root("orphans-advisor");
    child(&root, AGENT, None);
    child(&root, ADVISOR, Some("advisor"));
    let parent = SessionId::parse(PARENT).expect("合写法");
    let found = orphans(&root, &parent, &BTreeSet::new());
    assert_eq!(found, [SessionId::parse(AGENT).expect("合写法")]);
}
