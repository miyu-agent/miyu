//! 工具要结束这一轮（施工 4-14）：真 actor、假模型。她调了要结束的工具，这一轮就结束，不再请求模型（剧本只给一步，多请求就
//! 没得回）；同一步别的工具照常跑完。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::event::{Body, EndReason};
use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::*;

fn catalog() -> Catalog {
    let tools = [
        Fake::new("skip", Access::Read, Act::Ends) as Arc<dyn Tool>,
        Fake::new("read", Access::Read, Act::Echo) as Arc<dyn Tool>,
    ];
    Catalog::new(tools).expect("合写法")
}

#[tokio::test]
async fn the_turn_ends_without_another_request() {
    let home = Home::new();
    let models =
        Script::new([Play::calls(&[("skip", "{}"), ("read", "{}")])]).titles([Play::Says("测试")]);
    let handle = home.create_with(&models, &catalog()).await;
    let session = handle.id().clone();
    ask(&handle, "m1", say("群里有人说话了"))
        .await
        .expect("会话在跑");
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    let ended = loop {
        let ended = home
            .log(&session)
            .into_iter()
            .find_map(|event| match event.body {
                Body::TurnEnded(ended) => Some(ended),
                _ => None,
            });
        if let Some(ended) = ended {
            break ended;
        }
        assert!(tokio::time::Instant::now() < deadline, "六十秒内没结束");
        tokio::time::sleep(Duration::from_millis(10)).await;
    };
    assert_eq!(ended.reason, EndReason::Completed);
    let log = home.log(&session);
    let results: Vec<bool> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::ToolResult(result) => Some(result.end_turn),
            _ => None,
        })
        .collect();
    assert_eq!(results.len(), 2, "同一步的两件都跑完了");
    assert_eq!(results.iter().filter(|ends| **ends).count(), 1);
    assert_eq!(models.requests().len(), 1, "不再请求模型");
}
