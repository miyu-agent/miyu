//! 文件改了、前缀要变时发送前问一声（施工 OV-1 下，`docs/construction/OV-1-同名覆盖工具说明和提示词.md` 第十节）：握手声明了
//! `caps.prefix_ask` 的头，属主的覆盖改过以后头一句拒 `prefix_changing`、带改了的文件；带 `keep` 的照常说、这一轮照新的拼；
//! 照新的拼过了不再问；重做、开轮的命令、设长期目标也问；没声明的头不问；带 `compact` 的先压缩再说。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_endpoint::Core;
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};

use crate::goal_command::goal_tools;
use crate::prompt_override::latest_system;
use crate::support::*;

/// 改的那一份覆盖。
const RULE: &str = "home/alice/prompts/core/local-paths-rule.txt";

/// 握手：令牌对，中文，能输入，会问「前缀要变」。
async fn hello_asking(client: &mut Client) -> Value {
    client
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
        .await
}

/// 写 `text` 进那一份覆盖，等核心盯着的那一头看到（十秒为限）。
async fn edit(home: &Home, core: &Core, text: &str) {
    let before = core.edits_version(&alice());
    home.write(RULE, text);
    for _ in 0..1000 {
        if core.edits_version(&alice()) > before {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("十秒内没看到改动");
}

/// 拒绝的原因，没拒的是 `ok`。
fn reason(reply: &Value) -> &str {
    reply["error"]["data"]["reason"].as_str().unwrap_or("ok")
}

#[tokio::test]
async fn an_edited_prefix_is_asked_before_the_next_turn() {
    let home = Home::new();
    std::fs::create_dir_all(home.root.path().join("home/alice/prompts/core")).expect("建得了");
    home.write("home/alice/commands/review.md", "Review $ARGUMENTS.\n");
    let script = Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
        Play::Says("好。"),
        Play::Says("看过了。"),
        Play::Says("三。"),
        Play::Says("嗯。"),
        Play::Says("<analysis>a</analysis>\n<summary>\nThe user said x.\n</summary>"),
        Play::Says("四。"),
    ])
    .window(1_000_000);
    // 带长期目标的工具：`/goal` 才开得了。
    let core = home.core_with_tools(&script, goal_tools(), TOKEN);
    let mut asking = Client::connect(Arc::clone(&core));
    hello_asking(&mut asking).await;
    let session = asking.create("c1", "~").await;
    // 第一句说七万个字：压得掉它（出厂的尾巴是 16000 token）。
    let first = asking.say("s1", &session, &"x".repeat(70_000)).await;
    assert_eq!(reason(&first), "ok", "没改过的不问：{first}");
    home.until_turns(&session, 1).await;

    edit(&home, &core, "Mine about local paths.\n").await;
    let refused = asking.say("s2", &session, "在吗").await;
    assert_eq!(reason(&refused), "prefix_changing", "{refused}");
    let files = &refused["error"]["data"]["files"];
    assert!(
        files
            .as_array()
            .is_some_and(|files| files.contains(&json!(RULE))),
        "{refused}"
    );
    let kept = asking
        .call(
            "s3",
            "session.send",
            json!({"session": session, "text": "在吗", "prefix": "keep"}),
        )
        .await;
    assert_eq!(reason(&kept), "ok", "{kept}");
    home.until_turns(&session, 2).await;
    let text = latest_system(&home, &session);
    assert!(
        text.contains("Mine about local paths."),
        "这一轮照新的拼：{text}"
    );
    // 照新的拼过了，没有新的改动不再问。
    let again = asking.say("s3b", &session, "好").await;
    assert_eq!(reason(&again), "ok", "{again}");
    home.until_turns(&session, 3).await;

    // 开新一轮的另外两个入口：重做、开轮的命令。
    edit(&home, &core, "Second about local paths.\n").await;
    let redo = asking
        .call("r1", "session.redo", json!({"session": session}))
        .await;
    assert_eq!(reason(&redo), "prefix_changing", "{redo}");
    let run = asking
        .call(
            "r2",
            "command.run",
            json!({"session": session, "text": "/review a.rs"}),
        )
        .await;
    assert_eq!(reason(&run), "prefix_changing", "{run}");
    let goal = asking
        .call(
            "r3",
            "command.run",
            json!({"session": session, "text": "/goal finish the job"}),
        )
        .await;
    assert_eq!(reason(&goal), "prefix_changing", "{goal}");
    let run = asking
        .call(
            "r4",
            "command.run",
            json!({"session": session, "text": "/review a.rs", "prefix": "keep"}),
        )
        .await;
    assert_eq!(reason(&run), "ok", "{run}");
    home.until_turns(&session, 4).await;
    assert!(latest_system(&home, &session).contains("Second about local paths."));

    // 没声明的头照旧直接发；照新的拼过了，声明了的头也不再问。
    edit(&home, &core, "Fourth about local paths.\n").await;
    let mut plain = Client::connect(Arc::clone(&core));
    plain.hello().await;
    let said = plain.say("p1", &session, "还在吗").await;
    assert_eq!(reason(&said), "ok", "老的头不问：{said}");
    home.until_turns(&session, 5).await;
    assert!(latest_system(&home, &session).contains("Fourth about local paths."));
    let quiet = asking.say("s3c", &session, "嗯").await;
    assert_eq!(reason(&quiet), "ok", "{quiet}");
    home.until_turns(&session, 6).await;

    // 先压缩再说：压缩那一轮完了才开这一句的那一轮。
    edit(&home, &core, "Third about local paths.\n").await;
    let refused = asking.say("s4", &session, "再说一句").await;
    assert_eq!(reason(&refused), "prefix_changing", "{refused}");
    let compacted = asking
        .call(
            "s5",
            "session.send",
            json!({"session": session, "text": "再说一句", "prefix": "compact"}),
        )
        .await;
    assert_eq!(reason(&compacted), "ok", "{compacted}");
    home.until_turns(&session, 8).await;
    let log = home.log(&session);
    let done = log
        .iter()
        .position(|event| matches!(event.body, Body::ContextCompacted(_)))
        .expect("压了");
    // 那一句可能在压缩那一轮里就排上了：看它开的那一轮在压完之后。
    let said = log
        .iter()
        .rfind(|event| matches!(&event.body, Body::MessageUser(_)))
        .expect("说了")
        .seq;
    let opened = log
        .iter()
        .position(|event| {
            matches!(&event.body, Body::TurnStarted(started) if started.trigger == Some(said))
        })
        .expect("开了那一轮");
    assert!(done < opened, "压完了才说");
    assert!(latest_system(&home, &session).contains("Third about local paths."));
}

/// `/goal` 设定照命令带的 `prefix` 放行：不带的拒，带 `keep` 的设上、开一轮。
#[tokio::test]
async fn a_goal_is_set_once_the_change_is_let_through() {
    let home = Home::new();
    std::fs::create_dir_all(home.root.path().join("home/alice/prompts/core")).expect("建得了");
    let script = Script::new([Play::Says("一。"), Play::Says("做完了。")]);
    let core = home.core_with_tools(&script, goal_tools(), TOKEN);
    let mut asking = Client::connect(Arc::clone(&core));
    hello_asking(&mut asking).await;
    let session = asking.create("c1", "~").await;
    asking.say("s1", &session, "在吗").await;
    home.until_turns(&session, 1).await;
    edit(&home, &core, "Mine about local paths.\n").await;
    let text = "/goal --rounds 1 finish the job";
    let refused = asking
        .call(
            "g1",
            "command.run",
            json!({"session": session, "text": text}),
        )
        .await;
    assert_eq!(reason(&refused), "prefix_changing", "{refused}");
    let kept = asking
        .call(
            "g2",
            "command.run",
            json!({"session": session, "text": text, "prefix": "keep"}),
        )
        .await;
    assert_eq!(reason(&kept), "ok", "{kept}");
}

/// 改技能不问（施工 PT-2 上，原来是误报）：技能那一块钉在快照里、中途变了走尾巴注入，system 不变、前缀缓存不断，问人「前缀要
/// 变了」是假的。改了自己的技能以后下一句照常说，两轮的 system 一字不差。
#[tokio::test]
async fn an_edited_skill_does_not_ask_because_the_prefix_stays() {
    let home = Home::new();
    const SKILL: &str = "home/alice/skills/notes/SKILL.md";
    home.write(
        SKILL,
        "---\nname: notes\ndescription: Take notes.\n---\nWrite them down.\n",
    );
    let script = Script::new([Play::Says("一。"), Play::Says("二。")]);
    let core = home.core_with_tools(&script, goal_tools(), TOKEN);
    let mut asking = Client::connect(Arc::clone(&core));
    hello_asking(&mut asking).await;
    let session = asking.create("c1", "~").await;
    let first = asking.say("s1", &session, "在吗").await;
    assert_eq!(reason(&first), "ok", "{first}");
    home.until_turns(&session, 1).await;
    let before = latest_system(&home, &session);

    let version = core.edits_version(&alice());
    home.write(
        SKILL,
        "---\nname: notes\ndescription: Take notes, short ones.\n---\nWrite them down.\n",
    );
    // 给盯着的那一头一秒：原来这里号会变（技能那棵树在盯），现在不盯了，号照旧。
    for _ in 0..100 {
        if core.edits_version(&alice()) > version {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(core.edits_version(&alice()), version, "技能不算前缀的改动");
    let second = asking.say("s2", &session, "好").await;
    assert_eq!(reason(&second), "ok", "改技能不问：{second}");
    home.until_turns(&session, 2).await;
    assert_eq!(
        latest_system(&home, &session),
        before,
        "system 一字不差：前缀缓存不断"
    );
}
