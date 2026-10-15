//! 撤销、恢复的回应里给人看的几样（施工 4-7 下）：会话的工作目录、那一轮人说的话、撤了几轮。施工 U-1 起撤销不再改回
//! 文件（2026-10-15 项目主人定）：回应里没有 `files`、`commands`，文件原样不动，只产生撤销（恢复）那一条。

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

/// 基础系统的工具，加一件假的执行命令的 `run`。
fn tools() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let mut tools = miyu_basesystem::tools(&resources).expect("出厂的资源读得出来");
    let run: Arc<dyn Tool> = Fake::new("run", Access::Execute, Act::Echo);
    tools.push(run);
    Catalog::new(tools).expect("合写法")
}

/// 在场地的工作目录里造会话，她照 `plays` 走一轮：把 `a.txt` 从 `old` 改成 `new`。交回客户端和会话。
async fn edited(home: &Home, plays: Vec<Play>) -> (Client, String) {
    std::fs::write(home.work.join("a.txt"), "old\n").expect("写得进");
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "改一下").await;
    home.until_turns(&session, 1).await;
    (client, session)
}

/// 读 `a.txt`，改成 `new`，顺手执行一条命令。
fn edit_and_run() -> Vec<Play> {
    vec![
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[
            ("write", r#"{"file_path":"a.txt","content":"new\n"}"#),
            ("run", "{}"),
        ]),
        Play::Says("改好了。"),
    ]
}

/// 给人看的路径：Windows 上去掉 `\\?\` 这个前缀，和回应里写的一样。
fn plain(path: &Path) -> String {
    let text = path.to_string_lossy();
    text.strip_prefix(r"\\?\").unwrap_or(&text).to_string()
}

/// 回应里的 `result`。
fn result_of(reply: &Value) -> &Value {
    assert!(reply.get("error").is_none(), "{reply}");
    &reply["result"]
}

/// 撤最后一轮：回应里写着会话的工作目录、那一轮人说的话、撤了几轮；没有 `files`、`commands`，文件不动。恢复同样。
#[tokio::test]
async fn an_undo_says_which_turn_and_leaves_the_file_alone() {
    let home = Home::new();
    let (mut client, session) = edited(&home, edit_and_run()).await;
    let reply = client
        .call("c3", "session.revert", json!({"session": session}))
        .await;
    let result = result_of(&reply);
    assert_eq!(
        result["events"].as_array().map(Vec::len),
        Some(1),
        "只有撤销那一条：{result}"
    );
    assert_eq!(result["cwd"], json!(plain(&home.work)));
    assert_eq!(result["turns"], json!(1));
    assert_eq!(result["said"], json!("改一下"));
    for gone in ["files", "commands", "compactions"] {
        assert!(result.get(gone).is_none(), "没有 {gone}：{result}");
    }
    assert_eq!(
        std::fs::read_to_string(home.work.join("a.txt")).expect("在"),
        "new\n",
        "文件不动"
    );
    let reply = client
        .call("c4", "session.unrevert", json!({"session": session}))
        .await;
    let result = result_of(&reply);
    assert_eq!(
        result["events"].as_array().map(Vec::len),
        Some(1),
        "{result}"
    );
    assert_eq!(result["said"], json!("改一下"));
    assert!(result.get("files").is_none(), "{result}");
    assert_eq!(
        std::fs::read_to_string(home.work.join("a.txt")).expect("在"),
        "new\n"
    );
}

/// 写一次 `a.txt`，内容是 `content`。
fn write(content: &str) -> Play {
    Play::calls(&[(
        "write",
        &json!({"file_path": "a.txt", "content": content}).to_string(),
    )])
}

/// 撤最后一轮，交回回应里的 `result`。
async fn undo(client: &mut Client, id: &str, session: &str) -> Value {
    let reply = client
        .call(id, "session.revert", json!({"session": session}))
        .await;
    result_of(&reply).clone()
}

/// 撤最后一轮：说的是第二轮那句话的第一行（前后的空白去掉）。
#[tokio::test]
async fn the_words_of_the_undone_turn_are_said() {
    let home = Home::new();
    std::fs::write(home.work.join("a.txt"), "old\n").expect("写得进");
    let plays = vec![
        Play::calls(&[("run", "{}")]),
        Play::Says("跑完了。"),
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        write("new\n"),
        Play::Says("改好了。"),
    ];
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "跑一下").await;
    home.until_turns(&session, 1).await;
    client.say("c3", &session, "  改一下  \n顺便看看").await;
    home.until_turns(&session, 2).await;
    let reply = client
        .call("c4", "session.revert", json!({"session": session}))
        .await;
    assert_eq!(result_of(&reply)["said"], json!("改一下"), "{reply}");
}

/// 会话的工作目录是一个链接：回应里写真实的位置，头照它写相对的路径才对得上。
#[cfg(unix)]
#[tokio::test]
async fn the_working_directory_is_given_as_its_real_place() {
    let home = Home::new();
    std::fs::write(home.work.join("a.txt"), "old\n").expect("写得进");
    let link = home.work.with_extension("link");
    std::os::unix::fs::symlink(&home.work, &link).expect("建得了链接");
    let script = Script::new(edit_and_run());
    let mut client = Client::connect(home.core_with_tools(&script, tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &link.to_string_lossy()).await;
    client.say("c2", &session, "改一下").await;
    home.until_turns(&session, 1).await;
    let result = undo(&mut client, "c3", &session).await;
    std::fs::remove_file(&link).expect("删得掉");
    assert_eq!(result["cwd"], json!(plain(&home.work)), "{result}");
}

/// 人说的全是空白：没有 `said`（施工 4-9 再补一：以前是空字符串）。
#[tokio::test]
async fn blank_words_give_no_said() {
    let home = Home::new();
    std::fs::write(home.work.join("a.txt"), "old\n").expect("写得进");
    let plays = vec![
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        write("new\n"),
        Play::Says("改好了。"),
    ];
    let mut client = Client::connect(home.core_with_tools(&Script::new(plays), tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "  \n\t\n ").await;
    home.until_turns(&session, 1).await;
    let result = undo(&mut client, "c3", &session).await;
    assert!(result.get("said").is_none(), "{result}");
    assert_eq!(result["turns"], json!(1), "{result}");
}
