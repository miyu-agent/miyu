//! 这个会话日志的只读入口只解要的种类（施工 V-49，`history` 用）：`LogDir::read_range_only` 照存储的 `read_range_only` 读，不要的
//! 种类的正文合 JSON、不合写法的照读（读成不认识的种类），`read_range` 报坏了；要的种类坏了的两样报的一字不差。

use std::fs;
use std::path::{Path, PathBuf};

use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::Seq;
use miyu_tool::ReadLog;

use super::super::LogDir;

/// 四行：造会话、一句话、注入的事实（正文是 `injected`）、一句话（正文是 `said`）。
fn write(name: &str, injected: &str, said: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("miyu-session-only-{name}-{}", std::process::id()));
    drop(fs::remove_dir_all(&dir));
    fs::create_dir_all(&dir).unwrap();
    let line = |seq: u64, kind: &str, body: &str| {
        format!(
            "{{\"seq\":{seq},\"at\":\"2026-11-05T07:00:00.000Z\",\"kind\":\"{kind}\",\"by\":{{\"kind\":\"person\"}},\"body\":{body}}}\n"
        )
    };
    let text = [
        line(
            1,
            "session.created",
            r#"{"venue":"local","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"workspace","read_only":false}}"#,
        ),
        line(
            2,
            "message.user",
            r#"{"blocks":[{"type":"text","text":"a"}]}"#,
        ),
        line(3, "context.injected", injected),
        line(4, "message.user", said),
    ];
    fs::write(dir.join("000000000001.jsonl"), text.concat()).unwrap();
    dir
}

/// 读第一条到最后，交回读到的或者报的错。
fn read(dir: &Path, only: Option<&[&str]>) -> Result<Vec<Event>, String> {
    let log = LogDir(dir.to_path_buf());
    let mut got = Vec::new();
    let last = Seq::new(u64::MAX).unwrap();
    let mut each = |events: Vec<Event>| {
        got.extend(events);
        true
    };
    match only {
        Some(only) => log.read_range_only(Seq::FIRST, last, only, &mut each)?,
        None => log.read_range(Seq::FIRST, last, &mut each)?,
    }
    Ok(got)
}

#[test]
fn the_log_dir_reads_only_the_wanted_kinds() {
    let good = r#"{"blocks":[{"type":"text","text":"b"}]}"#;
    let only: &[&str] = &["tool.result"];
    // 注入的事实正文合 JSON、不合写法：只解要的种类照读，读成不认识的种类；全解报坏了。
    let dir = write("unwanted", "7", good);
    let events = read(&dir, Some(only)).unwrap();
    assert_eq!(events.len(), 4);
    assert!(
        matches!(&events[2].body, Body::Unknown { kind, .. } if kind.as_str() == "context.injected")
    );
    assert!(matches!(&events[3].body, Body::MessageUser(_)));
    assert_eq!(
        read(&dir, None).unwrap_err(),
        "000000000001.jsonl line 3 is broken"
    );
    drop(fs::remove_dir_all(&dir));
    // 人话的正文不合写法、外壳坏了：两样报的一字不差。
    for (name, injected, said) in [
        ("wanted", r#"{"kind":"env","text":"x"}"#, "7"),
        (
            "shell",
            r#"{"kind":"env","text":"x"}"#,
            r#"{"blocks":[]},"body":{}"#,
        ),
    ] {
        let dir = write(name, injected, said);
        let full = read(&dir, None).unwrap_err();
        assert_eq!(read(&dir, Some(only)).unwrap_err(), full, "{name}");
        assert_eq!(full, "000000000001.jsonl line 4 is broken", "{name}");
        drop(fs::remove_dir_all(&dir));
    }
}
