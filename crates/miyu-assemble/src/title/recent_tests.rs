//! 人要求重起标题的请求（施工 TT-1，[`recent`]）：指令、上限和自动起的同一份，喂最近答过的至多三轮，没有第一轮；只答过
//! 一轮的和自动起的逐字节一样；工具、思考、中间一步说的不要；最新那一轮还没答的不带；放不下的先整轮去掉最老的，只剩一轮
//! 还放不下的截中间。自动起的那一份（[`request`]）在同一段历史上照旧只喂第一轮。

use miyu_kernel::block::Block;
use miyu_kernel::request::Message;

use super::*;
use crate::test_support::{Log, text_json, texts, title_texts};
use crate::texts::Title;

/// 一块字的回复的内容块。
fn words(text: &str) -> String {
    format!("[{}]", text_json(text))
}

/// 完整的一轮：人说 `said`，她答 `answer`。交回回答的序号。
fn turn(log: &mut Log, said: &str, answer: &str) -> u64 {
    let asked = log.say(said);
    log.start(asked);
    let answered = log.reply(&words(answer));
    log.end("completed");
    answered
}

/// 请求里那一条 user 的字。
fn text_of(request: &Request) -> String {
    assert!(request.tools.is_empty());
    assert_eq!(request.system, "");
    assert_eq!((request.stable, request.continuation), (0, false));
    let [Message::User { blocks }] = request.messages.as_slice() else {
        panic!("只有一条 user：{:?}", request.messages);
    };
    let [Block::Text(text)] = blocks.as_slice() else {
        panic!("一块字：{blocks:?}");
    };
    text.text.clone()
}

/// 五轮的会话。交回最后一个回答的序号。
fn five_turns(log: &mut Log) -> u64 {
    let mut last = 0;
    for n in ["一", "二", "三", "四", "五"] {
        last = turn(log, &format!("问{n}"), &format!("答{n}"));
    }
    last
}

#[test]
fn the_last_three_turns_go_in_and_the_first_does_not() {
    let mut log = Log::new();
    let last = five_turns(&mut log);
    let (request, upto) = recent(log.history(), &texts()).unwrap();
    assert_eq!(upto.get(), last, "照到的是最新那个回答");
    assert_eq!(
        text_of(&request),
        "<title>\nU: 问三\n\nA: 答三\n\nU: 问四\n\nA: 答四\n\nU: 问五\n\nA: 答五"
    );
    // 自动起的那一份在同一段历史上照旧只喂第一轮。
    let (auto, _) = super::request(log.history(), &texts()).unwrap();
    assert_eq!(text_of(&auto), "<title>\nU: 问一\n\nA: 答一");
}

#[test]
fn with_one_turn_it_is_the_request_the_first_title_used() {
    let mut log = Log::new();
    turn(&mut log, "看看 src", "src 下有两个文件。");
    assert_eq!(
        recent(log.history(), &texts()),
        super::request(log.history(), &texts())
    );
}

/// 工具调用、工具结果、思考、中间一步说的、注入的事实都不要；最新那一轮还没答的不带。
#[test]
fn only_the_words_of_answered_turns_go_in() {
    let mut log = Log::new();
    turn(&mut log, "在吗", "在。");
    let asked = log.say("读一下 lib.rs");
    log.start(asked);
    log.fact("<env/>");
    let call = log.reply_calling("我先读一下。");
    log.result(&call, "ok", "pub mod assemble;");
    let answered = log.reply(
        r#"[{"type":"reasoning","text":"想一想"},{"type":"text","text":"  只有一个模块。\n"}]"#,
    );
    log.end("completed");
    let asked = log.say("再读 main.rs");
    log.start(asked);
    log.reply_calling("好，我读。");
    let (request, upto) = recent(log.history(), &texts()).unwrap();
    assert_eq!(
        text_of(&request),
        "<title>\nU: 在吗\n\nA: 在。\n\nU: 读一下 lib.rs\n\nA: 只有一个模块。"
    );
    assert_eq!(upto.get(), answered, "没答完的那一轮不算");
}

/// 整份连指令至多 `tokens` 个 token：先整轮去掉最老的、最前写一行省略了；只剩一轮还放不下的截中间。
#[test]
fn over_the_limit_the_oldest_turns_go_first() {
    let mut log = Log::new();
    for (said, answer) in [("a", "b"), ("c", "d"), ("e", "f")] {
        turn(&mut log, &said.repeat(10), &answer.repeat(10));
    }
    let mut texts = texts();
    // 指令 8 字节，一轮 28 字节，三轮连空行 88 字节。限 20 个 token 是 80 字节：去掉最老的一轮，剩两轮连省略的那一行 69 字节。
    texts.title = Some(Title {
        tokens: 20,
        ..title_texts()
    });
    let (request, _) = recent(log.history(), &texts).unwrap();
    assert_eq!(
        text_of(&request),
        format!(
            "<title>\n[omitted]\n\nU: {}\n\nA: {}\n\nU: {}\n\nA: {}",
            "c".repeat(10),
            "d".repeat(10),
            "e".repeat(10),
            "f".repeat(10)
        )
    );
    // 限 10 个 token 是 40 字节：只剩最新一轮还放不下，两段截中间。
    texts.title = Some(Title {
        tokens: 10,
        ..title_texts()
    });
    let text = text_of(&recent(log.history(), &texts).unwrap().0);
    assert!(text.len() <= 40, "{text:?}");
    assert!(text.starts_with("<title>\n[omitted]\n\nU: "), "{text:?}");
    assert!(text.contains("[...]") && text.contains("A: f"), "{text:?}");
    assert!(text.ends_with('f'), "回答的尾巴留着：{text:?}");
    assert!(
        !["aa", "bb", "cc", "dd"]
            .iter()
            .any(|old| text.contains(old)),
        "{text:?}"
    );
}

/// 没有回答的、快照里没有起标题的字或者回顾的标签的，都没有。
#[test]
fn nothing_without_an_answer_or_the_texts() {
    let mut log = Log::new();
    log.say("看看 src");
    assert!(recent(log.history(), &texts()).is_none());
    let mut log = Log::new();
    turn(&mut log, "一", "答一");
    for old in [
        Texts {
            title: None,
            ..texts()
        },
        Texts {
            recap: None,
            ..texts()
        },
    ] {
        assert!(recent(log.history(), &old).is_none(), "以前造的快照");
    }
}
