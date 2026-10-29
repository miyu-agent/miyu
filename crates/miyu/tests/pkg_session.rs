//! 会话里的 `miyu pkg`（施工 BS-1 中下，设计 `44-基础系统拆包.md` 第三节第 4 条）：真的 `miyu core`，模型是本机回环上的假服务器，
//! 调一次 `shell`。命令里经 `PATH` 最前面的 `miyu` 跑 `miyu pkg`，看见 `MIYU_CALL_SOCK` 就经通道调 `packages`、`package`，
//! 不连核心的协议；会话里用不了的子命令退出码 2。每次调用记一条 `tool.nested`，挂在那次 `shell` 下面。搭法同 `call.rs`。

use serde_json::{Value, json};

use crate::call::{enter, finish, log, miyu, read, run, until_done};
use crate::support::Home;

#[tokio::test]
async fn miyu_pkg_in_a_session_goes_through_the_channel() {
    let home = Home::new();
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let mut script = enter(&work);
    script.push_str(&miyu("list", "pkg list"));
    script.push_str(&miyu("info", "pkg info miyu-base-shell"));
    script.push_str(&miyu("files", "pkg files miyu-base-shell"));
    script.push_str(finish());
    let args = json!({"command": script, "description": "看看装了什么"});
    let (mut core, session, _server) = run(&home, &work, &args).await;
    until_done(&home, &session, &work).await;

    assert_eq!(read(&work, "list.code"), "0", "{}", read(&work, "list.err"));
    assert!(
        read(&work, "list.out")
            .lines()
            .any(|line| line.starts_with("miyu-base-shell: Commands.")),
        "列出来的是 packages 那一件的字：{}",
        read(&work, "list.out")
    );
    assert_eq!(read(&work, "info.code"), "0", "{}", read(&work, "info.err"));
    let info: Value = serde_json::from_str(&read(&work, "info.out")).expect("一行 JSON");
    assert_eq!(info["package"], "miyu-base-shell", "{info}");
    assert_eq!(read(&work, "files.code"), "2");
    assert_eq!(
        read(&work, "files.err"),
        "miyu pkg files does not work inside a Miyu session."
    );

    let nested: Vec<Value> = log(&home, &session)
        .into_iter()
        .filter(|event| event["kind"] == "tool.nested")
        .collect();
    assert_eq!(
        nested
            .iter()
            .map(|event| (
                event["body"]["name"].clone(),
                event["body"]["status"].clone()
            ))
            .collect::<Vec<_>>(),
        [
            (json!("packages"), json!("ok")),
            (json!("packages"), json!("ok"))
        ],
        "用不了的那一次没交给核心"
    );
    core.kill().expect("停得了");
    core.wait().expect("收得了");
}
