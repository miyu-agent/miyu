//! 假模型服务：读请求头、找记号、回答的写法，和真起一个收两个请求。

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use super::*;

#[test]
fn a_head_is_read_once_the_blank_line_is_there() {
    assert_eq!(
        parse_head(b"POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\n"),
        None
    );
    let data =
        b"POST /v1/chat/completions HTTP/1.1\r\nhost: x\r\ncontent-length: 12\r\n\r\n{\"a\":";
    assert_eq!(
        parse_head(data),
        Some(Head {
            method: "POST".to_string(),
            length: 12,
            size: data.len() - 5,
        })
    );
    let get = b"GET /v1/models HTTP/1.1\r\nHost: x\r\n\r\n";
    assert_eq!(
        parse_head(get).map(|head| (head.method, head.length)),
        Some(("GET".to_string(), 0))
    );
}

#[test]
fn a_marker_is_found_anywhere() {
    assert!(mentions(b"perf-7 at the start", b"perf-7"));
    assert!(mentions(b"in the middle perf-7 here", b"perf-7"));
    assert!(mentions(b"at the end perf-7", b"perf-7"));
    assert!(!mentions(b"perf-70 is not perf-8", b"perf-9"));
    assert!(!mentions(b"short", b"a longer marker"));
    assert!(!mentions(b"anything", b""));
}

#[test]
fn the_reply_is_an_openai_chat_stream() {
    let body = stream_pieces("你好 \"quoted\"", 1, 1000, 800);
    let chunks: Vec<&str> = body
        .split("\n\n")
        .filter(|chunk| !chunk.is_empty())
        .collect();
    assert_eq!(chunks.len(), 3);
    assert_eq!(chunks[2], "data: [DONE]");
    let said: serde_json::Value =
        serde_json::from_str(chunks[0].strip_prefix("data: ").unwrap()).unwrap();
    assert_eq!(said["choices"][0]["delta"]["content"], "你好 \"quoted\"");
    let done: serde_json::Value =
        serde_json::from_str(chunks[1].strip_prefix("data: ").unwrap()).unwrap();
    assert_eq!(done["choices"][0]["finish_reason"], "stop");
    assert_eq!(done["usage"]["total_tokens"], 1800);
}

/// 切成几块（施工 V-22）：块数照给的，接起来是原文，不切开一个字；字比块少的一个字一块；头一块才带 `role`。
#[test]
fn a_split_reply_joins_back_to_the_text() {
    let contents = |body: &str| -> Vec<serde_json::Value> {
        body.split("\n\n")
            .filter_map(|chunk| chunk.strip_prefix("data: "))
            .filter(|chunk| *chunk != "[DONE]")
            .map(|chunk| serde_json::from_str::<serde_json::Value>(chunk).unwrap())
            .filter(|said| said["choices"][0]["finish_reason"].is_null())
            .map(|said| said["choices"][0]["delta"].clone())
            .collect()
    };
    let text = filler(3500);
    for pieces in [1, 7, 300] {
        let deltas = contents(&stream_pieces(&text, pieces, 10, 10));
        assert_eq!(deltas.len(), pieces, "{pieces} 块");
        let joined: String = deltas
            .iter()
            .map(|delta| delta["content"].as_str().unwrap())
            .collect();
        assert_eq!(joined, text);
        assert_eq!(deltas[0]["role"], "assistant");
        assert!(deltas[1..].iter().all(|delta| delta.get("role").is_none()));
    }
    assert_eq!(
        contents(&stream_pieces("你好", 5, 1, 1)).len(),
        2,
        "一个字一块"
    );
}

#[test]
fn the_filler_reaches_its_size() {
    for bytes in [0, 1, 500, 3500] {
        let text = filler(bytes);
        assert!(text.len() >= bytes, "{bytes}");
        assert!(text.len() < bytes + 400, "{bytes}");
    }
}

/// 发一个请求，读完回应。
async fn send(base: &str, request: String) -> String {
    let address = base.trim_start_matches("http://").trim_end_matches("/v1");
    let mut socket = TcpStream::connect(address).await.unwrap();
    socket.write_all(request.as_bytes()).await.unwrap();
    let mut response = String::new();
    socket.read_to_string(&mut response).await.unwrap();
    response
}

#[tokio::test]
async fn it_answers_and_hands_over_the_marked_request() {
    let mut fake = Fake::start(100).await.unwrap();
    let models = send(
        &fake.base_url,
        "GET /v1/models HTTP/1.1\r\nHost: x\r\n\r\n".to_string(),
    )
    .await;
    assert!(
        models.ends_with(r#"{"object":"list","data":[]}"#),
        "{models}"
    );

    // 请求体比一次读的大，分几次才读完。
    let body = format!("{{\"text\":\"{}perf-1\"}}", "x".repeat(200_000));
    let request = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let reply = send(&fake.base_url, request).await;
    assert!(reply.starts_with("HTTP/1.1 200 OK\r\n"), "{reply}");
    assert!(reply.ends_with("data: [DONE]\n\n"), "{reply}");

    let arrival = fake.request_with("perf-1").await.unwrap();
    assert_eq!(arrival.body, body.as_bytes());
}

/// 回答里收尾那一块的用量：照请求体估的输入、照正文估的输出（施工 V-2 上）。
fn usage_of(stream: &str) -> serde_json::Value {
    stream
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str::<serde_json::Value>(data).ok())
        .find_map(|chunk| chunk.get("usage").cloned())
        .expect("收尾那一块带用量")
}

#[test]
fn usage_follows_the_size_of_the_request() {
    let small = answer(&[b'x'; 4_000], "reply", 1);
    let large = answer(&[b'x'; 400_000], "reply", 1);
    assert_eq!(usage_of(&small)["prompt_tokens"], 1_000);
    assert_eq!(usage_of(&large)["prompt_tokens"], 100_000);
    assert!(small.contains("reply"), "{small}");
}

#[test]
fn a_summary_request_gets_a_summary() {
    let body =
        br#"{"messages":[{"content":"Write a detailed summary of the conversation above. ..."}]}"#;
    let answered = answer(body, "ordinary reply", 1);
    assert!(answered.contains("<summary>"), "{answered}");
    assert!(!answered.contains("ordinary reply"), "{answered}");
    let ordinary = answer(b"{\"messages\":[]}", "ordinary reply", 1);
    assert!(!ordinary.contains("<summary>"), "{ordinary}");
}

fn script() -> script::Script {
    script::Script {
        marker: "perf-tool-03".to_string(),
        calls: (0..3)
            .map(|n| script::ToolCall {
                id: format!("perf_03_{n:03}"),
                name: "read".to_string(),
                arguments: format!("{{\"file_path\":\"/w/{n}\"}}"),
            })
            .collect(),
    }
}

/// 剧本（施工 V-29）：提到记号的照请求体里有几次的结果回下一次调用，回完了收尾；不提记号的不归它。
#[test]
fn a_script_answers_the_next_call() {
    let script = script();
    assert_eq!(script.answered(b"perf-tool-04: go."), None);
    assert_eq!(script.answered(b"perf-tool-03: go."), Some(0));
    assert_eq!(
        script.answered(b"perf-tool-03 perf_03_000 perf_03_001"),
        Some(2)
    );
    // 中间缺了一次的只数到缺的那里。
    assert_eq!(script.answered(b"perf-tool-03 perf_03_001"), Some(0));
    let stream = script.answer(b"perf-tool-03 perf_03_000", 1);
    let chunks: Vec<serde_json::Value> = stream
        .split("\n\n")
        .filter_map(|chunk| chunk.strip_prefix("data: "))
        .filter(|chunk| *chunk != "[DONE]")
        .map(|chunk| serde_json::from_str(chunk).unwrap())
        .collect();
    assert!(stream.ends_with("data: [DONE]\n\n"));
    assert_eq!(
        chunks[0]["choices"][0]["delta"]["content"],
        script::PREAMBLE
    );
    let call = &chunks[1]["choices"][0]["delta"]["tool_calls"][0];
    assert_eq!(call["id"], "perf_03_001");
    assert_eq!(call["function"]["name"], "read");
    assert_eq!(call["function"]["arguments"], "{\"file_path\":\"/w/1\"}");
    assert_eq!(chunks[2]["choices"][0]["finish_reason"], "tool_calls");
    // 回完了：只有那一句，不带调用，照常 `stop`。
    let last = script.answer(b"perf-tool-03", 3);
    assert!(last.contains(script::PREAMBLE));
    assert!(!last.contains("tool_calls"));
    assert!(last.contains("\"finish_reason\":\"stop\""));
}

/// 真起一个（施工 V-29）：照剧本回，写完交来时刻，归剧本的请求带着第几次。
#[tokio::test]
async fn a_scripted_answer_reports_when_it_was_written() {
    let mut fake = Fake::start(10).await.unwrap();
    fake.script(Some(script()));
    let body = "{\"x\":\"perf-tool-03 perf_03_000\"}";
    let request = format!(
        "POST /v1/chat/completions HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    );
    let reply = send(&fake.base_url, request).await;
    assert!(reply.contains("perf_03_001"));
    let arrival = fake.request_with("perf-tool-03").await.unwrap();
    assert_eq!(arrival.step, Some(1));
    assert!(arrival.cpu.is_none());
    let answered = arrival.answered.await.unwrap();
    assert!(answered.at >= arrival.at);
}
