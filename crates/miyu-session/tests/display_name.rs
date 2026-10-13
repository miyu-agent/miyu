//! 工具给人看的名字（施工 9-8 六补）：真 actor、假模型。她调了写了显示名的工具（像脚本、MCP 的、扩展的），结果里记下派它那一刻
//! 的显示名；核心自带的那种没写的不记；工具崩了、交回出错的照样记。

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::event::{Body, ToolResult};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

#[tokio::test]
async fn a_result_records_the_name_people_see_when_it_was_called() {
    let home = Home::new();
    let tools = [
        Fake::named(
            "weather",
            Access::Read,
            &[("zh", "天气"), ("en", "Weather")],
            Act::Echo,
        ) as Arc<dyn Tool>,
        Fake::new("read", Access::Read, Act::Echo) as Arc<dyn Tool>,
        Fake::named("boom", Access::Read, &[("zh", "炸了")], Act::Panics) as Arc<dyn Tool>,
    ];
    let models = Script::new([
        Play::calls(&[("weather", "{}"), ("read", "{}"), ("boom", "{}")]),
        Play::Says("好。"),
    ])
    .titles([Play::Says("测试")]);
    let handle = home
        .create_with(&models, &Catalog::new(tools).expect("合写法"))
        .await;
    ask(&handle, "m1", say("今天天气")).await.expect("会话在跑");
    let results = |log: Vec<miyu_kernel::event::Event>| -> Vec<ToolResult> {
        log.into_iter()
            .filter_map(|event| match event.body {
                Body::ToolResult(result) => Some(result),
                _ => None,
            })
            .collect()
    };
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let got = loop {
        let got = results(home.log(handle.id()));
        if got.len() == 3 {
            break got;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "六十秒内两个结果没到齐"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    let named = BTreeMap::from([
        ("en".to_string(), "Weather".to_string()),
        ("zh".to_string(), "天气".to_string()),
    ]);
    let of = |call: usize| {
        got.iter()
            .find(|result| result.call_id.to_string().ends_with(&format!("_{call}")))
            .map(|result| result.display_name.clone())
            .expect("有这个结果")
    };
    assert_eq!(of(1), named, "写了显示名的记下");
    assert!(of(2).is_empty(), "没写的不记");
    assert_eq!(
        of(3),
        BTreeMap::from([("zh".to_string(), "炸了".to_string())]),
        "崩了的照样记"
    );
}
