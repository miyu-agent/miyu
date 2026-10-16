//! 她发出去的图（施工 X-19）：`tool.result.sent` 一张一条 `media`，排在那一步工具后面，把前面那段时间线收起；图一定带宽高。

use miyu_kernel::block::Image;
use miyu_kernel::event::Sent;
use miyu_kernel::id::{ContentHash, MediaType};
use miyu_kernel::testkit::{Line, Play};
use std::sync::Arc;

use miyu_view::{Body, Change, Projector};

use crate::support::*;

fn sticker() -> Sent {
    Sent {
        image: Image {
            blob: ContentHash::of(b"cat"),
            name: None,
            media_type: MediaType::parse("image/png").expect("合写法"),
            width: 120,
            height: 96,
            path: None,
        },
        alt: Some("A cat waving.".to_string()),
        sticker: true,
    }
}

#[test]
fn a_sent_image_follows_its_step_and_closes_the_group() {
    let mut stage = stage();
    stage.model([Line::calls("", &[("read", r#"{"file_path":"a"}"#)])]);
    stage.tools([Play::Sends("Sent m1.".to_string(), vec![sticker()])]);
    stage.say("hi");
    let (_, changes) = live(&stage);
    let added = changes
        .iter()
        .position(
            |c| matches!(c, Change::Add { entry, .. } if matches!(entry.body, Body::Media(_))),
        )
        .expect("加了图那一条");
    let closed = changes[..added].iter().any(|c| {
        matches!(c, Change::Update { entry, .. } if matches!(&entry.body, Body::Group(g) if !g.open))
    });
    assert!(closed, "图一出来，前面那段时间线就收起：{changes:#?}");
    let entries = same(&stage);
    let kinds: Vec<String> = entries
        .iter()
        .map(|entry| json(entry)["kind"].as_str().unwrap_or_default().to_string())
        .collect();
    let tool = kinds
        .iter()
        .position(|kind| kind == "tool")
        .expect("有工具那一步");
    let media = kinds
        .iter()
        .position(|kind| kind == "media")
        .expect("有图那一条");
    assert!(tool < media, "排在那一步后面：{kinds:?}");
    let group = json(of_kind(&entries, "group")[0]);
    assert!(group.get("open").is_none(), "时间线收起了：{group}");
    let media = json(of_kind(&entries, "media")[0]);
    assert_eq!(media["image"]["width"], 120, "{media}");
    assert_eq!(media["image"]["height"], 96);
    assert_eq!(media["image"]["kind"], "image");
    assert_eq!(media["alt"], "A cat waving.");
    assert_eq!(media["sticker"], true);
    assert!(matches!(
        entries[media_index(&entries)].body,
        Body::Media(_)
    ));
}

fn media_index(entries: &[miyu_view::Entry]) -> usize {
    entries
        .iter()
        .position(|entry| matches!(entry.body, Body::Media(_)))
        .expect("有图那一条")
}

/// 收起那一行的字：每一格接起来。
fn summary(group: &serde_json::Value, key: &str) -> String {
    group[key]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part["text"].as_str())
        .collect()
}

/// 一种情况：调的几件工具和参数、第一件发的图、中文那一行的开头、英文那一行的开头（空的是照旧数）。
type Case<'a> = (&'a [(&'a str, &'a str)], Vec<Sent>, &'a str, &'a str);

/// 只为发一张图的那一段（施工 X-24）：工具全是同一个包的、一共发了一张，收起那一行打头写那件工具的显示名，英文的写英文的；
/// 混了别的包的工具、发了不止一张的照旧数。
#[test]
fn a_group_that_only_sends_one_image_is_named_after_its_tool() {
    let cases: [Case; 3] = [
        (
            &[("read", r#"{"file_path":"a"}"#)],
            vec![sticker()],
            "读取 · ",
            "Read · ",
        ),
        (
            &[
                ("read", r#"{"file_path":"a"}"#),
                ("shell", r#"{"command":"ls"}"#),
            ],
            vec![sticker()],
            "",
            "",
        ),
        (
            &[("read", r#"{"file_path":"a"}"#)],
            vec![sticker(), sticker()],
            "",
            "",
        ),
    ];
    for (calls, sent, local, english) in cases {
        let mut stage = stage();
        stage.model([Line::calls("", calls)]);
        let mut plays = vec![Play::Sends("Sent.".to_string(), sent.clone())];
        plays.extend(calls.iter().skip(1).map(|_| Play::Done("ok".to_string())));
        stage.tools(plays);
        stage.say("hi");
        // 连接是中文的：本地那一份是中文、英文那一份是英文。
        let mut projector = Projector::new(Arc::new(texts("zh")), None);
        for event in stage.log() {
            projector.event(event);
        }
        let entries = projector.entries().to_vec();
        let group = json(of_kind(&entries, "group")[0]);
        let (zh, en) = (summary(&group, "summary"), summary(&group, "summary_en"));
        if local.is_empty() {
            assert!(
                !zh.starts_with("读取") && !en.starts_with("Read"),
                "照旧数：{zh} / {en}"
            );
        } else {
            assert!(zh.starts_with(local), "{zh}");
            assert!(en.starts_with(english), "{en}");
            assert_eq!(zh.matches(" · ").count(), 1, "名字后面只接用时：{zh}");
        }
    }
}
