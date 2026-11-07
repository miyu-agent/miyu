//! 测试扩展答 `driver.call`（施工 MD-2，设计 66 第五节）：照 `driver:<怎么答>` 交 `driver.delta`、再回答。回的字是请求里最后一段字
//! （人说的那一句）前面加 `echo: `，两个会话同时调分得清谁是谁的。
//!
//! - `echo`：一段一段交完、回答带用量和 `sent`；
//! - `half`：交了开头一段就以退出码 3 退出（交了一半断了）；
//! - `pair`：第一次先不答，第二次来了两次交错着交（第一次的字、第二次的字、各自收尾），先答第二次、再答第一次：核心照 `call`
//!   分开，两个会话各拿到自己的。

/// 压着没答的那一次：请求编号、调用编号、要回的字。
pub(crate) struct Held {
    first: Option<(String, String, String)>,
}

impl Held {
    pub(crate) fn new() -> Held {
        Held { first: None }
    }
}

/// 答这一行 `driver.call`（JSON-RPC 编号 `id`），照 `how`：交回要写的几行，照先后。
pub(crate) fn answered(id: &str, how: &str, line: &str, held: &mut Held) -> Vec<String> {
    let call = super::field(line, "call").unwrap_or_default();
    let said = format!("echo: {}", last_text(line));
    match how {
        "echo" => {
            let mut lines = deltas(&call, &said);
            lines.push(answer(id));
            lines
        }
        "half" => {
            super::send(&delta(
                &call,
                &format!(
                    r#"{{"start":{{"index":0,"kind":"text"}}}},{{"text":{{"index":0,"text":"{}"}}}}"#,
                    super::quoted(&said)
                ),
            ));
            std::process::exit(3);
        }
        "pair" => match held.first.take() {
            None => {
                held.first = Some((id.to_string(), call, said));
                Vec::new()
            }
            Some((first_id, first_call, first_said)) => {
                let start = r#"{"start":{"index":0,"kind":"text"}}"#;
                let text = |said: &str| {
                    format!(
                        r#"{{"text":{{"index":0,"text":"{}"}}}}"#,
                        super::quoted(said)
                    )
                };
                let end = r#"{"end":{"index":0}}"#;
                vec![
                    delta(&first_call, &format!("{start},{}", text(&first_said))),
                    delta(&call, &format!("{start},{}", text(&said))),
                    delta(&call, end),
                    delta(&first_call, end),
                    answer(id),
                    answer(&first_id),
                ]
            }
        },
        _ => Vec::new(),
    }
}

/// 一块正文 `said`：开始、字、收尾，分两条 `driver.delta`。
fn deltas(call: &str, said: &str) -> Vec<String> {
    vec![
        delta(
            call,
            &format!(
                r#"{{"start":{{"index":0,"kind":"text"}}}},{{"text":{{"index":0,"text":"{}"}}}}"#,
                super::quoted(said)
            ),
        ),
        delta(call, r#"{"end":{"index":0}}"#),
    ]
}

fn delta(call: &str, deltas: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","method":"driver.delta","params":{{"call":"{call}","deltas":[{deltas}]}}}}"#
    )
}

fn answer(id: &str) -> String {
    format!(
        r#"{{"jsonrpc":"2.0","id":"{id}","result":{{"usage":{{"uncached":5,"cache_read":0,"cache_write":0,"output":2}},"sent":"sha256:{}"}}}}"#,
        "ef".repeat(32)
    )
}

/// 请求里最后一段 `"text":"…"` 的字：人说的那一句（参数里请求排在最前，后面几格没有 `text`）。
fn last_text(line: &str) -> String {
    let Some((_, rest)) = line.rsplit_once(r#""text":""#) else {
        return String::new();
    };
    rest.split_once('"')
        .map_or("", |(text, _)| text)
        .to_string()
}
