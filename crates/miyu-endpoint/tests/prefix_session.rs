//! 发送前问「前缀要变了」照这个会话自己判（施工 PX-1）：网页上开了几个会话、各用各的人格，换了别的人格的头像、放了图片，
//! 这些会话的前缀其实一个字没变，不该问；换了人格、已经先压缩过的会话，下一句不再问第二次；改了它自己用的人格的人设照样问，
//! `data.files` 只列真进它前缀的那几个。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_session::testkit::{Play, Script};

use crate::support::*;

/// 握手：令牌对，中文，能输入，会问「前缀要变」。
async fn hello_asking(client: &mut Client) {
    let reply = client
        .call(
            "hello-1",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": "test", "version": "0.0.0"},
                "locale": "zh-CN",
                "caps": {"input": true, "prefix_ask": true},
                "token": TOKEN,
            }),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
}

/// 往你那一层写 `path`（`root/` 下），等核心盯着的那一头看到（十秒为限）。
async fn touch(home: &Home, core: &Core, path: &str, bytes: &[u8]) {
    let before = core.edits_version();
    let file = home.root.path().join(path);
    std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
    std::fs::write(&file, bytes).expect("写得进");
    for _ in 0..1000 {
        if core.edits_version() > before {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("十秒内没看到改动：{path}");
}

/// 拒绝的原因，没拒的是 `ok`。
fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"].as_str().unwrap_or("ok")
}

/// 两个人格：会话用的 `miyu`，别的 `other`。
fn personas(home: &Home) {
    home.write("root/personas/miyu/prompts/persona.md", "You are Miyu.\n");
    home.write("root/personas/other/prompts/persona.md", "You are Other.\n");
}

#[tokio::test]
async fn pictures_and_other_personas_do_not_ask() {
    let home = Home::new();
    personas(&home);
    let script = Script::new([Play::Says("一。"), Play::Says("二。"), Play::Says("三。")]);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    hello_asking(&mut client).await;
    let session = client.create_as("c", "~", "miyu").await;
    assert_eq!(reason(&client.say("s1", &session, "在吗").await), "ok");
    home.until_turns(&session, 1).await;
    // 别的人格的头像、删掉的（这里换了它的人设）：这个会话一个字没变。
    touch(&home, &core, "root/personas/other/avatar.png", b"png").await;
    touch(
        &home,
        &core,
        "root/personas/other/prompts/persona.md",
        b"You are Changed.\n",
    )
    .await;
    let second = client.say("s2", &session, "在吗").await;
    assert_eq!(reason(&second), "ok", "别的人格的改动不问：{second}");
    home.until_turns(&session, 2).await;
    // 它自己用的人格的图片：也不进前缀。
    touch(&home, &core, "root/personas/miyu/avatar.webp", b"webp").await;
    let third = client.say("s3", &session, "在吗").await;
    assert_eq!(reason(&third), "ok", "只改图片不问：{third}");
}

#[tokio::test]
async fn its_own_persona_still_asks_and_only_its_files_are_named() {
    let home = Home::new();
    personas(&home);
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    hello_asking(&mut client).await;
    let session = client.create_as("c", "~", "miyu").await;
    client.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    touch(&home, &core, "root/personas/other/avatar.png", b"png").await;
    touch(
        &home,
        &core,
        "root/personas/miyu/prompts/persona.md",
        b"You are Miyu, edited.\n",
    )
    .await;
    let refused = client.say("s2", &session, "在吗").await;
    assert_eq!(reason(&refused), "prefix_changing", "{refused}");
    assert_eq!(
        refused["error"]["data"]["files"],
        json!(["personas/miyu/prompts/persona.md"]),
        "只列真进它前缀的：{refused}"
    );
}

/// 网页的复现：会话刚换了人格、先压缩过；之后有人在 `root/personas/` 下给别的人格放了一张图，下一句不该问。换人格之前就有的
/// 改动也不再问第二次。
#[tokio::test]
async fn a_recast_session_is_not_asked_again() {
    let home = Home::new();
    personas(&home);
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said hi.\n</summary>"),
        Play::Says("在。"),
        Play::Says("嗯。"),
    ])
    .window(1_000_000);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    hello_asking(&mut client).await;
    let session = client.create("c", "~").await;
    let first = format!("在吗{}", "x".repeat(70_000));
    assert_eq!(reason(&client.say("s1", &session, &first).await), "ok");
    home.until_turns(&session, 1).await;
    // 换人格之前就有的改动。
    touch(
        &home,
        &core,
        "root/personas/miyu/prompts/persona.md",
        b"You are Miyu, edited.\n",
    )
    .await;
    let recast = client
        .call(
            "r",
            "session.configure",
            json!({"session": session, "persona": "miyu"}),
        )
        .await;
    assert!(recast.get("error").is_none(), "{recast}");
    let second = client.say("s2", &session, "在吗").await;
    assert_eq!(reason(&second), "ok", "换人格压缩过了，不再问：{second}");
    home.until_turns(&session, 2).await;
    touch(&home, &core, "root/personas/other/avatar.png", b"png").await;
    let third = client.say("s3", &session, "在吗").await;
    assert_eq!(reason(&third), "ok", "别的人格的图片不问：{third}");
}

/// 还没说过话就换了人格（没东西可压缩，快照等下一个回合才换）：换之前就有的改动，换完下一句也不再问（施工 PX-1，`session.configure`
/// 换人格、换预设记成放行到这一刻）。
#[tokio::test]
async fn a_recast_before_any_turn_is_not_asked_either() {
    let home = Home::new();
    personas(&home);
    let script = Script::new([Play::Says("在。")]);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    hello_asking(&mut client).await;
    let session = client.create("c", "~").await;
    touch(
        &home,
        &core,
        "root/personas/miyu/prompts/persona.md",
        b"You are Miyu, edited.\n",
    )
    .await;
    let recast = client
        .call(
            "r",
            "session.configure",
            json!({"session": session, "persona": "miyu"}),
        )
        .await;
    assert!(recast.get("error").is_none(), "{recast}");
    let first = client.say("s1", &session, "在吗").await;
    assert_eq!(reason(&first), "ok", "换人格时已经放行：{first}");
}

/// 说过几句、换了人格（压缩过）、下一句还没发：之后有人给别的人格换了头像（施工 PX-1补，网页的复现）。快照等下一个回合才换成
/// 新人格的，干算的基准要是那一份，不是还没换下来的旧快照：
/// - 这一句不问，取消了什么都不改再发也不问；
/// - 再改它新人格的人设，问，`data.files` 只列那一个，不把别的人格的图片一起列上。
#[tokio::test]
async fn a_recast_then_another_persona_s_picture_does_not_ask() {
    let home = Home::new();
    personas(&home);
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said hi.\n</summary>"),
        Play::Says("在。"),
    ])
    .window(1_000_000);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    hello_asking(&mut client).await;
    let session = client.create("c", "~").await;
    let first = format!("在吗{}", "x".repeat(70_000));
    assert_eq!(reason(&client.say("s1", &session, &first).await), "ok");
    home.until_turns(&session, 1).await;
    let recast = client
        .call(
            "r",
            "session.configure",
            json!({"session": session, "persona": "miyu"}),
        )
        .await;
    assert!(recast.get("error").is_none(), "{recast}");
    touch(&home, &core, "root/personas/other/avatar.png", b"png").await;
    let second = client.say("s2", &session, "在吗").await;
    assert_eq!(reason(&second), "ok", "别的人格的头像不问：{second}");
    let again = client.say("s2b", &session, "在吗").await;
    assert_eq!(reason(&again), "ok", "再发也不问：{again}");
    home.until_turns(&session, 2).await;
}

/// 同上，换的是它自己将要用的新人格的人设：问，只列那一个文件。
#[tokio::test]
async fn a_recast_then_its_new_persona_edited_names_only_that_file() {
    let home = Home::new();
    personas(&home);
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said hi.\n</summary>"),
        Play::Says("在。"),
    ])
    .window(1_000_000);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    hello_asking(&mut client).await;
    let session = client.create("c", "~").await;
    let first = format!("在吗{}", "x".repeat(70_000));
    assert_eq!(reason(&client.say("s1", &session, &first).await), "ok");
    home.until_turns(&session, 1).await;
    let recast = client
        .call(
            "r",
            "session.configure",
            json!({"session": session, "persona": "miyu"}),
        )
        .await;
    assert!(recast.get("error").is_none(), "{recast}");
    touch(&home, &core, "root/personas/other/avatar.png", b"png").await;
    touch(
        &home,
        &core,
        "root/personas/miyu/prompts/persona.md",
        b"You are Miyu, edited.\n",
    )
    .await;
    let refused = client.say("s2", &session, "在吗").await;
    assert_eq!(reason(&refused), "prefix_changing", "{refused}");
    assert_eq!(
        refused["error"]["data"]["files"],
        json!(["personas/miyu/prompts/persona.md"]),
        "只列它新人格的那一个：{refused}"
    );
}

/// 换回无人格（施工 PX-1补，网页复现的第 4 步）：同样不为别的人格的头像问。
#[tokio::test]
async fn a_recast_to_no_persona_then_a_picture_does_not_ask() {
    let home = Home::new();
    personas(&home);
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said hi.\n</summary>"),
        Play::Says("在。"),
    ])
    .window(1_000_000);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    hello_asking(&mut client).await;
    let session = client.create_as("c", "~", "other").await;
    let first = format!("在吗{}", "x".repeat(70_000));
    assert_eq!(reason(&client.say("s1", &session, &first).await), "ok");
    home.until_turns(&session, 1).await;
    let recast = client
        .call(
            "r",
            "session.configure",
            json!({"session": session, "persona": null}),
        )
        .await;
    assert!(recast.get("error").is_none(), "{recast}");
    touch(&home, &core, "root/personas/other/avatar.png", b"png").await;
    let second = client.say("s2", &session, "在吗").await;
    assert_eq!(
        reason(&second),
        "ok",
        "无人格的不为别的人格的头像问：{second}"
    );
}

/// 换上了新人格以后基准就作废（施工 PX-1补）：换人格那一刻记下的那一份已经用过了，之后改了人设、跑了一轮、又改回去，前缀从
/// 改过的那一份变回原样，照样要问，不能因为它和旧基准一样就放过。
#[tokio::test]
async fn the_baseline_is_dropped_once_the_new_persona_is_in() {
    let home = Home::new();
    personas(&home);
    let script = Script::new([
        Play::Says("好。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said hi.\n</summary>"),
        Play::Says("在。"),
        Play::Says("嗯。"),
        Play::Says("哦。"),
    ])
    .window(1_000_000);
    let core = home.core(&script);
    let mut client = Client::connect(Arc::clone(&core));
    hello_asking(&mut client).await;
    let session = client.create("c", "~").await;
    let first = format!("在吗{}", "x".repeat(70_000));
    assert_eq!(reason(&client.say("s1", &session, &first).await), "ok");
    home.until_turns(&session, 1).await;
    let recast = client
        .call(
            "r",
            "session.configure",
            json!({"session": session, "persona": "miyu"}),
        )
        .await;
    assert!(recast.get("error").is_none(), "{recast}");
    assert_eq!(reason(&client.say("s2", &session, "在吗").await), "ok");
    home.until_turns(&session, 2).await;
    touch(
        &home,
        &core,
        "root/personas/miyu/prompts/persona.md",
        b"You are Miyu, edited.\n",
    )
    .await;
    let edited = client.say("s3", &session, "在吗").await;
    assert_eq!(reason(&edited), "prefix_changing", "{edited}");
    let kept = client
        .call(
            "s3k",
            "session.send",
            json!({"session": session, "text": "在吗", "prefix": "keep"}),
        )
        .await;
    assert!(kept.get("error").is_none(), "{kept}");
    home.until_turns(&session, 3).await;
    touch(
        &home,
        &core,
        "root/personas/miyu/prompts/persona.md",
        b"You are Miyu.\n",
    )
    .await;
    let reverted = client.say("s4", &session, "在吗").await;
    assert_eq!(
        reason(&reverted),
        "prefix_changing",
        "改回去也要问：{reverted}"
    );
}
