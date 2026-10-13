//! 软件包带来的知识库（施工 K-8，`docs/blueprint/knowledge.md`）：真会话、真目录。装了的包里的库她搜得到，开了 `auto` 的
//! 提到标题名自动想起；写进去拒，说清是随包来的、写到别的库里。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::session::Command;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

fn catalog(home: &Home) -> Catalog {
    let resources = home.resources.path();
    Catalog::in_packages(vec![(
        miyu_knowledge::PACKAGE,
        miyu_knowledge::tools(resources).expect("读得出"),
    )])
    .expect("合写法")
}

/// 说 `words`，等到结束了 `turn` 轮，交回日志。
async fn chat(home: &Home, handle: &Handle, turn: usize, words: &str) -> Vec<Event> {
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: words.to_string(),
        })],
        urgent: false,
        venue: None,
    };
    within(
        "回应",
        handle.command(id(&format!("cmd-{turn}")), alice(), said),
    )
    .await
    .expect("会话在跑");
    until_logged(home, handle.id(), |log| {
        log.iter()
            .filter(|event| matches!(event.body, Body::TurnEnded(_)))
            .count()
            >= turn
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

#[tokio::test]
async fn a_package_base_is_searched_and_recalled_but_not_written() {
    let home = Home::new();
    let pack = home.scratch.0.join("world-pack");
    let base = pack.join("kb/world");
    std::fs::create_dir_all(&base).expect("建得了");
    std::fs::write(base.join("kb.toml"), "auto = true\n").expect("写得进");
    std::fs::write(base.join("people.md"), "# 人物\n\n## 艾琳\n\n守灯人。\n").expect("写得进");
    home.knowledge
        .use_packages(vec![("world-pack".to_string(), pack)]);
    // 先照文件补好索引：头一回补在慢的机器上（Windows 的 CI）会把挂接点拖过 300 毫秒，那一轮照规矩不交（同
    // `knowledge_recall.rs`）；这里测的是包里的库挑得上，不是头一回补要多久。
    home.knowledge.list(&alice_account()).expect("补得了");
    let search = serde_json::json!({"query": "守灯人"}).to_string();
    let save = serde_json::json!({"kb": "world", "path": "a.md", "text": "记一笔"}).to_string();
    let script = Script::new([
        Play::calls(&[("kb_search", &search)]),
        Play::Says("好。"),
        Play::calls(&[("kb_save", &save)]),
        Play::Says("好。"),
    ]);
    let handle = home
        .create_full(
            &script,
            &catalog(&home),
            Opening::default(),
            Lines::default(),
        )
        .await;
    let log = chat(&home, &handle, 1, "艾琳是谁").await;
    assert!(
        last_result(&log).starts_with("[world] people.md: 人物 › 艾琳"),
        "{}",
        last_result(&log)
    );
    assert!(
        log.iter().any(|event| matches!(&event.body,
            Body::ContextInjected(fact) if fact.text.starts_with("<knowledge>\n[world] people.md")
        )),
        "开着 auto 的包里的库，提到标题名自动想起"
    );
    let log = chat(&home, &handle, 2, "记一下").await;
    assert_eq!(
        last_result(&log),
        "world comes with a package and is read-only. Save to another knowledge base."
    );
    assert!(!base.join("a.md").exists());
    assert!(
        !home
            .root
            .account_dir(&alice_account())
            .join("kb/world")
            .exists(),
        "不另建一个盖住它"
    );
}
