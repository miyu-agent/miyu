//! 她发出去的图（施工 X-19）：`tool.result.sent` 一张一条 `media`，排在那一步工具后面，把前面那段时间线收起；图一定带宽高。

use miyu_kernel::block::Image;
use miyu_kernel::event::Sent;
use miyu_kernel::id::{ContentHash, MediaType};
use miyu_kernel::testkit::{Line, Play};
use miyu_view::{Body, Change};

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
