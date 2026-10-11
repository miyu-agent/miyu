//! 知识库的两件工具（施工 K-1 下，`docs/blueprint/knowledge.md`「她的工具」）：参数，端口交回的几种，结果的句子逐字节比，
//! 给人看的说法。端口是假的：真的在 `miyu-session` 里测。

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::block::Block;
use miyu_kernel::time::UtcOffset;
use miyu_tool::{
    Call, Done, KbHit, KbPage, KbRefused, KnowledgePort, Pending, Progress, Stop, Tool,
};

/// 一个假端口：记下她要做的，照预先定好的交回。
#[derive(Default)]
struct Fake {
    searched: Mutex<Vec<(String, Option<String>)>>,
    read: Mutex<Vec<(String, String, usize)>>,
    refuse: Option<KbRefused>,
    hits: Vec<KbHit>,
    page: Option<KbPage>,
}

impl KnowledgePort for Fake {
    fn search<'a>(
        &'a self,
        query: String,
        kb: Option<String>,
    ) -> Pending<'a, Result<Vec<KbHit>, KbRefused>> {
        self.searched
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((query, kb));
        let answer = self.refuse.clone().map_or(Ok(self.hits.clone()), Err);
        Box::pin(async move { answer })
    }

    fn read<'a>(
        &'a self,
        kb: String,
        path: String,
        page: usize,
    ) -> Pending<'a, Result<KbPage, KbRefused>> {
        self.read
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((kb, path, page));
        let answer = match &self.refuse {
            Some(refused) => Err(refused.clone()),
            None => Ok(self.page.clone().expect("备了一页")),
        };
        Box::pin(async move { answer })
    }
}

fn tools() -> Vec<Arc<dyn Tool>> {
    miyu_knowledge::tools(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"))
        .expect("读得出")
}

/// 用假端口 `port`（没有的是这个会话查不了知识库）调一次 `name`。
async fn call(port: Option<Arc<Fake>>, name: &str, args: serde_json::Value) -> Done {
    let tool = tools()
        .into_iter()
        .find(|tool| tool.spec().name == name)
        .expect("有这件");
    let call = Call {
        args: args.to_string(),
        cwd: String::new(),
        home: None,
        data_root: None,
        seen: Arc::default(),
        stop: Stop::default(),
        sandbox: None,
        log: None,
        offset: UtcOffset::from_minutes(0).expect("零时区"),
        agents: None,
        messages: None,
        jobs: None,
        sessions: None,
        usage: None,
        questions: None,
        memory: None,
        knowledge: port.map(|port| port as Arc<dyn KnowledgePort>),
        goals: None,
        packages: None,
        ids: None,
    };
    tool.run(call, Progress::new(|_| {})).await
}

fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

fn said(done: &Done) -> String {
    done.human
        .as_ref()
        .map(|said| said.key.clone())
        .unwrap_or_default()
}

fn hit(kb: &str, path: &str, heading: &str, text: &str) -> KbHit {
    KbHit {
        kb: kb.to_string(),
        path: path.to_string(),
        heading: heading.to_string(),
        text: text.to_string(),
    }
}

#[tokio::test]
async fn the_two_tools_are_there_and_only_read() {
    let names: Vec<String> = tools()
        .iter()
        .map(|tool| tool.spec().name.clone())
        .collect();
    assert_eq!(names, ["kb_search", "kb_read"]);
    for tool in tools() {
        assert_eq!(
            tool.spec().access,
            miyu_kernel::tool::Access::Read,
            "{}",
            tool.spec().name
        );
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
        "[notes] grammar.md: 语法 › て形\n食べる → 食べて\n\n[world] towns/east.txt\n东边的镇子靠海。\n<b>不转义</b>",
        "字原样给，末尾的换行去掉"
    );
    assert_eq!(said(&done), "software/knowledge/kb_search/found");
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
