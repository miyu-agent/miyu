//! 场景：回复写到了输出上限（施工 3-10，`session/truncated.rs`）。截掉了工具调用的一个都不派、跟一句拆小了接着做；
//! 只截在正文里的照被打断的回复接着写；什么正文都没写出来的同截掉调用；马上再请求，不算步数；连着 3 次以后再写到上限
//! 照出错结束；以前造的快照没有那一句，照样不派、再请求。

use super::*;
use crate::event::MessageAssistant;
use crate::facts::FactTemplates;

/// 带上写到上限那一句的替身：`<output-limit/>`。
fn limited() -> Stage {
    let with = || {
        let mut policy = policy();
        policy.facts = FactTemplates::new(
            r#"<e t="{time}" z="{timezone}" d="{cwd}"/>"#,
            r#"<p l="{level}"/>"#,
            "<reply-cut/>",
            None,
            None,
        )
        .and_then(|facts| facts.with_output_limit(Some("<output-limit/>")))
        .unwrap();
        policy
    };
    Stage::new(with, environment("~/src/miyu"), at(0))
}

/// 写一个文件，参数截在半截。
fn cut_write() -> Line {
    Line::calls("我来写", &[("write", r#"{"path":"a.md","text":"很长"#)]).truncated()
}

fn assistant(stage: &Stage, index: usize) -> &MessageAssistant {
    let Body::MessageAssistant(reply) = &stage.log()[index].body else {
        panic!("第 {} 条是回复", index + 1);
    };
    reply
}

#[test]
fn cut_tool_calls_are_not_run_and_she_is_told_to_split() {
    let mut stage = limited();
    stage.model([cut_write(), Line::says("拆开写完了。")]);
    stage.say("写个长文件");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 context.injected:output_limit kernel t3",
            "9 message.assistant model t3",
            "10 model.called:ok kernel t3",
            "11 turn.ended:completed kernel t3",
        ])
    );
    let cut = assistant(&stage, 5);
    assert!(cut.interrupted);
    assert_eq!(
        cut.blocks,
        [Block::Text(Text {
            text: "我来写".to_string()
        })],
        "调用一个都不留"
    );
    let Body::ModelCalled(called) = &stage.log()[6].body else {
        panic!("7 号是模型调用");
    };
    assert!(called.truncated);
    let Body::ContextInjected(fact) = &stage.log()[7].body else {
        panic!("8 号是那一句");
    };
    assert_eq!(fact.text, "<output-limit/>");
    assert_eq!(stage.requests().len(), 2, "马上再请求");
    let Body::ModelCalled(called) = &stage.log()[9].body else {
        panic!("10 号是模型调用");
    };
    assert!(!called.truncated);
}

#[test]
fn a_reply_cut_in_its_text_is_continued() {
    let mut stage = limited();
    stage.model([
        Line::says("很长的一段").truncated(),
        Line::says("接着说完。"),
    ]);
    stage.say("讲讲");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 context.injected:reply_cut kernel t3",
            "9 message.assistant model t3",
            "10 model.called:ok kernel t3",
            "11 turn.ended:completed kernel t3",
        ])
    );
    assert!(assistant(&stage, 5).interrupted);
}

#[test]
fn a_reply_with_only_thinking_counts_as_cut_work() {
    let mut stage = limited();
    stage.model([
        Line::says("").thinking("想了很久").truncated(),
        Line::says("好。"),
    ]);
    stage.say("hi");
    assert_eq!(
        story(&stage)[7],
        "8 context.injected:output_limit kernel t3"
    );
}

#[test]
fn an_empty_cut_reply_is_not_an_empty_reply_error() {
    let mut stage = limited();
    stage.model([Line::says("").truncated(), Line::says("好。")]);
    stage.say("hi");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 model.called:ok kernel t3",
            "7 context.injected:output_limit kernel t3",
            "8 message.assistant model t3",
            "9 model.called:ok kernel t3",
            "10 turn.ended:completed kernel t3",
        ])
    );
}

#[test]
fn three_in_a_row_are_carried_on_and_the_fourth_ends_the_turn() {
    let mut stage = limited();
    stage.model([cut_write(), cut_write(), cut_write(), Line::says("好了。")]);
    stage.say("写");
    assert_eq!(stage.requests().len(), 4);
    assert!(
        story(&stage)
            .last()
            .unwrap()
            .ends_with("turn.ended:completed kernel t3")
    );

    let mut stage = limited();
    stage.model([cut_write(), cut_write(), cut_write(), cut_write()]);
    stage.say("写");
    assert_eq!(stage.requests().len(), 4, "第四次不再来");
    let story = story(&stage);
    assert!(story.ends_with(&[
        "15 message.assistant model t3".to_string(),
        "16 model.called:error kernel t3".to_string(),
        "17 turn.ended:error kernel t3".to_string(),
    ]));
    let Body::ModelCalled(called) = &stage.log()[15].body else {
        panic!("16 号是模型调用");
    };
    let error = called.error.as_ref().expect("出错收");
    assert_eq!(error.class, ErrorClass::OutputLimit);
    assert!(called.truncated);
    assert!(
        assistant(&stage, 14)
            .blocks
            .iter()
            .all(|block| !matches!(block, Block::ToolCall(_))),
        "最后那一次的调用也不派"
    );
}

#[test]
fn a_finished_reply_in_between_starts_the_count_again() {
    let mut stage = limited();
    stage.model([
        cut_write(),
        cut_write(),
        cut_write(),
        Line::calls("", &[("read", r#"{"path":"a.md"}"#)]),
        cut_write(),
        Line::says("好了。"),
    ]);
    stage.tools([Play::Done("hello".to_string())]);
    stage.say("写");
    assert_eq!(stage.requests().len(), 6);
    assert!(
        story(&stage)
            .last()
            .unwrap()
            .ends_with("turn.ended:completed kernel t3")
    );
}

#[test]
fn carrying_on_is_not_a_step() {
    // 最多请求两次：写到上限以后接着的那一次不算，读完文件还能再请求一次，照常说完。
    let mut stage = Stage::new(
        || {
            let mut policy = policy();
            policy.step_limit = Some(2);
            policy
        },
        environment("~/src/miyu"),
        at(0),
    );
    stage.model([
        cut_write(),
        Line::calls("先看看。", &[("read", r#"{"path":"a.md"}"#)]),
        Line::says("说完了。"),
    ]);
    stage.tools([Play::Done("A".to_string())]);
    stage.say("hi");
    assert_eq!(stage.requests().len(), 3);
    assert!(
        story(&stage)
            .last()
            .unwrap()
            .ends_with("turn.ended:completed kernel t3")
    );
}

#[test]
fn an_older_snapshot_without_the_line_still_does_not_run_them() {
    let mut stage = stage();
    stage.model([cut_write(), Line::says("好了。")]);
    stage.say("写");
    assert_eq!(
        story(&stage),
        opening_then(&[
            "6 message.assistant model t3",
            "7 model.called:ok kernel t3",
            "8 message.assistant model t3",
            "9 model.called:ok kernel t3",
            "10 turn.ended:completed kernel t3",
        ])
    );
}

#[test]
fn a_cut_reply_still_counts_as_answered_for_retries() {
    let mut stage = limited();
    let mut lines = vec![Line::fails(ErrorClass::Retryable, "503"), cut_write()];
    lines.extend((0..5).map(|_| Line::fails(ErrorClass::Retryable, "503")));
    lines.push(Line::says("好了。"));
    stage.model(lines);
    stage.say("写");
    assert_eq!(
        stage.requests().len(),
        8,
        "写到上限那一次也算说完了：再来的次数重新数"
    );
    assert!(
        story(&stage)
            .last()
            .unwrap()
            .ends_with("turn.ended:completed kernel t3")
    );
}
