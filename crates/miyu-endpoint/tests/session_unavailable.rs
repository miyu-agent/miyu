//! 读不了的会话照原因分开拒（施工 YC-2，压测报告 F1 连带）：会话目录这一刻读不了（权限、打开文件数用光这类读写出错）的
//! `session_unavailable`，日志真读不懂的照旧 `session_broken`。原来一律说成损坏，人以为对话没了。只在 Unix 上（照权限挡读）。

use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

use serde_json::json;

use crate::support::{Client, Home, reason};
use miyu_session::testkit::{Play, Script};

/// 造一个会话，说完一轮，交回会话编号和它的目录。
async fn one_turn(home: &Home, client: &mut Client) -> (String, PathBuf) {
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("c2", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let id = miyu_kernel::id::SessionId::parse(&session).expect("合写法");
    (session, home.root.session_dir(&id))
}

/// 把 `dir` 的权限换成 `mode`。
fn chmod(dir: &PathBuf, mode: u32) {
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(mode)).expect("改得了权限");
}

#[tokio::test]
async fn a_log_that_cannot_be_read_right_now_is_unavailable_not_broken() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    let (session, dir) = one_turn(&home, &mut client).await;
    chmod(&dir, 0o000);
    let reply = client
        .call("v1", "view.page", json!({"session": session, "view": true}))
        .await;
    chmod(&dir, 0o700);
    assert_eq!(reason(&reply), Some("session_unavailable"), "{reply}");
    let reply = client
        .call("v2", "view.page", json!({"session": session, "view": true}))
        .await;
    assert!(reply.get("error").is_none(), "权限还回来就照常读：{reply}");
}

#[tokio::test]
async fn a_log_that_does_not_parse_is_still_broken() {
    use std::io::{Seek, SeekFrom, Write};

    let home = Home::new();
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    let (session, dir) = one_turn(&home, &mut client).await;
    let segment = std::fs::read_dir(&dir)
        .expect("会话目录在")
        .map(|entry| entry.expect("读得了").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .min()
        .expect("有一段日志");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .open(&segment)
        .expect("开得了");
    file.seek(SeekFrom::Start(0)).expect("挪得了");
    file.write_all(b"x").expect("写得进");
    drop(file);
    let reply = client
        .call("v1", "view.page", json!({"session": session, "view": true}))
        .await;
    assert_eq!(reason(&reply), Some("session_broken"), "{reply}");
}
