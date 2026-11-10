//! 日志中间坏了一行，只读打开（施工 YC-6，压测报告 F6）：坏行以前的照样翻得到，`view.page` 带上坏在哪（`broken`）；要载入会话的
//! （说话、订阅）拒 `session_read_only`，带着哪一段、第几行；日志一个字节都不动。原来整个会话 `session_broken`，坏行以前的也看不到。
//! 坏法照压测报告：中间一行换成别的、一行写到一半、序号重了一条、末尾多了一整行别的。

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::support::{Client, Home, logged, reason};
use miyu_session::testkit::{Play, Script};

/// 弄坏日志的一种法子：交回改好的几行，和坏的是第几行（从 1 数）。`lines` 是原来的几行，不带换行。
type Breaking = fn(Vec<String>) -> (Vec<String>, usize);

/// 中间一行换成不是 JSON 的。
fn garbage_in_the_middle(mut lines: Vec<String>) -> (Vec<String>, usize) {
    let k = lines.len() / 2;
    lines[k] = "not json at all".to_string();
    (lines, k + 1)
}

/// 中间一行只剩前一半（换行还在）。
fn cut_in_the_middle(mut lines: Vec<String>) -> (Vec<String>, usize) {
    let k = lines.len() / 2;
    let mut half = lines[k].len() / 2;
    while !lines[k].is_char_boundary(half) {
        half -= 1;
    }
    lines[k].truncate(half);
    (lines, k + 1)
}

/// 中间一行抄了一遍：后一行的序号重了。
fn duplicated_seq(mut lines: Vec<String>) -> (Vec<String>, usize) {
    let k = lines.len() / 2;
    let copy = lines[k - 1].clone();
    lines.insert(k, copy);
    (lines, k + 1)
}

/// 最后一行后面多了一整行别的（带换行：不是没写完的半行）。
fn garbage_at_the_end(mut lines: Vec<String>) -> (Vec<String>, usize) {
    lines.push("garbage".to_string());
    let n = lines.len();
    (lines, n)
}

/// 会话 `session` 唯一的那一段日志。
fn segment_of(home: &Home, session: &str) -> PathBuf {
    let id = miyu_kernel::id::SessionId::parse(session).expect("合写法");
    let segments: Vec<PathBuf> = std::fs::read_dir(home.root.session_dir(&id))
        .expect("会话目录在")
        .map(|entry| entry.expect("读得了").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .collect();
    assert_eq!(segments.len(), 1, "{segments:?}");
    segments[0].clone()
}

/// 造一个会话说完三轮，停下全部会话（像核心重启过），照 `breaking` 弄坏它的日志。交回会话编号、坏以前的事件、坏的是第几行、
/// 那一段，和弄坏以后磁盘上的字节。
async fn broken_session(home: &Home, breaking: Breaking) -> (String, Vec<Value>, usize, PathBuf) {
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let first = home.core(&script);
    let mut client = Client::connect(first.clone());
    client.hello().await;
    let session = client.create("c1", "~").await;
    for (turn, text) in ["a", "b", "c"].into_iter().enumerate() {
        client.say(&format!("c{}", turn + 2), &session, text).await;
        home.until_turns(&session, turn + 1).await;
    }
    first.stop_sessions().await;
    drop(client);
    let log = logged(home, &session);
    let segment = segment_of(home, &session);
    let text = std::fs::read_to_string(&segment).expect("读得了");
    let lines: Vec<String> = text.lines().map(str::to_string).collect();
    assert_eq!(lines.len(), log.len());
    let (lines, line) = breaking(lines);
    std::fs::write(&segment, lines.join("\n") + "\n").expect("写得进");
    (session, log, line, segment)
}

/// 磁盘上的字节。
fn bytes(path: &Path) -> Vec<u8> {
    std::fs::read(path).expect("读得了")
}

#[tokio::test]
async fn a_log_broken_midway_shows_what_came_before_and_refuses_to_go_on() {
    let ways: [(&str, Breaking); 4] = [
        ("garbage in the middle", garbage_in_the_middle),
        ("cut in the middle", cut_in_the_middle),
        ("duplicated seq", duplicated_seq),
        ("garbage at the end", garbage_at_the_end),
    ];
    for (way, breaking) in ways {
        let home = Home::new();
        let (session, log, line, segment) = broken_session(&home, breaking).await;
        let name = segment.file_name().unwrap().to_string_lossy().into_owned();
        let on_disk = bytes(&segment);
        let script = Script::new([Play::Says("不该说。")]);
        let mut client = Client::connect(home.core(&script));
        client.hello().await;
        let broken = json!({"segment": name, "line": line});

        // 翻得到坏行以前的，带上坏在哪。
        let page = client
            .call("v1", "view.page", json!({"session": session}))
            .await;
        let result = &page["result"];
        assert_eq!(result["events"], json!(log[..line - 1]), "{way}: {page}");
        assert_eq!(result["broken"], broken, "{way}: {page}");
        assert_eq!(result["more"], json!(false), "{way}: {page}");
        let page = client
            .call("v2", "view.page", json!({"session": session, "view": true}))
            .await;
        assert!(page["result"]["entries"].is_array(), "{way}: {page}");
        assert_eq!(page["result"]["broken"], broken, "{way}: {page}");
        // 不碰坏行的一页照旧，不带 `broken`。
        let page = client
            .call("v3", "view.page", json!({"session": session, "before": 3}))
            .await;
        assert_eq!(page["result"]["events"], json!(log[..2]), "{way}: {page}");
        assert!(page["result"].get("broken").is_none(), "{way}: {page}");

        // 不能接着聊：说话、订阅都拒，说清坏在哪。
        let reply = client.say("c9", &session, "again").await;
        assert_eq!(reason(&reply), Some("session_read_only"), "{way}: {reply}");
        assert_eq!(reply["error"]["data"]["segment"], json!(name), "{way}");
        assert_eq!(reply["error"]["data"]["line"], json!(line), "{way}");
        let said = reply["error"]["message"].as_str().unwrap_or_default();
        assert!(said.contains(&format!("第 {line} 行")), "{way}: {said}");
        let reply = client.subscribe("c10", &session).await;
        assert_eq!(reason(&reply), Some("session_read_only"), "{way}: {reply}");

        // 照样列得出来。
        let listed = client.call("l1", "session.list", json!({})).await;
        assert!(listed.get("error").is_none(), "{way}: {listed}");

        assert_eq!(bytes(&segment), on_disk, "{way}: 日志一个字节都不动");
    }
}

/// 坏在第一行的（连造会话那一条都读不成）：什么都看不到，照旧 `session_broken`。
#[tokio::test]
async fn a_log_broken_at_the_first_line_is_still_broken() {
    fn first_line(mut lines: Vec<String>) -> (Vec<String>, usize) {
        lines[0] = "x".to_string();
        (lines, 1)
    }
    let home = Home::new();
    let (session, _, _, segment) = broken_session(&home, first_line).await;
    let on_disk = bytes(&segment);
    let script = Script::new([Play::Says("不该说。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let page = client
        .call("v1", "view.page", json!({"session": session}))
        .await;
    assert_eq!(reason(&page), Some("session_broken"), "{page}");
    let reply = client.say("c9", &session, "again").await;
    assert_eq!(reason(&reply), Some("session_broken"), "{reply}");
    assert_eq!(bytes(&segment), on_disk);
}
