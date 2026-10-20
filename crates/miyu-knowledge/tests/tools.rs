//! 知识库查的两件工具（施工 K-1 下，`docs/blueprint/knowledge.md`「她的工具」）：参数，端口交回的几种，结果的句子逐字节比，
//! 给人看的说法。端口是假的：真的在 `miyu-session` 里测。写的三件在 `writes.rs`（施工 K-3）。

use std::sync::{Arc, PoisonError};

use miyu_tool::{KbHit, KbPage, KbRefused};

use crate::support::*;

#[tokio::test]
async fn the_five_tools_are_there_and_the_writers_write() {
    let names: Vec<String> = tools()
        .iter()
        .map(|tool| tool.spec().name.clone())
        .collect();
    assert_eq!(
        names,
        ["kb_search", "kb_read", "kb_save", "kb_edit", "kb_delete"]
    );
    for tool in tools() {
        let reads = ["kb_search", "kb_read"].contains(&tool.spec().name.as_str());
        let access = match reads {
            true => miyu_kernel::tool::Access::Read,
            false => miyu_kernel::tool::Access::Write,
        };
        assert_eq!(tool.spec().access, access, "{}", tool.spec().name);
    }
}

#[tokio::test]
async fn searching_gives_a_passage_per_hit_with_where_it_came_from() {
    let port = Arc::new(Fake {
        hits: vec![
            hit("notes", "grammar.md", "语法 › て形", "食べる → 食べて"),
            hit(
                "world",
                "towns/east.txt",
                "",
                "东边的镇子靠海。\n<b>不转义</b>\n",
            ),
            // PDF 的那一块说第几页（施工 K-5）。
            KbHit {
                page: Some(3),
                ..hit("manuals", "cat.pdf", "", "每年十月打疫苗。")
            },
        ],
        ..Fake::default()
    });
    let done = call(
        Some(Arc::clone(&port)),
        "kb_search",
        serde_json::json!({"query": "食べて", "kb": "notes"}),
    )
    .await;
    assert!(!done.error);
    assert_eq!(
        text(&done),
        "[notes] grammar.md: 语法 › て形\n食べる → 食べて\n\n[world] towns/east.txt\n东边的镇子靠海。\n<b>不转义</b>\n\n[manuals] cat.pdf, page 3\n每年十月打疫苗。",
        "字原样给，末尾的换行去掉"
    );
    assert_eq!(said(&done), "software/knowledge/kb_search/found");
    // 给头看的那一份（施工 W-11）：没有标题的、不是 PDF 的不写那一格，字去掉末尾的换行。
    assert_eq!(done.data, done.detail, "给脚本的是同一份（施工 M-2）");
    assert_eq!(
        done.detail,
        Some(serde_json::json!({"hits": [
            {"kb": "notes", "path": "grammar.md", "heading": "语法 › て形", "snippet": "食べる → 食べて"},
            {"kb": "world", "path": "towns/east.txt", "snippet": "东边的镇子靠海。\n<b>不转义</b>"},
            {"kb": "manuals", "path": "cat.pdf", "page": 3, "snippet": "每年十月打疫苗。"},
        ]}))
    );
    assert_eq!(
        *port.searched.lock().unwrap_or_else(PoisonError::into_inner),
        [("食べて".to_string(), Some("notes".to_string()))]
    );

    let none = call(
        Some(Arc::new(Fake::default())),
        "kb_search",
        serde_json::json!({"query": "猫"}),
    )
    .await;
    assert!(!none.error, "没搜到不算出错");
    assert_eq!(text(&none), "Nothing found.");
    assert_eq!(said(&none), "software/knowledge/kb_search/nothing");
    assert_eq!(none.data, none.detail, "给脚本的是同一份（施工 M-2）");
    assert_eq!(none.detail, Some(serde_json::json!({"hits": []})));
}

#[tokio::test]
async fn reading_gives_the_page_and_says_where_it_is_when_there_are_more() {
    let port = Arc::new(Fake {
        page: Some(KbPage {
            text: "第二页的字\n".to_string(),
            page: 2,
            pages: 3,
        }),
        ..Fake::default()
    });
    let done = call(
        Some(Arc::clone(&port)),
        "kb_read",
        serde_json::json!({"kb": "notes", "path": "a.md", "page": 2}),
    )
    .await;
    assert_eq!(text(&done), "第二页的字\n(Page 2 of 3.)");
    assert_eq!(done.data, done.detail, "给脚本的是同一份（施工 M-2）");
    assert_eq!(
        done.detail,
        Some(
            serde_json::json!({"kb": "notes", "path": "a.md", "page": 2, "pages": 3, "text": "第二页的字\n"})
        ),
        "给头的是这一页的正文，不带「还有几页」那一行"
    );
    assert_eq!(said(&done), "software/knowledge/kb_read/page");
    assert_eq!(
        *port.read.lock().unwrap_or_else(PoisonError::into_inner),
        [("notes".to_string(), "a.md".to_string(), 2)]
    );

    let one = Arc::new(Fake {
        page: Some(KbPage {
            text: "只有一页\n".to_string(),
            page: 1,
            pages: 1,
        }),
        ..Fake::default()
    });
    let done = call(
        Some(Arc::clone(&one)),
        "kb_read",
        serde_json::json!({"kb": "notes", "path": "a.md"}),
    )
    .await;
    assert_eq!(text(&done), "只有一页\n", "只有一页的原样给字");
    assert_eq!(
        one.read.lock().unwrap_or_else(PoisonError::into_inner)[0].2,
        1,
        "不写是第一页"
    );
}

#[tokio::test]
async fn refusals_and_bad_arguments_are_said_plainly() {
    for (refused, said_key, line) in [
        (
            KbRefused::NoBase("nope".to_string()),
            "common/no-base",
            "There is no knowledge base nope.",
        ),
        (
            KbRefused::NoDocument("notes".to_string(), "../a.md".to_string()),
            "common/no-document",
            "There is no document ../a.md in notes.",
        ),
        (
            KbRefused::NoPage(4),
            "common/no-page",
            "There is no page 4.",
        ),
        (
            KbRefused::Failed("disk full".to_string()),
            "common/failed",
            "Knowledge bases are not available right now: disk full.",
        ),
    ] {
        let port = Arc::new(Fake {
            refuse: Some(refused),
            ..Fake::default()
        });
        for (name, args) in [
            ("kb_search", serde_json::json!({"query": "猫"})),
            (
                "kb_read",
                serde_json::json!({"kb": "notes", "path": "a.md"}),
            ),
        ] {
            let done = call(Some(Arc::clone(&port)), name, args).await;
            assert!(done.error, "{name}");
            assert_eq!(text(&done), line, "{name}");
            assert_eq!(said(&done), format!("software/knowledge/{said_key}"));
        }
    }
    let done = call(None, "kb_search", serde_json::json!({"query": "猫"})).await;
    assert_eq!(
        text(&done),
        "Knowledge bases are not available in this session."
    );
    let done = call(
        Some(Arc::new(Fake::default())),
        "kb_read",
        serde_json::json!({"kb": "notes"}),
    )
    .await;
    assert!(done.error);
    assert_eq!(
        text(&done),
        "The arguments are not right: missing field `path` at line 1 column 14."
    );
}
