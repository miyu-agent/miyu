//! 带附件（施工 C-9，`docs/blueprint/tools/send_message.md`「附件」）：照这一轮的工作目录读、照内容认，连同那句话交给端口；
//! 执行前的链照 `targets` 查读哪几个；有一个读不成、太大的，整句不发，端口一次都不问；只订不发的不能带。

use serde_json::json;

use miyu_kernel::id::MediaType;
use miyu_tool::attachment::{Kind, LIMIT};
use miyu_tool::{Delivered, Target};

use super::{Port, text};
use crate::support::{Site, absolute, check, human, readable, said, tool};

/// 一个 PNG 的开头：签名、IHDR 里的宽高，量宽高够了。
fn png(width: u32, height: u32) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR".to_vec();
    bytes.extend(width.to_be_bytes());
    bytes.extend(height.to_be_bytes());
    bytes.extend([8, 6, 0, 0, 0, 0, 0, 0, 0]);
    bytes
}

/// 发给 j1 一句、带着 `paths`。
fn with(paths: &[&str]) -> serde_json::Value {
    json!({"to": "j1", "message": "See these.", "attachments": paths})
}

#[tokio::test]
async fn files_and_pictures_go_along_with_the_message() {
    let site = Site::new();
    site.file("work/notes/plan.md", b"# Plan\n");
    site.file("work/dot.png", &png(2, 1));
    let port = Port::new(Ok(Delivered::Sent));
    let done = site
        .done_with_messages(
            "send_message",
            with(&["notes/plan.md", "dot.png"]),
            Some(port.clone()),
        )
        .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "Message sent to j1.\n");
    let [attached] = port.attachments().try_into().expect("问过一次");
    let [plan, dot] = attached.try_into().expect("照先后两个");
    assert_eq!(plan.name.as_str(), "plan.md");
    assert_eq!(plan.bytes, b"# Plan\n");
    assert_eq!(
        plan.kind,
        Kind::File {
            media_type: MediaType::parse("text/plain").unwrap()
        },
        "文本照内容认"
    );
    assert_eq!(
        plan.path.as_ref().map(|path| path.as_str().to_string()),
        Some(absolute(&site.real("work/notes/plan.md"))),
        "带上真实的位置"
    );
    assert_eq!(dot.name.as_str(), "dot.png");
    assert_eq!(
        dot.kind,
        Kind::Image {
            media_type: MediaType::parse("image/png").unwrap(),
            width: 2,
            height: 1
        },
        "图照内容认、量宽高"
    );
    // 不带的照旧：端口拿到的是空的。
    let port = Port::new(Ok(Delivered::Sent));
    site.done_with_messages("send_message", with(&[]), Some(port.clone()))
        .await;
    assert_eq!(port.attachments(), [Vec::new()]);
}

#[test]
fn the_files_to_read_are_its_targets() {
    let site = Site::new();
    let call = site.call_with_packages(with(&["a.txt", "/srv/b.png"]), None);
    let read = |path: &str| Target {
        path: path.to_string(),
        write: false,
        itself: false,
    };
    assert_eq!(
        tool("send_message").targets(&call),
        [read("a.txt"), read("/srv/b.png")],
        "照读文件的规矩查：工作区以外的问人"
    );
    let call = site.call_with_packages(json!({"to": "j1", "message": "Hi."}), None);
    assert!(tool("send_message").targets(&call).is_empty());
    let call = site.call_with_packages(with(&["a.txt"]), None);
    assert_eq!(
        tool("subagent").targets(&call),
        [] as [Target; 0],
        "参数不对的交回空的"
    );
    let call = site.call_with_packages(
        json!({"description": "Look", "prompt": "Look at it.", "attachments": ["a.txt"]}),
        None,
    );
    assert_eq!(tool("subagent").targets(&call), [read("a.txt")]);
}

#[tokio::test]
async fn one_bad_attachment_stops_the_whole_message() {
    let site = Site::new();
    site.file("work/ok.txt", b"fine");
    site.file("work/dir/inside.txt", b"x");
    site.file("work/huge.png", &png(9000, 10));
    std::fs::write(site.0.join("work/big.bin"), vec![1u8; LIMIT as usize + 1]).unwrap();
    let mut checked = Vec::new();
    let cases = [
        (
            "missing.txt",
            "There is no file or directory at \"missing.txt\".\n",
            said("common/missing").with("path", "missing.txt"),
        ),
        ("dir", "\"dir\" is a directory.\n", said("common/directory")),
        (
            "huge.png",
            "\"huge.png\" is too large to attach: files can be up to 20 MiB, images up to 5 MiB and 8000 pixels a side.\n",
            said("attachments/too-big").with("path", "huge.png"),
        ),
        (
            "big.bin",
            "\"big.bin\" is too large to attach: files can be up to 20 MiB, images up to 5 MiB and 8000 pixels a side.\n",
            said("attachments/too-big").with("path", "big.bin"),
        ),
    ];
    for (bad, want, human_said) in cases {
        let port = Port::new(Ok(Delivered::Sent));
        let done = site
            .done_with_messages("send_message", with(&["ok.txt", bad]), Some(port.clone()))
            .await;
        assert!(done.error, "{bad}");
        assert_eq!(text(&done), want, "{bad}");
        assert!(port.asked().is_empty(), "{bad}：整句不发");
        check(&mut checked, human(done), human_said);
    }
    let port = Port::new(Ok(Delivered::Sent));
    let nine = ["ok.txt"; 9];
    let done = site
        .done_with_messages("send_message", with(&nine), Some(port.clone()))
        .await;
    assert!(done.error);
    assert_eq!(text(&done), "Attach at most 8 files.\n");
    assert!(port.asked().is_empty());
    check(
        &mut checked,
        human(done),
        said("attachments/too-many").with("limit", "8"),
    );
    let eight = ["ok.txt"; 8];
    let done = site
        .done_with_messages("send_message", with(&eight), Some(port.clone()))
        .await;
    assert!(!done.error, "正好 8 个的收");
    readable(&checked, &["send_message"]);
}

#[tokio::test]
async fn attachments_need_a_message() {
    let site = Site::new();
    site.file("work/ok.txt", b"fine");
    let port = Port::new(Ok(Delivered::Sent));
    let done = site
        .done_with_messages(
            "send_message",
            json!({"to": "0192f3a0-2222-7abc-8def-00119f03b21c", "notify_when_idle": true, "attachments": ["ok.txt"]}),
            Some(port.clone()),
        )
        .await;
    assert!(done.error);
    assert!(
        text(&done).contains("missing field `message`"),
        "{}",
        text(&done)
    );
    assert!(port.asked().is_empty());
}
