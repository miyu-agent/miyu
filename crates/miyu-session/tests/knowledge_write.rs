//! 她写知识库（施工 K-3，`docs/blueprint/knowledge.md`「怎么走」第七条，19 N14）：真知识库的目录、真会话，执行器替身照剧本
//! 调工具，数据根在临时目录。一个库都没有时只有 `kb_save`，写到还没有的库就是建它，下一个回合五件都有；新建、追加、改照
//! 文件对，撤销那一轮文件不动（施工 U-1：撤销不再改回文件）；人格收窄的写不了没列的、建不了新库；挪进回收站、撤销不移回来
//! （只在 Linux 上跑：别的平台删进的是系统真的回收站）。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::TurnId;
use miyu_kernel::request::Request;
use miyu_kernel::session::Command;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 基础系统、记忆加知识库的五件，照包登记，和核心里一样。
fn catalog(home: &Home) -> Catalog {
    let resources = home.resources.path();
    Catalog::in_packages(vec![
        (
            miyu_tool::BASESYSTEM,
            miyu_basesystem::tools(resources).expect("读得出"),
        ),
        (
            miyu_memory::PACKAGE,
            miyu_memory::tools(resources).expect("读得出"),
        ),
        (
            miyu_knowledge::PACKAGE,
            miyu_knowledge::tools(resources).expect("读得出"),
        ),
    ])
    .expect("合写法")
}

async fn session(home: &Home, script: &Script) -> Handle {
    home.create_full(script, &catalog(home), Opening::default(), Lines::default())
        .await
}

/// 说 `words`，等到结束了 `turns` 轮，交回日志。
async fn chat(home: &Home, handle: &Handle, turns: usize, words: &str) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: None,
    };
    within(
        "回应",
        handle.command(id(&format!("cmd-{turns}")), alice(), said),
    )
    .await
    .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= turns
    })
    .await
}

/// 日志里最后一次调用的结果。
fn last_result(log: &[Event]) -> String {
    let result = log
        .iter()
        .rev()
        .find_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result),
            _ => None,
        })
        .expect("有一次调用");
    match result.blocks.as_slice() {
        [Block::Text(Text { text })] => text.clone(),
        other => panic!("一段字：{other:?}"),
    }
}

/// 日志里最后一个回合。
fn last_turn(log: &[Event]) -> TurnId {
    log.iter()
        .rev()
        .find_map(|event| match event.body {
            Body::TurnStarted(_) => event.turn,
            _ => None,
        })
        .expect("有回合")
}

fn names(request: &Request) -> Vec<&str> {
    let mut names: Vec<&str> = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .filter(|name| name.starts_with("kb_"))
        .collect();
    names.sort_unstable();
    names
}

fn call(name: &str, args: serde_json::Value) -> Play {
    Play::calls(&[(name, &args.to_string())])
}

fn kb(home: &Home) -> std::path::PathBuf {
    home.root.path().join("kb")
}

fn read(path: &std::path::Path) -> String {
    std::fs::read_to_string(path).expect("读得出")
}

#[tokio::test]
async fn with_no_base_only_save_is_given_and_saving_makes_the_first() {
    let home = Home::new();
    // 施工 K-3 补（R-9 真跑查出来的）：编号写成中文的，说清编号怎么写，她下一次照着改；什么都不建。
    let script = Script::new([
        call(
            "kb_save",
            serde_json::json!({"kb": "菜谱", "path": "pork.md", "text": "红烧肉炖一小时"}),
        ),
        call(
            "kb_save",
            serde_json::json!({"kb": "recipes", "path": "pork.md", "text": "红烧肉炖一小时", "about": "家常菜"}),
        ),
        Play::Says("记好了。"),
        Play::Says("嗯。"),
    ]);
    let handle = session(&home, &script).await;
    let log = chat(&home, &handle, 1, "记一下红烧肉").await;
    let results: Vec<String> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => match result.blocks.as_slice() {
                [Block::Text(Text { text })] => Some(text.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    assert_eq!(
        results,
        [
            "菜谱 is not a usable knowledge base id. Use lowercase letters, digits, - and _, starting with a letter.",
            "Created knowledge base recipes and saved pork.md.",
        ]
    );
    assert!(
        !kb(&home).join("菜谱").exists(),
        "编号写错的那一次什么都没建"
    );
    assert_eq!(read(&kb(&home).join("recipes/pork.md")), "红烧肉炖一小时\n");
    assert_eq!(
        read(&kb(&home).join("recipes/kb.toml")),
        "description = \"家常菜\"\n"
    );
    chat(&home, &handle, 2, "好").await;
    let requests = script.requests();
    assert_eq!(
        names(&requests[0].1),
        ["kb_save"],
        "一个库都没有：只有新建那一件"
    );
    assert_eq!(
        names(&requests[3].1),
        ["kb_delete", "kb_edit", "kb_read", "kb_save", "kb_search"],
        "有了库，下一个回合五件都有"
    );
}

#[tokio::test]
async fn saving_appending_and_editing_follow_the_file_and_undo_leaves_it() {
    let home = Home::new();
    let file = kb(&home).join("notes/a.md");
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
    std::fs::write(&file, "猫\n").expect("写得进");
    let script = Script::new([
        call(
            "kb_save",
            serde_json::json!({"kb": "notes", "path": "a.md", "text": "狗", "append": true}),
        ),
        Play::Says("接上了。"),
        call(
            "kb_edit",
            serde_json::json!({"kb": "notes", "path": "a.md", "old": "狗", "new": "兔"}),
        ),
        Play::Says("改好了。"),
        call(
            "kb_save",
            serde_json::json!({"kb": "notes", "path": "a.md", "text": "又一份"}),
        ),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script).await;
    assert_eq!(
        last_result(&chat(&home, &handle, 1, "加一句").await),
        "Appended to a.md in notes."
    );
    assert_eq!(read(&file), "猫\n狗\n");
    let log = chat(&home, &handle, 2, "改一句").await;
    assert_eq!(last_result(&log), "Edited a.md in notes.");
    assert_eq!(read(&file), "猫\n兔\n");
    let edited = last_turn(&log);
    assert_eq!(
        last_result(&chat(&home, &handle, 3, "再新建").await),
        "a.md already exists in notes. Use append, or kb_edit."
    );
    ask(&handle, "undo", Command::Revert { turn: Some(edited) })
        .await
        .expect("会话在跑");
    assert_eq!(
        read(&file),
        "猫\n兔\n",
        "撤销改的那一轮：文件不动（施工 U-1）"
    );
}

#[tokio::test]
async fn a_narrowed_persona_writes_only_its_own_and_makes_none() {
    let home = Home::new();
    for name in ["notes", "secret"] {
        let dir = kb(&home).join(name);
        std::fs::create_dir_all(&dir).expect("建得了");
        std::fs::write(dir.join("a.md"), "字\n").expect("写得进");
    }
    let persona = home.root.path().join("personas/engineer/persona.toml");
    std::fs::write(persona, "[knowledge]\nbases = [\"notes\"]\n").expect("写得进");
    let script = Script::new([
        call(
            "kb_save",
            serde_json::json!({"kb": "secret", "path": "b.md", "text": "x"}),
        ),
        Play::Says("好。"),
        call(
            "kb_save",
            serde_json::json!({"kb": "fresh", "path": "b.md", "text": "x"}),
        ),
        Play::Says("好。"),
        call(
            "kb_edit",
            serde_json::json!({"kb": "secret", "path": "a.md", "old": "字", "new": "x"}),
        ),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script).await;
    for (turn, words) in [(1, "写秘密"), (2, "建新库"), (3, "改秘密")] {
        let result = last_result(&chat(&home, &handle, turn, words).await);
        assert!(
            result.starts_with("There is no knowledge base "),
            "{result}"
        );
    }
    assert!(!kb(&home).join("secret/b.md").exists());
    assert!(!kb(&home).join("fresh").exists(), "收窄的建不了新库");
    assert_eq!(read(&kb(&home).join("secret/a.md")), "字\n");
}

#[tokio::test]
async fn a_persona_narrowed_to_nothing_it_can_read_gets_no_save_either() {
    // 施工 K-3 修（验收手册待定 13，2026-10-15 项目主人定写也照收窄）：收窄了的建不了库，查不到一个的（收窄成空的、列着的
    // 一个都还没有）给了 `kb_save` 也写不进，不给；列着的库有了，下一个回合五件都有。没收窄的照旧（一个库都没有也给它）。
    let home = Home::new();
    let dir = kb(&home).join("secret");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("a.md"), "字\n").expect("写得进");
    let persona = home.root.path().join("personas/engineer/persona.toml");
    std::fs::write(&persona, "[knowledge]\nbases = []\n").expect("写得进");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。"), Play::Says("哦。")]);
    let handle = session(&home, &script).await;
    chat(&home, &handle, 1, "收窄成空的").await;
    std::fs::write(&persona, "[knowledge]\nbases = [\"notes\"]\n").expect("写得进");
    chat(&home, &handle, 2, "列着的还没有").await;
    let notes = kb(&home).join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("b.md"), "字\n").expect("写得进");
    chat(&home, &handle, 3, "列着的有了").await;
    let requests = script.requests();
    assert!(
        names(&requests[0].1).is_empty(),
        "{:?}",
        names(&requests[0].1)
    );
    assert!(
        names(&requests[1].1).is_empty(),
        "{:?}",
        names(&requests[1].1)
    );
    assert_eq!(
        names(&requests[2].1),
        ["kb_delete", "kb_edit", "kb_read", "kb_save", "kb_search"]
    );
}

#[cfg(target_os = "linux")]
#[tokio::test]
async fn a_document_goes_to_the_trash_and_undo_leaves_it_there() {
    let home = Home::outside_temp();
    let file = kb(&home).join("notes/old.md");
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
    std::fs::write(&file, "过时了\n").expect("写得进");
    let script = Script::new([
        call(
            "kb_delete",
            serde_json::json!({"kb": "notes", "path": "old.md"}),
        ),
        Play::Says("删了。"),
    ]);
    let handle = session(&home, &script).await;
    let log = chat(&home, &handle, 1, "删掉过时的").await;
    assert_eq!(last_result(&log), "Moved old.md in notes to the trash.");
    assert!(!file.exists());
    ask(
        &handle,
        "undo",
        Command::Revert {
            turn: Some(last_turn(&log)),
        },
    )
    .await
    .expect("会话在跑");
    assert!(!file.exists(), "撤销那一轮：不从回收站移回来（施工 U-1）");
}

#[tokio::test]
async fn a_mounted_folder_is_searched_and_read_but_never_written() {
    // 施工 K-4：挂进来的照搜、照读；写的三件拒（只读，19 N7），挂的目录、库目录里什么都没动。
    let home = Home::new();
    let notes = home.home.join("notes");
    std::fs::create_dir_all(&notes).expect("建得了");
    std::fs::write(notes.join("a.md"), "团子是橘猫。\n").expect("写得进");
    let dir = kb(&home).join("notes");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(
        dir.join("kb.toml"),
        format!("mount = '{}'\n", notes.display()),
    )
    .expect("写得进");
    let edit = serde_json::json!({"kb": "notes", "path": "a.md", "old": "橘猫", "new": "白猫"});
    let script = Script::new([
        call("kb_search", serde_json::json!({"query": "橘猫"})),
        call(
            "kb_read",
            serde_json::json!({"kb": "notes", "path": "a.md"}),
        ),
        call(
            "kb_save",
            serde_json::json!({"kb": "notes", "path": "b.md", "text": "x"}),
        ),
        call("kb_edit", edit),
        call(
            "kb_delete",
            serde_json::json!({"kb": "notes", "path": "a.md"}),
        ),
        Play::Says("好。"),
    ]);
    let handle = session(&home, &script).await;
    let log = chat(&home, &handle, 1, "看看笔记").await;
    let results: Vec<String> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => match result.blocks.as_slice() {
                [Block::Text(Text { text })] => Some(text.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect();
    let read_only = "notes is a mounted folder and read-only. Save to another knowledge base.";
    assert_eq!(
        results,
        [
            "[notes] a.md\n团子是橘猫。",
            "团子是橘猫。\n",
            read_only,
            read_only,
            read_only,
        ]
    );
    assert_eq!(read(&notes.join("a.md")), "团子是橘猫。\n");
    assert!(!notes.join("b.md").exists() && !dir.join("b.md").exists());
}
