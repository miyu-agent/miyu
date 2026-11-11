//! 只有路径的附件（施工 AT-1，`docs/blueprint/protocol.md` 的 `session.send` 第 5 条）：`attachments` 里没有 `blob`、只有
//! `path` 的项记成一块 `path`，核心不读、不查在不在；和有 `blob` 的混着发先后不乱；视图里照样是附件，带 `path_only`；
//! 重发、撤销恢复、导出再导入都照带；写法不对的、场所会话里的、回答提问时的参数不对，什么都不写。

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::request::Message;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
use crate::support::*;

/// 一张 `width` × `height` 的 PNG 的开头。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend_from_slice(&width.to_be_bytes());
    bytes.extend_from_slice(&height.to_be_bytes());
    bytes.extend_from_slice(&[8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 会话日志里人说的话，照先后，每一句是它的块写成的 JSON。
fn said(home: &Home, session: &str) -> Vec<Value> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MessageUser(message) => {
                Some(serde_json::to_value(message.blocks).expect("写得成"))
            }
            _ => None,
        })
        .collect()
}

/// 视图里人说的那几条的附件，照先后。
async fn shown(client: &mut Client, id: &str, session: &str) -> Vec<Value> {
    let reply = client
        .call(id, "view.page", json!({"session": session, "view": true}))
        .await;
    reply["result"]["entries"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|entry| entry["kind"] == "user")
        .map(|entry| entry["attachments"].clone())
        .collect()
}

/// 一个 31 MiB 的、核心那台机器上并没有的文件：只给路径。
fn big() -> Value {
    json!({"path": "/data/没有这个文件.iso", "name": "没有这个文件.iso", "size": 32_505_856u64})
}

/// 记成的那一块。
fn big_block() -> Value {
    json!({"type": "path", "name": "没有这个文件.iso", "path": "/data/没有这个文件.iso", "size": 32_505_856u64})
}

#[tokio::test]
async fn a_path_only_attachment_is_recorded_sent_and_shown_in_order() {
    let home = Home::new();
    let script = Script::new([Play::Says("看到了。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let picture = home.work.join("shot.png");
    std::fs::write(&picture, png(8, 6)).expect("写得了");
    let put = client.call("p", "blob.put", json!({"path": picture})).await;
    let mut image = put["result"].clone();
    image["path"] = json!(picture);
    let session = client.create("c1", "~").await;
    // 没写大小的、`size` 写 `null` 的都是没有大小；多写的格不看。
    let bare = json!({"path": "~/notes.md", "name": "notes.md", "size": null, "kind": "file"});
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "看看", "attachments": [big(), image, bare]}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    home.until_turns(&session, 1).await;
    let blocks = said(&home, &session);
    let [blocks] = blocks.as_slice() else {
        panic!("只说了一句：{blocks:?}");
    };
    assert_eq!(blocks[0], json!({"type": "text", "text": "看看"}));
    assert_eq!(blocks[1], big_block(), "核心不读它、不查它在不在");
    assert_eq!(blocks[2]["type"], json!("image"), "{blocks}");
    assert_eq!(
        blocks[3],
        json!({"type": "path", "name": "notes.md", "path": "~/notes.md"})
    );
    assert_eq!(blocks.as_array().map(Vec::len), Some(4));
    // 她看到的请求里，最后一条人说的就是这几块，先后照发的。
    let requests = script.requests();
    let Some(Message::User { blocks: sent }) = requests[0].1.messages.last() else {
        panic!("最后一条是人说的");
    };
    let sent = serde_json::to_value(sent).expect("写得成");
    let sent = sent.as_array().expect("是数组");
    assert_eq!(
        sent[sent.len() - 4..],
        *blocks.as_array().expect("是数组"),
        "{sent:?}"
    );
    // 没建附件目录：只有路径的不落成文件，带了路径的图也不拷。
    let dir = miyu_store::attachments::dir(
        &home.root,
        &miyu_kernel::id::SessionId::parse(&session).expect("会话编号"),
    );
    assert!(!dir.exists(), "没建附件目录");
    // 视图里照样是附件：没有 `blob`、类型，带 `path_only`；有 blob 的那一项和原来一样。
    let attachments = shown(&mut client, "v1", &session).await;
    assert_eq!(
        attachments,
        [json!([
            {"kind": "file", "name": "没有这个文件.iso", "path": "/data/没有这个文件.iso",
             "size": 32_505_856u64, "path_only": true},
            {"kind": "image", "name": "shot.png", "media_type": "image/png",
             "blob": image["blob"], "width": 8, "height": 6, "path": picture},
            {"kind": "file", "name": "notes.md", "path": "~/notes.md", "path_only": true},
        ])]
    );
}

#[tokio::test]
async fn a_path_only_attachment_alone_is_a_message() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "", "attachments": [big()]}),
        )
        .await;
    assert!(reply["result"]["events"].is_array(), "{reply}");
    assert_eq!(said(&home, &session), [json!([big_block()])]);
}

#[tokio::test]
async fn path_only_attachments_that_do_not_fit_are_bad_params() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    for attachment in [
        // 相对的路径、空的路径、带控制字符的、太长的。
        json!({"path": "data/a.iso", "name": "a.iso"}),
        json!({"path": "", "name": "a.iso"}),
        json!({"path": "/data/a\n.iso", "name": "a.iso"}),
        json!({"path": format!("/{}", "a".repeat(4096)), "name": "a.iso"}),
        json!({"path": 7, "name": "a.iso"}),
        // 名字：没写、空的、太长的、带斜杠的、不是字。
        json!({"path": "/data/a.iso"}),
        json!({"path": "/data/a.iso", "name": null}),
        json!({"path": "/data/a.iso", "name": ""}),
        json!({"path": "/data/a.iso", "name": "a".repeat(256)}),
        json!({"path": "/data/a.iso", "name": "data/a.iso"}),
        json!({"path": "/data/a.iso", "name": 7}),
        // 大小：负的、小数、字、布尔。
        json!({"path": "/data/a.iso", "name": "a.iso", "size": -1}),
        json!({"path": "/data/a.iso", "name": "a.iso", "size": 1.5}),
        json!({"path": "/data/a.iso", "name": "a.iso", "size": 1.0}),
        json!({"path": "/data/a.iso", "name": "a.iso", "size": "12"}),
        json!({"path": "/data/a.iso", "name": "a.iso", "size": true}),
        // 既没有 `blob` 也没有 `path`。
        json!({"name": "a.iso"}),
        json!({"name": "a.iso", "size": 3}),
        json!({}),
    ] {
        for (method, params) in [
            (
                "session.send",
                json!({"session": session, "text": "x", "attachments": [attachment]}),
            ),
            (
                "session.redo",
                json!({"session": session, "attachments": [attachment]}),
            ),
        ] {
            let reply = client.call("c2", method, params).await;
            assert_eq!(
                reason(&reply),
                Some("bad_params"),
                "{method} {attachment}：{reply}"
            );
            assert_eq!(reply["error"]["code"], json!(-32602), "{reply}");
        }
    }
    // 一项不对，整句不送：前面那一项对的也不记。
    let reply = client
        .call(
            "c3",
            "session.send",
            json!({"session": session, "text": "x",
                   "attachments": [big(), {"path": "a.iso", "name": "a.iso"}]}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(home.log(&session).len(), 1, "什么都没写");
}

/// 重发：不带新附件的照原来的带；换成只有路径的也行；撤销、恢复以后还在。
#[tokio::test]
async fn a_path_only_attachment_goes_along_with_redo_and_undo() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("看到了。"),
        Play::Says("又看了。"),
        Play::Says("换了一个。"),
    ]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let sent = json!({"session": session, "text": "看看", "attachments": [big()]});
    let reply = client.call("c2", "session.send", sent).await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    // 撤销、恢复：那一句连着附件回来，视图里也在。
    let reply = client
        .call("c3", "session.revert", json!({"session": session}))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let reply = client
        .call("c4", "session.unrevert", json!({"session": session}))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let attachments = shown(&mut client, "v2", &session).await;
    assert_eq!(attachments.len(), 1, "{attachments:?}");
    assert_eq!(attachments[0][0]["path_only"], json!(true));
    // 只换话：原来那个只有路径的照带。
    let reply = client
        .call(
            "c5",
            "session.redo",
            json!({"session": session, "text": "再看看"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 2).await;
    let last = said(&home, &session).pop().expect("重发了");
    assert_eq!(
        last,
        json!([{"type": "text", "text": "再看看"}, big_block()])
    );
    // 换附件：换成另一个只有路径的。
    let other = json!({"path": "C:\\data\\b.bin", "name": "b.bin", "size": 0});
    let reply = client
        .call(
            "c6",
            "session.redo",
            json!({"session": session, "attachments": [other]}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 3).await;
    let last = said(&home, &session).pop().expect("重发了");
    assert_eq!(
        last,
        json!([
            {"type": "text", "text": "再看看"},
            {"type": "path", "name": "b.bin", "path": "C:\\data\\b.bin", "size": 0},
        ])
    );
    // 她每一轮看到的最后一句都带着那一块。
    for (_, request) in script.requests() {
        let Some(Message::User { blocks }) = request.messages.last() else {
            panic!("最后一条是人说的");
        };
        let blocks = serde_json::to_value(blocks).expect("写得成");
        let last = blocks.as_array().and_then(|blocks| blocks.last());
        assert_eq!(last.map(|block| &block["type"]), Some(&json!("path")));
    }
}

/// 导出原始包、在另一个数据根导入：这一项没有 blob 可带，条目照样在。
#[tokio::test]
async fn a_path_only_attachment_survives_export_and_import() {
    let (source, target) = (Home::new(), Home::new());
    let mut client = Client::connect(source.core(&Script::new([Play::Says("好。")])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let sent = json!({"session": session, "text": "看看", "attachments": [big()]});
    let reply = client.call("c2", "session.send", sent).await;
    assert!(reply.get("error").is_none(), "{reply}");
    source.until_turns(&session, 1).await;
    let out = source.work.join("one.zip");
    let reply = client
        .call(
            "e1",
            "session.export",
            json!({"session": session, "format": "bundle", "to": {"path": out}}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    let mut there = Client::connect(target.core(&Script::new([])));
    there.hello().await;
    let imported = there
        .call("i1", "session.import", json!({"path": out}))
        .await;
    assert_eq!(
        imported["result"]["sessions"],
        json!([session]),
        "{imported}"
    );
    assert_eq!(
        said(&target, &session),
        [json!([{"type": "text", "text": "看看"}, big_block()])]
    );
    let attachments = shown(&mut there, "v1", &session).await;
    assert_eq!(
        attachments,
        [
            json!([{"kind": "file", "name": "没有这个文件.iso", "path": "/data/没有这个文件.iso",
                 "size": 32_505_856u64, "path_only": true}])
        ]
    );
}

/// 压缩：带着这一项的那一轮收进摘要，摘要请求里它还是那一块（驱动照样写成一句）；压完接着说，视图里它还在。
#[tokio::test]
async fn a_path_only_attachment_goes_through_compaction() {
    let home = Home::new();
    let script = Script::new([
        Play::Says("好。"),
        Play::Says(
            "<analysis>a</analysis>\n<summary>\nThe user attached a file by path.\n</summary>",
        ),
        Play::Says("还在。"),
    ])
    .window(1_000_000);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    // 说得够长，超了留着的尾巴，才有能压的（同 `tests/compact.rs`）。
    let sent = json!({"session": session, "text": "x".repeat(70_000), "attachments": [big()]});
    let reply = client.call("c2", "session.send", sent).await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    let reply = client
        .call("c3", "session.compact", json!({"session": session}))
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 2).await;
    let requests = script.requests();
    let summarized = serde_json::to_value(&requests[1].1.messages).expect("写得成");
    assert!(
        summarized.to_string().contains(&big_block().to_string()),
        "摘要请求里那一句还带着这一块"
    );
    let reply = client.say("c4", &session, "还在吗").await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 3).await;
    let attachments = shown(&mut client, "v1", &session).await;
    assert_eq!(
        attachments[0][0]["path_only"],
        json!(true),
        "{attachments:?}"
    );
}

/// 场所会话不收（平台桥代表平台上的人说的）；回答提问、拒绝时附的也不收：都是参数不对，什么都不写。
#[tokio::test]
async fn venue_sessions_and_answers_do_not_take_path_only_attachments() {
    let home = Home::new();
    let script = Script::new([Play::Says("在。")]);
    let mut client = Client::connect(bound_core(&home, &script, Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let owner = json!({"external": "qq:10001"});
    let before = home.log(&session).len();
    let reply = client
        .call(
            "s1",
            "session.send",
            json!({"session": session, "text": "看这个", "as": owner, "attachments": [big()]}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    assert_eq!(home.log(&session).len(), before, "什么都没写");
    // 同一句不带这一项的照收。
    let reply = client
        .call(
            "s2",
            "session.send",
            json!({"session": session, "text": "看这个", "as": owner, "attachments": []}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    // 场所会话里重发也不收。
    let reply = client
        .call(
            "s3",
            "session.redo",
            json!({"session": session, "attachments": [big()]}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
    // 回答提问、拒绝时附的：照旧要 `blob`。
    let reply = client
        .call(
            "a1",
            "session.answer",
            json!({"session": session, "call": "call_1_1", "decision": "deny", "attachments": [big()]}),
        )
        .await;
    assert_eq!(reason(&reply), Some("bad_params"), "{reply}");
}
