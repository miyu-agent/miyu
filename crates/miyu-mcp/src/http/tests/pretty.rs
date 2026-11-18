//! 排成多行的回应（审计 AUD-5）：缩进着写的 JSON、一个事件拆成几行 `data:` 的 SSE，都照一条消息收，握手、列工具、调工具照常。

use super::*;

#[tokio::test]
async fn a_server_answering_in_several_lines_is_understood() {
    for sse in [false, true] {
        let server = HttpServer::start(sse).await;
        server.pretty();
        let (log, lines) = log();
        let client = tokio::time::timeout(Duration::from_secs(10), connect(&server, log))
            .await
            .expect("握手不干等")
            .0
            .unwrap_or_else(|error| panic!("连得上（sse={sse}）：{error:?}"));
        assert_eq!(client.era(), &Era::Legacy("2025-06-18".to_string()));
        let listed = client.tools().await.expect("列得出");
        assert_eq!(listed.tools[0].name, "echo");
        let called = client
            .call("echo", json!({"text": "hi"}))
            .await
            .expect("调得到");
        assert_eq!(called.content.len(), 1, "{called:?}");
        assert!(
            lines.lock().unwrap().is_empty(),
            "{:?}",
            lines.lock().unwrap()
        );
    }
}

#[test]
fn a_multi_line_event_is_one_compact_message() {
    let body = "data: {\ndata:   \"a\": [1,\ndata: 2]\ndata: }\n\n";
    assert_eq!(events(body), ["{\"a\":[1,2]}"]);
    // 解不出 JSON 的照原来接起来交出去，由下游照原来那样报错，不吞。
    assert_eq!(events("data: not\ndata: json\n\n"), ["not\njson"]);
}

#[test]
fn a_json_body_is_one_message_however_it_is_laid_out() {
    // 只有一个键，写出来的先后不看 `serde_json` 排不排键。
    let pretty = serde_json::to_string_pretty(&json!({"result": {"a": [1, 2]}})).expect("写得出");
    assert!(pretty.contains('\n'));
    assert_eq!(json_messages(&pretty), ["{\"result\":{\"a\":[1,2]}}"]);
    // 批量回应（数组）同一条消息里的一样整段交出去，和一行写的数组一样。
    assert_eq!(
        json_messages("[\n {\"id\": 1},\n {\"id\": 2}\n]\n"),
        ["[{\"id\":1},{\"id\":2}]"]
    );
    // 原来一行一条能过的照旧：整段不是一个 JSON 值的，还按行拆。
    assert_eq!(
        json_messages("{\"id\":1}\n{\"id\":2}\n"),
        ["{\"id\":1}", "{\"id\":2}"]
    );
    assert_eq!(json_messages(" {\"id\":1} "), ["{\"id\":1}"]);
}
