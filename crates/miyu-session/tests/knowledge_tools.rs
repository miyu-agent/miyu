//! 知识库的两件工具接在会话上（施工 K-1 下，`docs/blueprint/knowledge.md`「怎么走」第六条）：真知识库的目录、真会话，执行器
//! 替身照剧本调工具，数据根在临时目录。查得到至少一个库才有两件，建了第一个库下一个回合有、删光了下一个回合没有；群会话里
//! 也有，预设关着的没有；人格收窄的只查得到列着的，收窄成空的没有。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::id::VenueId;
use miyu_kernel::request::Request;
use miyu_kernel::session::Command;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 基础系统、记忆加知识库的两件，照包登记，和核心里一样（预设照包开关）。
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

async fn session(home: &Home, script: &Script, lines: Lines) -> Handle {
    home.create_full(script, &catalog(home), Opening::default(), lines)
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

/// 请求里有没有知识库的两件：两件一起有、一起没有。
fn shelved(request: &Request) -> bool {
    let names: Vec<&str> = request
        .tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect();
    let (search, read) = (names.contains(&"kb_search"), names.contains(&"kb_read"));
    assert_eq!(search, read, "两件一起给：{names:?}");
    search
}

fn call(name: &str, args: serde_json::Value) -> Play {
    Play::calls(&[(name, &args.to_string())])
}

/// 在 alice 的知识库 `kb` 里放一个文档。
fn put(home: &Home, kb: &str, path: &str, text: &str) {
    let file = home
        .root
        .account_dir(&alice_account())
        .join("kb")
        .join(kb)
        .join(path);
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
    std::fs::write(file, text).expect("写得进");
}

/// 样本人格 `engineer` 收窄成 `bases`（写进系统区那一层的 `persona.toml`）。
fn narrow(home: &Home, bases: &str) {
    let file = home
        .root
        .system()
        .join("personas")
        .join("engineer")
        .join("persona.toml");
    std::fs::write(file, format!("[knowledge]\nbases = {bases}\n")).expect("写得进");
}

#[tokio::test]
async fn the_two_come_with_the_first_base_and_go_with_the_last() {
    let home = Home::new();
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。"), Play::Says("哦。")]);
    let handle = session(&home, &script, Lines::default()).await;
    chat(&home, &handle, 1, "在吗").await;
    put(&home, "notes", "cat.md", "# 猫\n\n团子是橘猫。\n");
    chat(&home, &handle, 2, "建了一个库").await;
    std::fs::remove_dir_all(home.root.account_dir(&alice_account()).join("kb")).expect("删得了");
    chat(&home, &handle, 3, "删光了").await;
    let requests = script.requests();
    assert!(!shelved(&requests[0].1), "一个库都没有的不给");
    assert!(shelved(&requests[1].1), "建了第一个库，下一个回合有了");
    assert!(!shelved(&requests[2].1), "删光了，下一个回合没了");
}

#[tokio::test]
async fn she_finds_and_reads_only_what_her_persona_lists() {
    let home = Home::new();
    put(&home, "notes", "cat.md", "# 猫\n\n团子是橘猫。\n");
    put(&home, "secret", "plan.md", "团子的秘密计划\n");
    narrow(&home, r#"["notes", "gone"]"#);
    let script = Script::new([
        call("kb_search", serde_json::json!({"query": "团子"})),
        Play::Says("好。"),
        call(
            "kb_search",
            serde_json::json!({"query": "团子", "kb": "secret"}),
        ),
        Play::Says("好。"),
        call(
            "kb_read",
            serde_json::json!({"kb": "notes", "path": "cat.md"}),
        ),
        Play::Says("好。"),
        call(
            "kb_read",
            serde_json::json!({"kb": "secret", "path": "plan.md"}),
        ),
        Play::Says("好。"),
        call(
            "kb_read",
            serde_json::json!({"kb": "notes", "path": "nope.md"}),
        ),
        Play::Says("好。"),
        Play::Says("嗯。"),
    ]);
    let handle = session(&home, &script, Lines::default()).await;
    assert_eq!(
        last_result(&chat(&home, &handle, 1, "团子").await),
        "[notes] cat.md: 猫\n团子是橘猫。",
        "只搜人格列着的、在的那几个"
    );
    assert_eq!(
        last_result(&chat(&home, &handle, 2, "秘密").await),
        "There is no knowledge base secret.",
        "人格没列的当没有"
    );
    assert_eq!(
        last_result(&chat(&home, &handle, 3, "读").await),
        "# 猫\n\n团子是橘猫。\n"
    );
    assert_eq!(
        last_result(&chat(&home, &handle, 4, "读秘密").await),
        "There is no knowledge base secret."
    );
    assert_eq!(
        last_result(&chat(&home, &handle, 5, "读没有的").await),
        "There is no document nope.md in notes."
    );
    narrow(&home, "[]");
    chat(&home, &handle, 6, "收窄成空的").await;
    let requests = script.requests();
    assert!(shelved(&requests[0].1));
    assert!(
        !shelved(&requests.last().expect("有请求").1),
        "收窄成空的，下一个回合没了"
    );
}

#[tokio::test]
async fn groups_have_them_too_and_a_preset_can_turn_them_off() {
    let home = Home::new();
    put(&home, "notes", "cat.md", "团子是橘猫。\n");
    let off = miyu_policy::preset::read("[features]\nknowledge = false\n").expect("合写法");
    for (lines, has) in [
        (
            Lines {
                venue: VenueId::parse("qq:group:123456").expect("合写法"),
                group: true,
                ..Lines::default()
            },
            true,
        ),
        (
            Lines {
                preset: Some(miyu_policy::preset::Chosen::new(
                    "quiet".to_string(),
                    off.clone(),
                    &miyu_policy::features::Features::default(),
                )),
                ..Lines::default()
            },
            false,
        ),
    ] {
        let script = Script::new([Play::Says("好。")]);
        let handle = session(&home, &script, lines).await;
        chat(&home, &handle, 1, "看看").await;
        assert_eq!(shelved(&script.requests()[0].1), has);
    }
}
