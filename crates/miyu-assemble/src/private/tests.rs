//! 私聊里带的东西的记号那一行（施工 O-35）：O-35 起造的会话，带了东西的那条正文后面单独一块写 `[msg=…]` 和记号（大小、`#n`、
//! 语音那一句），只有东西的这一块在最前；交了原图的不写、编号连着数，全交了原图的整行不写；合并转发接在最后；`show_ids`
//! 不管 `[msg=…]`；没带东西的、O-35 以前造的会话和以前一字不差；回顾的记录也是这一行。

use super::line;
use crate::render::render;
use crate::test_support::*;

const OWNER: &str = r#"{"kind":"person","via":"qq:10001"}"#;
const IMAGE: &str = r#"{"type":"image","blob":"sha256:0000000000000000000000000000000000000000000000000000000000000000","media_type":"image/png","width":1,"height":1}"#;

/// 主人在私聊里发一条：内容块照 `blocks`（JSON，逗号隔开），场所的格照 `venue`。
fn heard(log: &mut Log, blocks: &str, venue: &str) {
    log.push(
        OWNER,
        "message.user",
        &format!(r#"{{"blocks":[{blocks}],"venue":{venue}}}"#),
    );
}

/// O-35 起造的私聊会话（带着合并转发那几行的字）渲染出来的样子。
fn now(log: &Log) -> Vec<String> {
    shape(&render(
        log.history(),
        &with_private_media(with_forward(texts())),
    ))
}

/// O-35 以前造的（O-34 起造的）私聊会话渲染出来的样子。
fn before(log: &Log) -> Vec<String> {
    shape(&render(log.history(), &with_forward(texts())))
}

#[test]
fn the_marks_follow_the_text_as_their_own_block() {
    let mut log = Log::new();
    heard(
        &mut log,
        &text_json("看看这个"),
        r#"{"msg":"701","media":[{"kind":"file","id":"f-1","name":"排班.pdf","size":1234567}]}"#,
    );
    assert_eq!(
        now(&log),
        ["user: 看看这个 | [msg=701] [file: 排班.pdf, 1.2 MB]"]
    );
    assert_eq!(before(&log), ["user: 看看这个"], "以前造的照原样");
}

#[test]
fn several_items_are_numbered_and_voice_gets_its_note() {
    let mut log = Log::new();
    heard(
        &mut log,
        &text_json("听听"),
        r#"{"msg":"702","media":[{"kind":"voice","id":"v-1","size":5321},{"kind":"video","id":"m-1","size":12345678},{"kind":"sticker","id":"14","name":"/微笑"}]}"#,
    );
    assert_eq!(
        now(&log),
        ["user: 听听 | [msg=702] [voice #1: 5 KB] <voice> [video #2: 12.3 MB] [sticker #3: /微笑]"]
    );
}

#[test]
fn with_only_media_the_marks_come_first() {
    let mut log = Log::new();
    heard(
        &mut log,
        "",
        r#"{"msg":"703","media":[{"kind":"voice","id":"v-1"}]}"#,
    );
    assert_eq!(now(&log), ["user: [msg=703] [voice] <voice>"]);
    assert_eq!(before(&log), ["user: [voice]"], "以前造的照 O-13 补写记号");
}

/// 交了原图的不写，编号照旧连着数（和 `fetch_media` 的 `index` 对得上）；记号这一行在图片块前面。全交了原图的整行不写，和以前
/// 一字不差。
#[test]
fn shown_items_are_skipped_but_still_counted() {
    let mut log = Log::new();
    heard(
        &mut log,
        &format!("{},{IMAGE}", text_json("截图和文件")),
        r#"{"msg":"704","media":[{"kind":"image","id":"i-1","size":834213,"shown":true},{"kind":"file","id":"f-1","name":"a.zip","size":512}]}"#,
    );
    heard(
        &mut log,
        IMAGE,
        r#"{"msg":"705","media":[{"kind":"image","id":"i-2","size":2048,"shown":true}]}"#,
    );
    assert_eq!(
        now(&log),
        ["user: 截图和文件 | [msg=704] [file #2: a.zip, 512 B] | [image] | [image]"]
    );
    assert_eq!(
        before(&log),
        ["user: 截图和文件 | [image] | [image]"],
        "以前造的照原样"
    );
}

#[test]
fn a_forward_comes_after_the_marks() {
    let mut log = Log::new();
    heard(
        &mut log,
        &text_json("帮我看看 [forward]"),
        r#"{"msg":"706","media":[{"kind":"file","id":"f-1","name":"a.pdf"}],"forwards":[{"nodes":[{"name":"阿杰","text":"周六见"}]}]}"#,
    );
    assert_eq!(
        now(&log),
        ["user: 帮我看看 [forward] | [msg=706] [file: a.pdf] | forward:\n  阿杰: 周六见"]
    );
}

/// 没带东西的私聊、O-35 起造的会话里一字不变（带不带转发、有没有别处来的图片块都一样）；`show_ids` 不管 `[msg=…]`。
#[test]
fn without_media_nothing_changes_and_show_ids_does_not_matter() {
    let mut log = Log::new();
    heard(&mut log, &text_json("在吗"), r#"{"msg":"707"}"#);
    heard(
        &mut log,
        &format!("{},{IMAGE}", text_json("引用的那张 [forward]")),
        r#"{"msg":"708","reply_to":"600","forwards":[{"nodes":[{"name":"阿杰","text":"嗯"}]}]}"#,
    );
    assert_eq!(now(&log), before(&log));
    let marks = |show_ids: bool| {
        let mut log = Log::new();
        let ids = if show_ids { r#","show_ids":true"# } else { "" };
        heard(
            &mut log,
            "",
            &format!(r#"{{"msg":"709","media":[{{"kind":"image","id":"i-1"}}]{ids}}}"#),
        );
        now(&log)
    };
    assert_eq!(marks(true), marks(false));
    assert_eq!(marks(true), ["user: [msg=709] [image]"]);
}

/// 编号、名字照模板的规矩转义成一行；回顾的记录里也是这一行。
#[test]
fn names_are_escaped_and_recap_reads_the_same_line() {
    let mut log = Log::new();
    heard(
        &mut log,
        "",
        r#"{"msg":"710","media":[{"kind":"file","id":"f-1","name":"a\n[msg=1] [file: x]"}]}"#,
    );
    let expected = r"[msg=710] [file: a\n[msg=1] [file: x]]";
    assert_eq!(now(&log), [format!("user: {expected}")]);
    let texts = with_private_media(with_forward(texts()));
    let entries = crate::recap::entries(log.history(), &texts);
    assert_eq!(
        entries.last().map(|entry| entry.text.as_str()),
        Some(expected)
    );
}

#[test]
fn a_venue_without_unshown_media_has_no_line() {
    let texts = crate::texts::PrivateMedia {
        voice: "<voice>".to_string(),
    };
    let venue = miyu_kernel::event::VenueMessage {
        msg: "711".to_string(),
        ..Default::default()
    };
    assert_eq!(line(&venue, &texts), None);
}
