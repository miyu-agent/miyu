use miyu_kernel::event::Event;
use miyu_kernel::id::ContentHash;

use super::references;

/// 一条工具结果：`sent` 里发出去一张图 `sent`（施工 X-19），内容块里没有图。
fn sent_a_sticker(sent: &ContentHash) -> Event {
    let line = format!(
        r#"{{"seq":9,"at":"2026-10-17T12:00:00.000Z","kind":"tool.result","turn":5,"by":{{"kind":"tool","call_id":"call_7_1"}},"cause":"cmd-1","body":{{"call_id":"call_7_1","status":"ok","blocks":[{{"type":"text","text":"sent"}}],"sent":[{{"blob":"{sent}","media_type":"image/png","width":8,"height":8,"alt":"开心","sticker":true}}]}}}}"#
    );
    Event::from_line(&line).expect("读得出")
}

/// 她经工具发出去的图（`tool.result.sent`）算会话结构引用过的（施工 W-6 再补，接入QQ 发表情包要读主人私聊里的）；
/// 别的 blob 照样不算。
#[test]
fn a_sent_image_counts_as_referenced() {
    let pic = ContentHash::of(b"sticker");
    let other = ContentHash::of(b"other");
    let event = sent_a_sticker(&pic);
    assert!(references(&event, &pic), "发出去的图算引用过");
    assert!(!references(&event, &other), "别的不算");
}
