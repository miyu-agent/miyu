//! 私聊里带的东西的记号（施工 O-35，`docs/construction/O-35-私聊看文件.md`）：O-35 起造的会话快照里钉下语音那一句（出厂的
//! 原文），私聊里带了东西的那条，请求里人那一条在正文的块后面单独一块写 `[msg=…]` 和记号：带图（交了原图的不写、编号连着数）、
//! 带文件、带语音、只有东西没有字、带东西又带转发，各一个请求字节的样例。照出厂的资源、真会话、整条消息的字节比。

use miyu_kernel::block::{Block, Image, Text};
use miyu_kernel::event::{Body, Forward, ForwardNode, Media, MediaKind, VenueMessage};
use miyu_kernel::id::{ContentHash, ExternalId, SessionId, VenueId};
use miyu_kernel::origin::{By, External};
use miyu_kernel::request::{Message, Request};
use miyu_kernel::session::Command;
use miyu_policy::Snapshot;
use miyu_session::Handle;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

use crate::support::*;

/// 语音那一句（出厂的原文，去掉行尾）。
const VOICE: &str = include_str!("../../../resources/core/venues/voice.txt");

/// 小林（`qq:20017`）在私聊里。
fn friend() -> By {
    By::External(External {
        venue: VenueId::parse("qq:private:20017").expect("场所合写法"),
        id: ExternalId::parse("qq:20017").expect("身份合写法"),
        owner: false,
        role: None,
    })
}

/// 带的一样东西。
fn item(kind: MediaKind, id: &str, name: Option<&str>, size: Option<u64>, shown: bool) -> Media {
    Media {
        kind,
        id: id.to_string(),
        name: name.map(str::to_string),
        size,
        shown,
        index: None,
    }
}

/// 一块图：blob 的内容是名字的字节（[`private_turns`] 放进属主名下），只看请求里块的字节。
fn picture(name: &str) -> Block {
    let image = serde_json::json!({"blob": ContentHash::of(name.as_bytes()), "name": name,
        "media_type": "image/png", "width": 4, "height": 3});
    Block::Image(serde_json::from_value::<Image>(image).expect("合写法"))
}

/// 一条私聊：正文 `words`（空的不带字块），后面接 `pictures`，场所的格编号 `msg`、带的东西 `media`、合并转发 `forwards`。
fn said(
    words: &str,
    pictures: Vec<Block>,
    msg: &str,
    media: Vec<Media>,
    forwards: Vec<Forward>,
) -> Command {
    let mut blocks = Vec::new();
    if !words.is_empty() {
        blocks.push(Block::Text(Text {
            text: words.to_string(),
        }));
    }
    blocks.extend(pictures);
    Command::Send {
        blocks,
        urgent: false,
        venue: Some(VenueMessage {
            msg: msg.to_string(),
            media,
            forwards,
            ..VenueMessage::default()
        }),
    }
}

/// 请求里最后一条 user 的最后 `count` 块的字节（前面是开头注入的事实）。
fn last_said(request: &Request, count: usize) -> String {
    let Some(Message::User { blocks }) = request
        .messages
        .iter()
        .rev()
        .find(|message| matches!(message, Message::User { .. }))
    else {
        panic!("有人说的话：{request:?}");
    };
    serde_json::to_string(&blocks[blocks.len() - count..]).expect("写得成 JSON")
}

/// 会话 `session` 的策略快照。
fn snapshot(home: &Home, session: &SessionId) -> Snapshot {
    let log = home.log(session);
    let Body::SessionCreated(created) = &log[0].body else {
        panic!("第 1 条应该是造会话：{:?}", log[0]);
    };
    let bytes = Blobs::new(home.root.blobs())
        .get(&created.policy)
        .expect("快照在 blob 里");
    Snapshot::from_bytes(&bytes).expect("读得懂")
}

/// 造一个私聊会话，照先后发 `messages`，每条一轮；交回每一轮的请求。
async fn private_turns(messages: Vec<Command>) -> (Home, SessionId, Vec<Request>) {
    let home = Home::new();
    // 图片块引用的 blob 放进属主名下（内容是假的，请求里只看块的字节）。
    let blobs = Blobs::new(home.root.blobs());
    for name in ["msg-801-1", "msg-805-1"] {
        blobs.put(name.as_bytes()).expect("存得下");
    }
    let script = Script::new(messages.iter().map(|_| Play::Says("嗯。")));
    let handle: Handle = home
        .create_full(
            &script,
            &Catalog::default(),
            Opening::default(),
            Lines {
                venue: VenueId::parse("qq:private:20017").expect("场所合写法"),
                group: false,
                ..Lines::default()
            },
        )
        .await;
    let session = handle.id().clone();
    for (n, message) in messages.into_iter().enumerate() {
        let mut pushes = watch(&handle).await;
        within(
            "回应",
            handle.command(id(&format!("say-{n}")), friend(), message),
        )
        .await
        .expect("会话在跑");
        until_turn_ends(&mut pushes).await;
    }
    stop(&handle).await;
    let requests = script
        .requests()
        .into_iter()
        .map(|(_, request)| request)
        .collect();
    (home, session, requests)
}

#[tokio::test]
async fn private_requests_carry_the_marks_after_the_text() {
    let voice = VOICE.trim_end();
    let messages = vec![
        // 带图：交了原图的不写，编号照旧连着数。
        said(
            "截图和文件",
            vec![picture("msg-801-1")],
            "801",
            vec![
                item(MediaKind::Image, "i-1", None, Some(834_213), true),
                item(
                    MediaKind::File,
                    "f-1",
                    Some("排班.pdf"),
                    Some(1_234_567),
                    false,
                ),
            ],
            Vec::new(),
        ),
        // 带文件。
        said(
            "看看",
            Vec::new(),
            "802",
            vec![item(
                MediaKind::File,
                "f-2",
                Some("a.zip"),
                Some(512),
                false,
            )],
            Vec::new(),
        ),
        // 带语音、只有东西没有字。
        said(
            "",
            Vec::new(),
            "803",
            vec![item(MediaKind::Voice, "v-1", None, Some(5321), false)],
            Vec::new(),
        ),
        // 带东西又带转发。
        said(
            "帮我看看 [forward]",
            Vec::new(),
            "804",
            vec![item(MediaKind::Video, "m-1", None, Some(12_345_678), false)],
            vec![Forward {
                nodes: vec![ForwardNode {
                    name: "阿杰".to_string(),
                    text: "周六见".to_string(),
                    media: Vec::new(),
                    forwards: Vec::new(),
                }],
                more: None,
            }],
        ),
        // 只有交了原图的图、没有字：记号那一行整行不写。
        said(
            "",
            vec![picture("msg-805-1")],
            "805",
            vec![item(MediaKind::Image, "i-5", None, Some(2048), true)],
            Vec::new(),
        ),
    ];
    let (home, session, requests) = private_turns(messages).await;
    let pinned = snapshot(&home, &session)
        .core
        .private_media
        .expect("O-35 起造的会话钉下了");
    assert_eq!(pinned.voice, VOICE);
    let image = |name: &str| serde_json::to_string(&picture(name)).expect("写得成 JSON");
    assert_eq!(
        last_said(&requests[0], 3),
        format!(
            r#"[{{"type":"text","text":"截图和文件"}},{{"type":"text","text":"[msg=801] [file #2: 排班.pdf, 1.2 MB]"}},{}]"#,
            image("msg-801-1")
        )
    );
    assert_eq!(
        last_said(&requests[1], 2),
        r#"[{"type":"text","text":"看看"},{"type":"text","text":"[msg=802] [file: a.zip, 512 B]"}]"#
    );
    assert_eq!(
        last_said(&requests[2], 1),
        format!(r#"[{{"type":"text","text":"[msg=803] [voice: 5 KB] {voice}"}}]"#)
    );
    assert_eq!(
        last_said(&requests[3], 3),
        r#"[{"type":"text","text":"帮我看看 [forward]"},{"type":"text","text":"[msg=804] [video: 12.3 MB]"},{"type":"text","text":"forward:\n  阿杰: 周六见"}]"#
    );
    assert_eq!(
        last_said(&requests[4], 1),
        format!("[{}]", image("msg-805-1"))
    );
    let Some(Message::User { blocks }) = requests[4].messages.last() else {
        panic!("最后是人说的");
    };
    assert!(
        blocks
            .iter()
            .all(|block| !matches!(block, Block::Text(text) if text.text.contains("[msg=805]"))),
        "{blocks:?}"
    );
}
