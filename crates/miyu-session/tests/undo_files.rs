//! 撤销、恢复不再改回文件（施工 U-1，2026-10-15 项目主人定：改回文件交给 git 这类程序），会话里用真的工具：改过的文件
//! 撤销、恢复以后原样不动，日志里没有 `files.restored`；她看过的照有效历史走，撤掉的那一轮读过的要重读。

use std::path::Path;

use miyu_kernel::event::{Body, Level, Permission, Said, ToolResult};
use miyu_kernel::id::TurnId;
use miyu_kernel::session::{Command, Outcome};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

fn base_system() -> Catalog {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Catalog::new(miyu_basesystem::tools(&resources).expect("出厂的资源读得出来")).expect("合写法")
}

/// 在场地的 `work/` 里干活，工作区这一级，没人能确认。
fn opening(home: &Home) -> Opening {
    Opening {
        permission: Permission {
            level: Level::Workspace,
            read_only: false,
        },
        attended: false,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
        sandbox_cache: None,
    }
}

/// 说一句，等这一轮结束。
async fn talk(handle: &miyu_session::Handle, id: &str, words: &str) {
    let mut pushes = watch(handle).await;
    ask(handle, id, say(words)).await.expect("会话在跑");
    until_turn_ends(&mut pushes).await;
}

/// 日志里最后一轮的编号。
fn last_turn(home: &Home, handle: &miyu_session::Handle) -> TurnId {
    home.log(handle.id())
        .iter()
        .rev()
        .find(|event| matches!(event.body, Body::TurnStarted(_)))
        .map(|event| TurnId::new(event.seq))
        .expect("开过一轮")
}

/// 日志里的工具结果。
fn results(home: &Home, handle: &miyu_session::Handle) -> Vec<ToolResult> {
    home.log(handle.id())
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .collect()
}

/// 场地 `work/` 里的一个文件的内容；没有的是空的。
fn read(home: &Home, name: &str) -> Option<String> {
    std::fs::read_to_string(home.scratch.0.join("work").join(name)).ok()
}

/// 在场地的 `work/` 里写一个文件，像是别人写的。
fn put(home: &Home, name: &str, text: &str) {
    std::fs::write(home.scratch.0.join("work").join(name), text).expect("写得进");
}

/// 她照 `plays` 走一轮：造会话，说一句，等这一轮结束。
async fn one_turn(home: &Home, plays: Vec<Play>) -> miyu_session::Handle {
    let script = Script::new(plays);
    let handle = home.create_as(&script, &base_system(), opening(home)).await;
    talk(&handle, "cmd-1", "改一下").await;
    handle
}

/// 读 `a.txt`，再把它写成 `new\n`。
fn edit_a() -> Vec<Play> {
    vec![
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("write", r#"{"file_path":"a.txt","content":"new\n"}"#)]),
        Play::Says("改好了。"),
    ]
}

/// 发撤销或者恢复，要接受，只产生那一条事件：文件不改回。
async fn undo(handle: &miyu_session::Handle, id: &str, command: Command) {
    match ask(handle, id, command).await.expect("会话在跑") {
        Outcome::Accepted { events } => assert_eq!(events.len(), 1, "{events:?}"),
        other => panic!("{other:?}"),
    }
}

#[tokio::test]
async fn an_edit_stays_as_it_is_after_undo_and_redo() {
    let home = Home::outside_temp();
    put(&home, "a.txt", "old\n");
    let handle = one_turn(&home, edit_a()).await;
    assert_eq!(read(&home, "a.txt").as_deref(), Some("new\n"));
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    assert_eq!(read(&home, "a.txt").as_deref(), Some("new\n"), "撤销不改回");
    undo(&handle, "cmd-3", Command::Unrevert).await;
    assert_eq!(read(&home, "a.txt").as_deref(), Some("new\n"), "恢复也不动");
    assert!(
        !home
            .log(handle.id())
            .iter()
            .any(|event| matches!(event.body, Body::FilesRestored(_))),
        "日志里没有改回文件的那一条"
    );
}

/// 撤掉的那一轮里读过的不算她看过：接着改要先读，文件照旧是撤销前的样子。
#[tokio::test]
async fn what_she_read_in_an_undone_turn_no_longer_counts() {
    let home = Home::outside_temp();
    let file = home.scratch.0.join("work/a.txt");
    std::fs::write(&file, "old\n").expect("写得进");
    let write = r#"{"file_path":"a.txt","content":"new\n"}"#;
    let script = Script::new([
        Play::calls(&[("read", r#"{"file_path":"a.txt"}"#)]),
        Play::calls(&[("write", write)]),
        Play::Says("改好了。"),
        Play::calls(&[("write", write)]),
        Play::Says("好。"),
    ]);
    let handle = home
        .create_as(&script, &base_system(), opening(&home))
        .await;
    talk(&handle, "cmd-1", "改一下").await;
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    talk(&handle, "cmd-3", "再改一下").await;
    let last = results(&home, &handle).pop().expect("有结果");
    assert_eq!(
        last.human,
        Some(Said::new("software/basesystem/common/not-read"))
    );
    assert_eq!(
        std::fs::read(&file).expect("读得出"),
        b"new\n",
        "撤销没改回、这一次也没写"
    );
}

/// 恢复以后，那一轮里读过的又算她看过：接着改用不着重读。
#[tokio::test]
async fn after_a_redo_what_she_read_counts_again() {
    let home = Home::outside_temp();
    put(&home, "a.txt", "old\n");
    let mut plays = edit_a();
    plays.push(Play::calls(&[(
        "write",
        r#"{"file_path":"a.txt","content":"newer\n"}"#,
    )]));
    plays.push(Play::Says("好。"));
    let handle = one_turn(&home, plays).await;
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    undo(&handle, "cmd-3", Command::Unrevert).await;
    talk(&handle, "cmd-4", "再改一下").await;
    assert_eq!(read(&home, "a.txt").as_deref(), Some("newer\n"));
}
