//! 撤销、恢复不再改回文件（施工 U-1，2026-10-15 项目主人定：改回文件交给 git 这类程序），会话里用真的工具：改过的文件
//! 撤销、恢复以后原样不动，日志里没有 `files.restored`；她看过的照有效历史走，撤掉的那一轮读过的要重读。以前的日志里真有
//! `files.restored` 的（内核不认识这一种了，施工 AUD-2），照常载得进、接着用。

use std::path::Path;

use miyu_kernel::event::{Body, Level, Permission, Said, ToolResult, ToolStatus};
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
            level: Level::Full,
            read_only: false,
        },
        attended: false,
        cwd: home.scratch.0.join("work").to_string_lossy().into_owned(),
        dirs: Vec::new(),
        sandbox: None,
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
            .any(|event| event.body.kind() == "files.restored"),
        "日志里没有改回文件的那一条"
    );
}

/// 以前的日志里带着 `files.restored` 的（施工 4-7 上到 U-1 之间，撤销过改了文件的回合会记一条）：这一种内核不认识了
/// （施工 AUD-2）。这样的日志照常载得进：那一条原样留在日志里，载入时跳过；那一次撤销照样能恢复，接着说话照常答。
#[tokio::test]
async fn an_old_log_with_a_files_restored_line_still_loads_and_goes_on() {
    let home = Home::outside_temp();
    put(&home, "a.txt", "old\n");
    let mut plays = edit_a();
    plays.push(Play::Says("好。"));
    let script = Script::new(plays);
    let handle = home
        .create_as(&script, &base_system(), opening(&home))
        .await;
    talk(&handle, "cmd-1", "改一下").await;
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    let id = handle.id().clone();
    stop(&handle).await;

    // 照 U-1 以前的核心记的样子，在撤销那一条后面接一条改回文件的结局：序号接着数，时刻、命令照撤销的那一条。
    let log = home.log(&id);
    let reverted = log.last().expect("有日志");
    assert!(
        matches!(reverted.body, Body::TurnReverted(_)),
        "{reverted:?}"
    );
    let result = log
        .iter()
        .find(|event| matches!(event.body, Body::ToolResult(_)))
        .expect("有工具结果")
        .seq
        .get();
    let segment = std::fs::read_dir(home.root.session_dir(&id))
        .expect("读得了")
        .map(|entry| entry.expect("读得了").path())
        .find(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
        .expect("有一段日志");
    let text = std::fs::read_to_string(&segment).expect("读得回来");
    let last: serde_json::Value =
        serde_json::from_str(text.lines().last().expect("有一行")).expect("是 JSON");
    let line = format!(
        r#"{{"seq":{},"at":{},"kind":"files.restored","by":{{"kind":"person"}},"cause":"cmd-2","body":{{"files":[{{"result":{result},"effect":0,"path":"/w/a.txt","action":"write","outcome":"restored"}}]}}}}"#,
        reverted.seq.get() + 1,
        last["at"],
    );
    std::fs::write(&segment, format!("{text}{line}\n")).expect("写得进");

    let cwd = opening(&home).cwd;
    let loaded = home.load_at(&id, &script, &base_system(), &cwd).await;
    let kept = home.log(&id);
    let old = kept
        .iter()
        .find(|event| event.body.kind() == "files.restored")
        .expect("那一条原样留在日志里");
    assert!(matches!(old.body, Body::Unknown { .. }), "{old:?}");
    undo(&loaded, "cmd-3", Command::Unrevert).await;
    talk(&loaded, "cmd-4", "接着说").await;
    let ended = home
        .log(&id)
        .iter()
        .filter(|event| matches!(event.body, Body::TurnEnded(_)))
        .count();
    assert_eq!(ended, 2, "载入以后那一轮照常走完");
    assert_eq!(read(&home, "a.txt").as_deref(), Some("new\n"), "文件没人动");
    stop(&loaded).await;
}

/// 撤掉的那一轮里读过的、写过的不算她看过：接着写当没看过照写（施工 ED-2），不会拿撤掉的那一轮看到的去比、说改过了。
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
        Play::calls(&[("write", r#"{"file_path":"a.txt","content":"newer\n"}"#)]),
        Play::Says("好。"),
    ]);
    let handle = home
        .create_as(&script, &base_system(), opening(&home))
        .await;
    talk(&handle, "cmd-1", "改一下").await;
    let turn = last_turn(&home, &handle);
    undo(&handle, "cmd-2", Command::Revert { turn: Some(turn) }).await;
    // 撤销不改回文件（施工 U-1）；再被别人改一下：撤掉的那一轮看到的要是还算，就对不上、不写了。
    std::fs::write(&file, "changed\n").expect("写得进");
    talk(&handle, "cmd-3", "再改一下").await;
    let last = results(&home, &handle).pop().expect("有结果");
    assert_eq!(last.status, ToolStatus::Ok, "{last:?}");
    assert_eq!(std::fs::read(&file).expect("读得出"), b"newer\n");
}

/// 恢复以后，那一轮里读过的、写过的又算她看过：拿「又被改了」看（施工 ED-2 起没看过的照写），记着的拦下。
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
    put(&home, "a.txt", "changed\n");
    talk(&handle, "cmd-4", "再改一下").await;
    let last = results(&home, &handle).pop().expect("有结果");
    assert_eq!(
        last.human,
        Some(Said::new("software/basesystem/common/stale"))
    );
    assert_eq!(read(&home, "a.txt").as_deref(), Some("changed\n"), "没写");
}
