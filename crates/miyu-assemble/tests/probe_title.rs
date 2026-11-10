//! 请求形状探针：起标题的那一张脸（施工 3-8 五补，`docs/blueprint/kernel/request.md`「起标题的请求」）。真内核照剧本跑，
//! 组装、给模型看的字都是出厂的：人问一句，她读了文件答，答完内核起标题；人再问，她再答，不再起。
//!
//! 起标题的请求是单独的一次：一条 user，指令接第一轮的对话记录，没有 system、工具面；和存档（`docs/designs/samples/probe/
//! title/` 的 `titles/`）逐字节比。主请求照样查五条性质、和存档比：标题的两条夹在两轮中间，第二轮的请求照旧接着第一轮的
//! 往后长。

use crate::support::{check, files, matches_the_archive, sent, title, titled_stage};
use miyu_kernel::block::Block;
use miyu_kernel::event::Body;
use miyu_kernel::request::Message;
use miyu_kernel::testkit::{Line, Play, Stage};

/// 剧本。
fn script() -> Stage {
    let mut s = titled_stage();
    // 1. 人问一句，她读了文件答；答完起标题。
    s.model([
        Line::calls(
            "我读一下测试的临时目录。",
            &[("read", r#"{"path":"tests/support/mod.rs"}"#)],
        ),
        Line::says("CI 红在 macOS：临时目录在 /var 这个链接下面，安全打开不走链接，拒绝了。"),
    ]);
    s.tools([Play::done(
        "fn temp_dir() -> PathBuf { std::env::temp_dir() }",
    )]);
    s.title_model([Line::says("macOS 上 CI 变红的原因")]);
    s.say("CI 为什么红了？");
    // 2. 人再问，她再答：起过了，不再起。
    s.advance(2);
    s.model([Line::says("换成真实路径：先 canonicalize 再打开。")]);
    s.say("那怎么改？");
    s
}

#[test]
fn the_title_session_matches_the_archive() {
    matches_the_archive("title", &script());
}

#[test]
fn the_same_script_gives_the_same_bytes() {
    assert_eq!(files(&script()), files(&script()));
}

/// 起标题的请求：一条 user，指令在最前，后面是第一轮人的话和她那一轮最后的回答；工具的输出、中间一步说的不在里面。只起
/// 一次，标题记在日志里。主请求的五条性质照查。
#[test]
fn a_title_is_its_own_request_and_leaves_the_main_ones_alone() {
    let session = script();
    if let Err(why) = check(&sent(&session)) {
        panic!("{why}");
    }
    let [(_, request)] = session.titles() else {
        panic!("只起一次：{:?}", session.titles().len());
    };
    assert!(request.tools.is_empty() && request.system.is_empty());
    let [Message::User { blocks }] = request.messages.as_slice() else {
        panic!("一条 user：{:?}", request.messages);
    };
    let [Block::Text(text)] = blocks.as_slice() else {
        panic!("一块字：{blocks:?}");
    };
    let instruction = title().instruction;
    assert_eq!(
        text.text,
        format!(
            "{instruction}User: CI 为什么红了？\n\nAssistant: CI 红在 macOS：临时目录在 /var 这个链接下面，安全打开不走链接，拒绝了。"
        )
    );
    let titled = session.log().iter().any(|event| {
        matches!(&event.body, Body::MetaChanged(changed) if changed.title.as_deref() == Some("macOS 上 CI 变红的原因"))
    });
    assert!(titled, "标题记在日志里");
}

/// 一次起标题的请求里那一条 user 的字。
fn asked(request: &miyu_kernel::request::Request) -> &str {
    assert!(request.tools.is_empty() && request.system.is_empty());
    let [Message::User { blocks }] = request.messages.as_slice() else {
        panic!("一条 user：{:?}", request.messages);
    };
    let [Block::Text(text)] = blocks.as_slice() else {
        panic!("一块字：{blocks:?}");
    };
    &text.text
}

/// 说一句，她答一句就完。
fn one_turn(session: &mut Stage, said: &str, answer: &str) {
    session.model([Line::says(answer)]);
    session.say(said);
}

/// 人要求重起（施工 TT-1），真内核、出厂的字：五轮的会话，同一份指令接最后三轮，没有第一轮；带工具调用的那一轮只有人的话
/// 和她最后的正文，工具的输出、中间一步说的不在里面；自动起的那一次没有跟着变。
#[test]
fn asking_again_feeds_the_last_three_turns_under_the_same_instruction() {
    let mut session = script();
    let auto = session.titles()[0].1.clone();
    one_turn(&mut session, "改完跑一下测试", "跑过了，三个平台都是绿的。");
    session.model([
        Line::calls("我看一下提交。", &[("read", r#"{"path":"CHANGELOG.md"}"#)]),
        Line::says("写进了更新记录。"),
    ]);
    session.tools([Play::done("## 未发布")]);
    session.say("记进更新记录了吗？");
    one_turn(&mut session, "那发版吧", "好，打标签。");
    session.title_model([Line::says("修 macOS 上的临时目录")]);
    session.retitle();
    let [(_, first), (_, again)] = session.titles() else {
        panic!("两次起标题的请求：{:?}", session.titles().len());
    };
    assert_eq!(*first, auto, "自动起的那一次照旧");
    let instruction = title().instruction;
    assert_eq!(
        asked(again),
        format!(
            "{instruction}User: 改完跑一下测试\n\nAssistant: 跑过了，三个平台都是绿的。\n\nUser: 记进更新记录了吗？\n\nAssistant: 写进了更新记录。\n\nUser: 那发版吧\n\nAssistant: 好，打标签。"
        )
    );
}

/// 只答过一轮的会话重起：和自动起的那一次是同一份请求，逐字节。
#[test]
fn with_one_turn_asking_again_sends_the_request_the_first_title_used() {
    let mut session = titled_stage();
    session.title_model([Line::says("打个招呼"), Line::says("问好")]);
    one_turn(&mut session, "在吗", "在。");
    session.retitle();
    let [(first, auto), (second, again)] = session.titles() else {
        panic!("两次起标题的请求：{:?}", session.titles().len());
    };
    assert_eq!((first, auto), (second, again));
}

/// 三轮合起来超过上限（出厂连指令 1024 个 token，4096 字节）：整轮去掉最老的，最前写一行省略了；剩下的照原样。
#[test]
fn over_the_limit_asking_again_drops_the_oldest_turn() {
    let mut session = titled_stage();
    session.title_model([Line::says("打个招呼"), Line::says("长对话")]);
    let (old, middle, new) = ("甲".repeat(500), "乙".repeat(400), "丙".repeat(400));
    one_turn(&mut session, "一", &old);
    one_turn(&mut session, "二", &middle);
    one_turn(&mut session, "三", &new);
    session.retitle();
    let [_, (_, again)] = session.titles() else {
        panic!("两次起标题的请求：{:?}", session.titles().len());
    };
    let instruction = title().instruction;
    assert_eq!(
        asked(again),
        format!(
            "{instruction}[Earlier exchanges omitted]\n\nUser: 二\n\nAssistant: {middle}\n\nUser: 三\n\nAssistant: {new}"
        )
    );
    assert!(asked(again).len() <= 4096);
}
