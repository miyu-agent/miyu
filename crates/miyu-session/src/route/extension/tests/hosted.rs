//! 经端口交出去的请求里的供应商侧工具（施工 HT-2）：别家的在 `driver.call` 的 `request` 里是一块 `text`，自己家的原样。
//! 换成什么字在 `wire/tests/hosted.rs` 里逐块查，这里只查走的是会话冻结的那份占位。

use super::*;

pub(super) use crate::route::extension::wire::tests::hosted::texts_with;

/// 带着出厂的供应商侧工具那四份的占位。
pub(super) fn texts() -> DriverTexts {
    texts_with(true)
}

#[tokio::test]
async fn hosted_tools_reach_the_extension_retold_unless_they_are_its_own() {
    use crate::route::extension::wire::tests::hosted::hosted;
    let fake = scripted(vec![Plan::Answer(hello(), Duration::ZERO, usage())]);
    let mut request = request(None);
    request.messages.insert(
        0,
        Message::Assistant {
            blocks: vec![hosted("call_1_1", Some("pi/ag")), hosted("call_1_2", None)],
        },
    );
    let (ended, _, _) = run(Arc::clone(&fake), Duration::from_secs(5), request).await;
    assert_eq!(error(&ended), None);
    let params = fake.params.lock().expect("锁")[0].clone();
    let blocks = &params["request"]["messages"][0]["blocks"];
    assert_eq!(blocks[0]["type"], "hosted_tool");
    assert_eq!(blocks[0]["private"]["data"]["id"], "srvtoolu_1");
    assert_eq!(blocks[1]["type"], "text");
    let text = blocks[1]["text"].as_str().expect("一段字");
    assert!(
        text.starts_with("<hosted-tool name=\"web_search\">\n"),
        "{text}"
    );
    assert!(text.ends_with("</hosted-tool>"), "末尾不带换行：{text}");
    assert!(
        text.contains("\n{\"query\":\"<rust>\"}\n"),
        "参数原样：{text}"
    );
}
