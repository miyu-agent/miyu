//! 她发一张图（施工 X-19）：扩展的工具回答带 `sent` 的，记进 `tool.result.sent`（图拷进会话属主名下、扩展写的本机路径不收），
//! 不进给模型的请求：她看到的只有结果的字。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_store::blob::Blobs;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

fn tools() -> Value {
    let spec = |name: &str| {
        json!({"name": name, "description": "A test tool.", "input_schema": {"type": "object"},
               "access": "read", "venues": ["local"]})
    };
    json!({"tools": [spec("sticker"), spec("too_many")]})
}

#[tokio::test]
async fn a_sent_image_is_logged_but_not_shown_to_the_model() {
    let home = Home::new();
    let blob = Blobs::new(home.root.blobs(&alice()))
        .put(b"\x89PNG fake sticker")
        .expect("存得进");
    let program = Program::new();
    let (path, step) = record(&home, "memes");
    install_providing(
        &home,
        "memes",
        &program.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{}", tools()),
            &format!("sent:{}", blob.as_str()),
            "serve",
        ]),
    );
    let script = Script::new([
        Play::calls(&[("sticker", "{}")]),
        Play::calls(&[("too_many", "{}")]),
        Play::Says("好。"),
    ])
    .titles([Play::Says("测试")]);
    let core = Arc::new(
        home.core_full(&script, Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).lines().count() < 3 {
        assert!(tokio::time::Instant::now() < deadline, "等不到登记");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "来一张").await;
    home.until_turns(&session, 1).await;

    let sent: Vec<_> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result.sent),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert_eq!(sent[0].image.blob, blob);
    assert_eq!(sent[0].alt.as_deref(), Some("A cat waving."));
    assert!(sent[0].sticker);
    assert_eq!(sent[0].image.path, None, "扩展写的本机路径不收");

    let requests = script.requests();
    assert_eq!(requests.len(), 3, "结果回给她以后她又调了一次、说了一句");
    let statuses: Vec<_> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(format!("{:?}", result.status)),
            _ => None,
        })
        .collect();
    assert_eq!(statuses, ["Ok", "Error"], "一次超过 4 张的当写法不对");
    let second = format!("{:?}", requests[1].1);
    assert!(second.contains("Sent m1."), "结果的字照常给她");
    assert!(
        !second.contains("A cat waving.") && !second.contains(blob.as_str()),
        "发出去的图不给她看：{second}"
    );
    core.stop_extensions().await;
}

/// 声明了系统账号的包（施工 Y-2中下）：它传的图在它自己名下，交回来的照它名下拷进会话属主名下，不照管理员的找。
#[tokio::test]
async fn a_system_accounts_image_is_handed_over_from_its_own_blobs() {
    let home = Home::new();
    let memes = miyu_kernel::id::AccountId::parse("memes").expect("合写法");
    let blob = Blobs::new(home.root.blobs(&memes))
        .put(b"\x89PNG fake sticker")
        .expect("存得进");
    let program = Program::new();
    let (path, step) = record(&home, "memes");
    install_providing(
        &home,
        "memes",
        &program.name(),
        "always",
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{}", tools()),
            &format!("sent:{}", blob.as_str()),
            "serve",
        ]),
    );
    let manifest = home
        .root
        .path()
        .join("home/alice/packages/memes/package.toml");
    let text = std::fs::read_to_string(&manifest).expect("装好了");
    std::fs::write(&manifest, format!("{text}system_account = true\n")).expect("写得进");
    let script = Script::new([Play::calls(&[("sticker", "{}")]), Play::Says("好。")])
        .titles([Play::Says("测试")]);
    let core = crate::blob_session::served_core(&home, &script);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(&path).lines().count() < 3 {
        assert!(tokio::time::Instant::now() < deadline, "等不到登记");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    assert!(
        read(&path).contains(r#""account":"memes""#),
        "{}",
        read(&path)
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "来一张").await;
    home.until_turns(&session, 1).await;

    let sent: Vec<_> = home
        .log(&session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(result.sent),
            _ => None,
        })
        .flatten()
        .collect();
    assert_eq!(sent.len(), 1, "{sent:?}");
    assert!(
        Blobs::new(home.root.blobs(&alice())).path(&blob).is_file(),
        "从它名下拷进了属主名下"
    );
    core.stop_extensions().await;
}
