//! 接在整份 messages 最末尾的那一句（施工 RT-1，`Request::tail`）：三家各编一遍，带和不带逐字节比：前面一个字节都不动，只在
//! 最后多出那一句；最后是 user 的并进那一条，是 tool 结果的另起一条 user；openai-chat 前缀续写的那一次不带；anthropic 最后那个
//! 缓存打点打在它前面那一块上，下一步（工具第二步）的请求正好接着用上一步存到那里的。

use std::collections::BTreeMap;

use crate::support::{call, claude, deepseek, gpt, id, read_tool, text, texts, tool_call};
use miyu_drivers::{Inputs, anthropic, openai_chat, openai_responses};
use miyu_kernel::request::{Message, Request};

const TAIL: &str = "（群里的提示）";

fn user(words: &str) -> Message {
    Message::User {
        blocks: vec![text(words)],
    }
}

fn request(messages: Vec<Message>, tail: Option<&str>) -> Request {
    Request {
        tools: vec![read_tool()],
        system: "You are Miyu.".to_string(),
        messages,
        stable: 0,
        continuation: false,
        described: Default::default(),
        output_cap: None,
        used: None,
        tail: tail.map(str::to_string),
    }
}

/// 第一步：人说的那一句结尾。
fn first() -> Vec<Message> {
    vec![
        user("你好"),
        Message::Assistant {
            blocks: vec![text("你好！")],
        },
        user("查一下天气"),
    ]
}

/// 工具第二步：调了一次工具、结果回来了。
fn second() -> Vec<Message> {
    let mut messages = first();
    messages.push(Message::Assistant {
        blocks: vec![tool_call("call_1_1", "read", "{}", None)],
    });
    messages.push(Message::Tool {
        call_id: id("call_1_1"),
        error: false,
        blocks: vec![text("晴")],
    });
    messages
}

fn chat(request: &Request) -> String {
    let body = openai_chat::encode(
        request,
        &call(Inputs::default(), None),
        &deepseek(),
        &texts(),
        &BTreeMap::new(),
    )
    .expect("编码得了")
    .body;
    String::from_utf8(body).expect("UTF-8")
}

fn responses(request: &Request) -> String {
    let body = openai_responses::encode(
        request,
        &gpt(Inputs::default(), None),
        &texts(),
        &BTreeMap::new(),
    )
    .expect("编码得了")
    .body;
    String::from_utf8(body).expect("UTF-8")
}

fn claude_body(request: &Request) -> String {
    let body = anthropic::encode(
        request,
        &claude(Inputs::default(), Some(1000)),
        &texts(),
        &BTreeMap::new(),
    )
    .expect("编码得了")
    .body;
    String::from_utf8(body).expect("UTF-8")
}

/// 两份字节相同的前缀有多长。
fn shared(a: &str, b: &str) -> usize {
    a.bytes().zip(b.bytes()).take_while(|(x, y)| x == y).count()
}

/// `with` 是 `without` 在 `at` 那里插进 `inserted`：前面一个字节都不动，后面原样接上。
fn inserted(without: &str, with: &str, inserted: &str) {
    let at = shared(without, with);
    assert_eq!(
        &with[at..at + inserted.len()],
        inserted,
        "插进去的不对：{with}"
    );
    assert_eq!(
        &with[at + inserted.len()..],
        &without[at..],
        "插进去以后的不对：{with}"
    );
}

#[test]
fn openai_chat_joins_the_last_user_or_starts_one_after_tool_results() {
    let (a, b) = (
        chat(&request(first(), None)),
        chat(&request(first(), Some(TAIL))),
    );
    inserted(&a, &b, &format!("\\n{TAIL}"));
    assert!(b.contains(&format!("查一下天气\\n{TAIL}\"")), "{b}");
    let (a, b) = (
        chat(&request(second(), None)),
        chat(&request(second(), Some(TAIL))),
    );
    inserted(
        &a,
        &b,
        &format!(",{{\"role\":\"user\",\"content\":\"{TAIL}\"}}"),
    );
}

#[test]
fn openai_chat_leaves_it_out_when_continuing_a_reply() {
    let mut cut = request(
        vec![
            user("讲个故事"),
            Message::Assistant {
                blocks: vec![text("从前有座山")],
            },
            user("（被打断的那一句）"),
        ],
        None,
    );
    cut.continuation = true;
    let without = chat(&cut);
    cut.tail = Some(TAIL.to_string());
    assert_eq!(chat(&cut), without, "前缀续写的那一次不带");
    assert!(!without.contains(TAIL));
}

#[test]
fn responses_joins_the_last_user_or_starts_one_after_tool_results() {
    let (a, b) = (
        responses(&request(first(), None)),
        responses(&request(first(), Some(TAIL))),
    );
    let at = shared(&a, &b);
    assert!(
        b[at..].starts_with(&format!("\\n{TAIL}")),
        "并进最后那条 user：{b}"
    );
    inserted(&a, &b, &format!("\\n{TAIL}"));
    let (a, b) = (
        responses(&request(second(), None)),
        responses(&request(second(), Some(TAIL))),
    );
    let at = shared(&a, &b);
    assert!(a[..at].ends_with('}'), "接在工具结果那一项后面：{b}");
    assert!(
        b[at..].starts_with(",{\"role\":\"user\""),
        "另起一条 user：{b}"
    );
    assert!(b[at..].contains(TAIL));
}

#[test]
fn anthropic_adds_a_text_block_and_marks_the_block_before_it() {
    for messages in [first(), second()] {
        let (a, b) = (
            claude_body(&request(messages.clone(), None)),
            claude_body(&request(messages, Some(TAIL))),
        );
        let plain = |body: &str| body.replace(",\"cache_control\":{\"type\":\"ephemeral\"}", "");
        inserted(
            &plain(&a),
            &plain(&b),
            &format!(",{{\"type\":\"text\",\"text\":\"{TAIL}\"}}"),
        );
        // 最后那个打点打在那一句前面那一块上：和不带的那一份打在同一处，到那里一个字节都不差。
        let mark = "\"cache_control\":{\"type\":\"ephemeral\"}}";
        let last = a.rfind(mark).expect("不带的打了点") + mark.len();
        assert_eq!(&b[..last], &a[..last], "{b}");
        assert!(
            b.rfind(mark).unwrap() < b.find(TAIL).unwrap(),
            "那一句后面不再打点：{b}"
        );
    }
}

/// 一家驱动：编成字。
type Encoder = fn(&Request) -> String;

/// 去掉 anthropic 的打点：打点挪来挪去不影响前缀（现成的探针也这样比）。
fn plain(body: &str) -> String {
    body.replace(",\"cache_control\":{\"type\":\"ephemeral\"}", "")
}

/// 工具第二步接着用上一步存的：上一步那一句前面的字节，这一步一个不少；只丢那一句。
#[test]
fn the_next_step_keeps_everything_before_the_tail() {
    let encoders: [(&str, Encoder); 3] = [
        ("openai-chat", chat),
        ("responses", responses),
        ("anthropic", |request| plain(&claude_body(request))),
    ];
    for (name, encode) in encoders {
        let step1 = encode(&request(first(), Some(TAIL)));
        let step2 = encode(&request(second(), Some(TAIL)));
        let before_tail = step1.find(TAIL).expect("带着那一句");
        let kept = shared(&step1, &step2);
        // 前面接的分隔（openai-chat、responses 的 `\n`，anthropic 那一块的开头）跟着那一句走：最多差那么几个字节。
        let joint = ",{\"type\":\"text\",\"text\":\"".len();
        assert!(
            kept + joint >= before_tail,
            "{name}：只差那一句（共 {kept}，那一句从 {before_tail} 起）"
        );
    }
    // anthropic：上一步最后那个打点（那一句前面那一块），这一步在同一处也打了，存到那里的接得上。
    let step1 = claude_body(&request(first(), Some(TAIL)));
    let step2 = claude_body(&request(second(), Some(TAIL)));
    let mark = ",\"cache_control\":{\"type\":\"ephemeral\"}";
    let upto = plain(&step1[..step1.rfind(mark).unwrap()]);
    let found = step2
        .match_indices(mark)
        .any(|(at, _)| plain(&step2[..at]) == upto);
    assert!(found, "这一步在上一步最后那个打点的同一处打了点：{step2}");
}
