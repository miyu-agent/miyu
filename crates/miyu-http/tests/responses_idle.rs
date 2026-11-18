//! Responses 接口的工具参数攒着、一项完了才交（施工 AU-013，审计 #013）：攒的那一阵对方一直在发，空闲超时不算。

use std::time::Duration;

use miyu_http::testkit::{Piece, Reply, Server};
use miyu_http::{Attempt, Endpoint, Outcome, Progress, Proxy, client, send};
use miyu_kernel::accumulate::{Delta, Kind};

/// 写进 JSON 字符串里：反斜杠、双引号转义（这几段里没有别的要转的）。
fn quoted(text: &str) -> String {
    text.replace('\\', "\\\\").replace('"', "\\\"")
}

fn event(name: &str, data: &str) -> Piece {
    Piece::Bytes(format!("event: {name}\ndata: {data}\n\n").into_bytes())
}

/// 修以前：参数一片片来，每片隔得比空闲超时短，加起来比它长；驱动攒着不交增量，执行器只认增量，当成没动静掐了（`Stalled`）。
/// 现在攒着的也算有动静，照常收完。
#[tokio::test]
async fn arguments_streaming_longer_than_the_idle_limit_are_not_stalled() {
    let mut pieces = vec![event(
        "response.output_item.added",
        r#"{"type":"response.output_item.added","output_index":0,"item":{"id":"fc_1","type":"function_call","arguments":"","call_id":"call_1","name":"write"}}"#,
    )];
    let parts = [
        "{\"path\":",
        "\"a.txt\",",
        "\"content\":",
        "\"x",
        "yz",
        "\"",
        "}",
    ];
    for part in parts {
        let delta = format!(
            r#"{{"type":"response.function_call_arguments.delta","output_index":0,"delta":"{}"}}"#,
            quoted(part)
        );
        pieces.push(Piece::Wait(Duration::from_millis(60)));
        pieces.push(event("response.function_call_arguments.delta", &delta));
    }
    let whole: String = parts.concat();
    let done = format!(
        r#"{{"type":"response.output_item.done","output_index":0,"item":{{"id":"fc_1","type":"function_call","arguments":"{}","call_id":"call_1","name":"write"}}}}"#,
        quoted(&whole)
    );
    pieces.push(event("response.output_item.done", &done));
    pieces.push(event(
        "response.completed",
        r#"{"type":"response.completed","response":{"status":"completed","usage":{"input_tokens":5,"output_tokens":9}}}"#,
    ));
    let server = Server::start(vec![Reply::stream(pieces)]).await;
    let client = client(Proxy::Off).expect("造得出客户端");
    let endpoint = Endpoint::new(&server.base_url, "sk-test");
    let driver = crate::support::responses();
    let mut deltas = Vec::new();
    let outcome = send(
        Attempt {
            client: &client,
            endpoint: &endpoint,
            driver: &driver,
            body: crate::support::BODY.to_vec().into(),
            path: "/responses",
            idle: Duration::from_millis(200),
        },
        std::future::pending(),
        |step| {
            if let Progress::Delta(delta) = step {
                deltas.push(delta);
            }
        },
    )
    .await;
    assert!(
        matches!(outcome, Outcome::Ended { error: None, .. }),
        "{outcome:?}"
    );
    assert!(deltas.iter().any(|delta| matches!(
        delta,
        Delta::Start { kind: Kind::ToolCall { name }, .. } if name == "write"
    )));
    let text: String = deltas
        .iter()
        .filter_map(|delta| match delta {
            Delta::Text { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(text, whole, "参数照整段交");
}
