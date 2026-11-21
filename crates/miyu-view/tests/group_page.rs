//! 群聊会话页要的三样（施工 FX-16）：翻页时切点前的人格、预设、权限级别照学（学了的和整份喂的一样）；没下载的平台媒体在 `user`
//! 条目里带种类（`media`）；触发这一轮的那一条带 `trigger: true`，顺带收进这一轮的旁听不带。

use std::sync::Arc;

use miyu_kernel::event::{Body as EventBody, Event};
use miyu_kernel::testkit::Line;
use miyu_view::{Entry, Projector};

use crate::support::*;

const CREATED: &str = r#"{"seq":1,"at":"2026-11-22T09:00:00.000Z","kind":"session.created","by":{"kind":"person"},"cause":"cmd-0","body":{"venue":"qq:group:777","policy":"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855","permission":{"level":"full","read_only":false},"persona":"persona-1","preset":"full"}}"#;

/// 群里有人说的第 `seq` 条：几块内容照 `blocks`，平台的格照 `venue`（JSON 对象）。
fn said(seq: u64, blocks: &str, venue: &str) -> String {
    format!(
        r#"{{"seq":{seq},"at":"2026-11-22T09:00:{seq:02}.000Z","kind":"message.user","by":{{"kind":"external","venue":"qq:group:777","id":"qq:20017","role":"member"}},"cause":"cmd-{seq}","body":{{"blocks":{blocks},"venue":{venue}}}}}"#
    )
}

/// 内核记的第 `seq` 条，种类 `kind`、属于第 `turn` 轮、内容 `body`。
fn kernel(seq: u64, kind: &str, turn: u64, body: &str) -> String {
    format!(
        r#"{{"seq":{seq},"at":"2026-11-22T09:00:{seq:02}.000Z","kind":"{kind}","turn":{turn},"by":{{"kind":"kernel"}},"cause":"cmd-{seq}","body":{body}}}"#
    )
}

/// 照先后喂这几行，交回条目。
fn fed(lines: &[String]) -> Vec<Entry> {
    let mut projector = Projector::new(Arc::new(texts("zh")), None);
    for line in lines {
        projector.event(&Event::from_line(line).unwrap_or_else(|e| panic!("{line}: {e}")));
    }
    projector.entries().to_vec()
}

/// 条目里的人格、预设、权限级别：回复的 `persona`、换人格的旁白、消息的 `level`，照编号。
fn cast(entries: &[Entry]) -> Vec<(String, serde_json::Value)> {
    entries
        .iter()
        .map(json)
        .filter_map(|e| {
            let id = e["id"].as_str().unwrap_or_default().to_string();
            match e["kind"].as_str() {
                Some("reply") => Some((id, e["persona"].clone())),
                Some("user") => Some((id, e["level"].clone())),
                Some("notice") if e["what"] == "persona" => Some((id, e.clone())),
                _ => None,
            }
        })
        .collect()
}

/// 翻页时切点前选的人格、预设，记着还没换上的，和权限级别，投影都照学：从每一句话切开，这一页里回复的 `persona`、换人格的旁白、
/// 消息的 `level` 和整份喂的那几条一样。改之前切点前的 `session.created`、`session.policy_changed` 不学，回复的 `persona` 全是
/// `null`、换上时的旁白写成从无人格换的。
#[test]
fn a_page_cut_later_keeps_the_persona_preset_and_level() {
    let mut stage = stage();
    stage.model([
        Line::says("one"),
        Line::says("two"),
        Line::says("three"),
        Line::says("four"),
    ]);
    stage.say("first");
    stage.recast(Some("persona-1"));
    stage.represet("chat");
    stage.swap_policy(
        "sha256:1111111111111111111111111111111111111111111111111111111111111111",
        policy,
    );
    stage.say("second");
    stage.recast(Some("persona-2"));
    stage.say("third");
    stage.swap_policy(
        "sha256:2222222222222222222222222222222222222222222222222222222222222222",
        policy,
    );
    stage.say("fourth");
    let log = stage.log();
    let whole = cast(&page(&stage));
    assert!(
        whole.iter().any(|(_, v)| v == "persona-2"),
        "整份的最后换成了 persona-2：{whole:?}"
    );
    let cuts = log
        .iter()
        .enumerate()
        .filter(|(_, event)| matches!(event.body, EventBody::MessageUser(_)))
        .map(|(cut, _)| cut);
    for cut in cuts.skip(1) {
        let mut projector = Projector::new(Arc::new(texts("en")), None);
        let page: Vec<&Event> = log[cut..].iter().collect();
        projector.feed_page(&log[..cut], &page);
        let paged = cast(projector.entries());
        let expected: Vec<_> = whole
            .iter()
            .filter(|(id, _)| paged.iter().any(|(seen, _)| seen == id))
            .cloned()
            .collect();
        assert!(!paged.is_empty());
        assert_eq!(paged, expected, "从第 {cut} 条切开");
    }
}

/// 没下载的平台媒体（`venue.media`，内容块里没有）：`user` 条目的 `media` 照先后带种类，表情、文件带上名字、大小；原图已经当图片块
/// 交进来的（`shown`）在 `attachments` 里，不再写一遍；什么都没带的不写。读回来一样。
#[test]
fn a_venue_message_names_the_media_it_carries() {
    let entries = fed(&[
        CREATED.to_string(),
        said(
            2,
            "[]",
            r#"{"msg":"1","media":[{"kind":"image","id":"A.png"},{"kind":"sticker","id":"s1","name":"[捂脸]"},{"kind":"video","id":"v1"},{"kind":"file","id":"f1","name":"a.pdf","size":2048},{"kind":"image","id":"B.png","shown":true}],"ambient":true}"#,
        ),
        said(
            3,
            r#"[{"type":"text","text":"在吗"}]"#,
            r#"{"msg":"2","ambient":true}"#,
        ),
    ]);
    let users = of_kind(&entries, "user");
    let carried = json(users[0]);
    assert_eq!(carried["text"], "");
    assert_eq!(
        carried["media"],
        serde_json::json!([
            {"kind": "image"},
            {"kind": "sticker", "name": "[捂脸]"},
            {"kind": "video"},
            {"kind": "file", "name": "a.pdf", "size": 2048},
        ]),
        "{carried}"
    );
    assert!(json(users[1]).get("media").is_none(), "没带的不写");
    for entry in &entries {
        round_trip(entry);
    }
}

/// 哪一条叫的她：`turn.started` 的 `trigger`、并进来的那一批（`turn.joined`）的最后一条带 `trigger: true`；照记下的几条开轮时一起
/// 收进这一轮的旁听（`triggers` 里别的）只有 `turn`，不带；没进哪一轮的也不带。读回来一样。
#[test]
fn only_the_message_that_called_her_is_marked_as_the_trigger() {
    let ambient = |seq: u64| {
        said(
            seq,
            r#"[{"type":"text","text":"在吗"}]"#,
            &format!(r#"{{"msg":"{seq}","ambient":true}}"#),
        )
    };
    let entries = fed(&[
        CREATED.to_string(),
        ambient(2),
        ambient(3),
        ambient(4),
        ambient(5),
        kernel(6, "turn.started", 6, r#"{"trigger":5,"triggers":[3,4,5]}"#),
        ambient(7),
        ambient(8),
        kernel(9, "turn.joined", 6, r#"{"triggers":[7,8]}"#),
        kernel(10, "turn.ended", 6, r#"{"reason":"completed"}"#),
    ]);
    let marks: Vec<(String, serde_json::Value, serde_json::Value)> = of_kind(&entries, "user")
        .into_iter()
        .map(json)
        .map(|e| {
            (
                e["id"].as_str().unwrap_or_default().to_string(),
                e["turn"].clone(),
                e["trigger"].clone(),
            )
        })
        .collect();
    let null = serde_json::Value::Null;
    let yes = serde_json::json!(true);
    let six = serde_json::json!(6);
    assert_eq!(
        marks,
        [
            ("m2".to_string(), null.clone(), null.clone()),
            ("m3".to_string(), six.clone(), null.clone()),
            ("m4".to_string(), six.clone(), null.clone()),
            ("m5".to_string(), six.clone(), yes.clone()),
            ("m7".to_string(), six.clone(), null),
            ("m8".to_string(), six, yes),
        ],
        "{entries:#?}"
    );
    for entry in &entries {
        round_trip(entry);
    }
}
