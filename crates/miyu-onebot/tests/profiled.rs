//! 私聊带「关于你」（施工 O-39，`onebot.md` 第一条「怎么走」第 7 条）：核心 P-9 起 `venue.session` 的 `profile` 写真的，造会话时
//! system 里带上管理员写的称呼和更多描述（属主是管理员本人的才带）。桥只给不是白名单成员的私聊（终端管理员本人，桥问以前
//! 认不出，核心照属主挑）写 `profile: true`；白名单成员的私聊、群不写这一格（不写，不是写假）。
//!
//! - 进程里的桥（转接记下 `venue.session` 的参数）：终端管理员的私聊带，白名单成员的、群的不带这一格。
//! - 真核心拉起真桥：个人设置写了 `profile.name`、`profile.about`，终端管理员私聊发一句，模型收到的 system 里有那一块；白名单
//!   成员的私聊没有。

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::support::group::*;
use crate::support::*;

/// 白名单成员。
const JIE: i64 = 20003;

/// 群号。
const GROUP: i64 = 3901;

/// 群里的人。
const LIN: i64 = 20004;

/// 假 NapCat 认得的群成员：她自己。
const MEMBERS: &[Member] = &[(BOT, "米尤", "miyu")];

/// 个人设置里写的两项。
const NAME: &str = "小林";
const ABOUT: &str = "写 Rust 的独立开发者";

/// 拼出来的那一块。
const BLOCK: &str = "<user>\nname: 小林\nabout: 写 Rust 的独立开发者\n</user>";

/// 白名单里只有 [`JIE`]（握手交来的配置）。
fn whitelisted() -> Value {
    let mut config = handed(&settings());
    config["onebot.whitelist"] = json!([format!("qq:{JIE}")]);
    config
}

/// 桥问 `venue.session` 的参数里场所是 `venue` 的那一条。
fn asked(relay: &Relay, venue: &str) -> Value {
    relay
        .sessions()
        .into_iter()
        .find(|params| params["venue"] == venue)
        .unwrap_or_else(|| panic!("桥没问过 {venue}：{:?}", relay.sessions()))
}

#[tokio::test]
async fn only_the_admins_private_chat_asks_for_the_profile() {
    let script = Script::new([Play::Says("在。"), Play::Says("嗯。")]);
    let home = Home::new(&script);
    let (serve, relay) = serve_handed(home.root.clone(), whitelisted(), None);
    let bridge = start(serve).await;
    let mut napcat = admin_napcat(bridge.port).await;
    napcat.private(JIE, 1, json!("在吗")).await;
    napcat
        .send(group_frame(
            GROUP,
            LIN,
            2,
            json!([plain("大家好")]),
            ("阿林", "lin"),
        ))
        .await;
    napcat.admin_says(3, "在吗").await;
    assert_eq!(napcat.reply().await, "在。");
    within("三个场所都问过", async {
        while relay.sessions().len() < 3 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await;
    let admin = asked(&relay, &format!("qq:private:{ADMIN}"));
    assert_eq!(admin["profile"], true, "终端管理员的私聊要：{admin}");
    let listed = asked(&relay, &format!("qq:private:{JIE}"));
    assert!(
        listed.get("profile").is_none(),
        "白名单成员的私聊不写：{listed}"
    );
    let group = asked(&relay, &format!("qq:group:{GROUP}"));
    assert!(group.get("profile").is_none(), "群不写：{group}");
    bridge.stop().await.expect("停得下");
}

/// 系统配置的 `[onebot]` 多写的：白名单里只有 [`JIE`]。
fn whitelist() -> String {
    format!("whitelist = [\"qq:{JIE}\"]\n")
}

/// 下一个动作是发给 `user` 的 `send_private_msg`：交回里面的字。
async fn private_reply(napcat: &mut Answering, user: i64) -> String {
    let action = napcat.action().await;
    assert_eq!(action["action"], "send_private_msg", "{action}");
    assert_eq!(action["params"]["user_id"], user, "{action}");
    action["params"]["message"][0]["data"]["text"]
        .as_str()
        .expect("有字")
        .to_string()
}

#[tokio::test]
async fn the_admins_private_chat_knows_the_profile_and_the_whitelisted_one_does_not() {
    let script = Script::new([Play::Says("在。"), Play::Says("嗯。")]);
    let (home, mut napcat, _) = started(&script, "", &whitelist(), MEMBERS).await;
    let mut core = miyu_client::open::Core::connect_running(&home.root, "test")
        .await
        .expect("连得上核心");
    let changes = json!({"layer": "personal", "changes": [
        {"key": "profile.name", "value": NAME},
        {"key": "profile.about", "value": ABOUT},
    ]});
    let reply = core
        .call("profile-1", "config.set", changes)
        .await
        .expect("改得了");
    assert!(reply.get("error").is_none(), "{reply}");
    napcat.send(private_frame(JIE, 1, json!([plain("在吗")])));
    assert_eq!(private_reply(&mut napcat, JIE).await, "在。");
    napcat.send(private_frame(ADMIN, 2, json!([plain("在吗")])));
    assert_eq!(private_reply(&mut napcat, ADMIN).await, "嗯。");
    let systems: Vec<String> = script
        .requests()
        .iter()
        .map(|(_, request)| request.system.clone())
        .collect();
    assert_eq!(systems.len(), 2, "{systems:?}");
    assert!(
        !systems[0].contains("<user>"),
        "白名单成员的私聊不带：{}",
        systems[0]
    );
    assert!(
        systems[1].contains(BLOCK),
        "终端管理员的私聊带：{}",
        systems[1]
    );
    stopped(home).await;
}
