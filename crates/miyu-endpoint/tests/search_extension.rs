//! 扩展的搜索来源（施工 N-5）：真核心拉起测试用的扩展，清单 `[[search]]` 声明了 `finder/web`。核心反向调它的 `search.query`：
//! 答了的交回清单里的名字和它的回应；它说限额的是 `Limited`，别的错是 `Failed`；没声明的来源、没开的扩展不问。

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;

use miyu_endpoint::search_sources::{Asked, query};
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 包 `finder` 的清单：程序 `program`、参数 `args`，要 `search` 能力，声明来源 `web`。
fn manifest(program: &str, args: &[String]) -> String {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Finder\" }}\n\n[command]\nname = \"finder\"\nprogram = \"{program}\"\nabout = {{ en = \"F\" }}\n\n[process]\nargs = [{}]\nstart = \"manual\"\ncapabilities = [\"search\"]\n\n[[search]]\nid = \"web\"\nname = {{ en = \"Finder\", zh = \"查找\" }}\n",
        args.join(", ")
    )
}

async fn ask(
    core: &miyu_endpoint::Core,
    source: &str,
    text: &str,
) -> Result<(String, serde_json::Value), Asked> {
    let owner = alice();
    query(
        core,
        (source, Some(&owner)),
        json!({"source": source, "query": text, "want": 8}),
        Duration::from_secs(30),
    )
    .await
}

#[tokio::test]
async fn the_extension_is_asked_and_its_answers_are_read() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "finder");
    let steps = steps(&[&step, "hello", "ask:provide:{\"tools\":[]}", "serve"]);
    home.write(
        "system/packages/finder/package.toml",
        &manifest(&program.name(), &steps),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    assert_eq!(
        ask(&core, "finder/web", "rust").await,
        Err(Asked::Failed("finder is not running".to_string())),
        "没开的不问"
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let enabled = client
        .call(
            "e1",
            "extension.enable",
            json!({"package": "finder", "approve": ["search"]}),
        )
        .await;
    assert!(enabled["result"].is_object(), "{enabled}");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).lines().count() < 3 {
        assert!(tokio::time::Instant::now() < deadline, "等不到它登记");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let (name, result) = ask(&core, "finder/web", "rust").await.expect("答了");
    assert_eq!(name, "Finder", "名字照清单");
    assert_eq!(result["hits"][0]["url"], "https://ext.example/rust");
    assert!(read(&path).contains(r#""query":"rust""#), "参数交到了");
    assert_eq!(ask(&core, "finder/web", "limit").await, Err(Asked::Limited));
    assert!(
        matches!(ask(&core, "finder/web", "boom").await, Err(Asked::Failed(why)) if why == "boom")
    );
    assert!(
        matches!(
            ask(&core, "finder/other", "rust").await,
            Err(Asked::Failed(_))
        ),
        "没声明的来源不问"
    );
    assert!(matches!(
        ask(&core, "exa", "rust").await,
        Err(Asked::Failed(_))
    ));
}
