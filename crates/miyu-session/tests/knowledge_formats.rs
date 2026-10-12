//! PDF 和网页（施工 K-5，`docs/blueprint/knowledge.md`「她的工具」）：真会话、真知识库的目录，执行器替身照剧本调工具。搜到
//! PDF 的那一块头一行说第几页，照那一页读得到；写 PDF、网页拒，说只有文字的写得了，什么都没动、不建库。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Event};
use miyu_kernel::session::Command;
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

fn call(name: &str, args: serde_json::Value) -> Play {
    Play::calls(&[(name, &args.to_string())])
}

/// 说一句，等这一轮结束，交回每次调用的结果。
async fn results(home: &Home, script: &Script) -> Vec<String> {
    let handle = home
        .create_full(script, &catalog(home), Opening::default(), Lines::default())
        .await;
    let said = Command::Send {
        blocks: vec![Block::Text(Text {
            text: "看看说明书".to_string(),
        })],
        urgent: false,
        venue: None,
    };
    within("回应", handle.command(id("cmd-1"), alice(), said))
        .await
        .expect("会话在跑");
    let log: Vec<Event> = until_logged(home, handle.id(), |log| {
        log.iter()
            .any(|event| matches!(event.body, Body::TurnEnded(_)))
    })
    .await;
    log.iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => match result.blocks.as_slice() {
                [Block::Text(Text { text })] => Some(text.clone()),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_pdf_is_found_by_page_read_by_page_and_never_written() {
    let home = Home::new();
    let dir = home.root.account_dir(&alice_account()).join("kb/manuals");
    std::fs::create_dir_all(&dir).expect("建得了");
    let pdf = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../miyu-store/tests/fixtures/knowledge/cat.pdf");
    std::fs::copy(&pdf, dir.join("cat.pdf")).expect("拷得了");
    let script = Script::new([
        call("kb_search", serde_json::json!({"query": "October"})),
        call(
            "kb_read",
            serde_json::json!({"kb": "manuals", "path": "cat.pdf", "page": 2}),
        ),
        call(
            "kb_save",
            serde_json::json!({"kb": "manuals", "path": "cat.pdf", "text": "x", "append": true}),
        ),
        call(
            "kb_save",
            serde_json::json!({"kb": "fresh", "path": "new.html", "text": "x"}),
        ),
        Play::Says("好。"),
    ]);
    let results = results(&home, &script).await;
    assert_eq!(
        results[..2],
        [
            "[manuals] cat.pdf, page 2\nVaccines are given every October.",
            "Vaccines are given every October.\n(Page 2 of 3.)",
        ]
    );
    assert_eq!(
        results[2..],
        [
            "cat.pdf is not a text document. Only .md, .markdown and .txt can be written.",
            "new.html is not a text document. Only .md, .markdown and .txt can be written.",
        ]
    );
    assert!(
        !home
            .root
            .account_dir(&alice_account())
            .join("kb/fresh")
            .exists(),
        "写不了的不建库"
    );
    assert_eq!(
        std::fs::read(dir.join("cat.pdf")).expect("还在"),
        std::fs::read(&pdf).expect("读得了"),
        "什么都没动"
    );
}
