//! 自己的平台身份（施工 BD-1，项目主人 10-27 定）：成员在个人设置里写 `external.me`，说这几个号是他。他那一份 im 问对应表认得出
//! 他，他群里这个号说的话记成他本人；机主那一份不认他写的（号在机主的群里照旧是外部身份）。

use std::path::PathBuf;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::id::AccountId;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;

use crate::support::extensions::*;
use crate::support::login::member;
use crate::support::served::replies;
use crate::support::*;

fn account(name: &str) -> AccountId {
    AccountId::parse(name).expect("合写法")
}

/// 包 `each` 替 `name` 的那一份记的文件。
fn log_of(home: &Home, name: &str) -> PathBuf {
    home.root
        .account_dir(&account(name))
        .join("state")
        .join("packages")
        .join("each")
        .join("calls.log")
}

/// 会话 `reply` 里人说的话是谁说的。
fn speakers(home: &Home, owner: &str, reply: &Value) -> Vec<By> {
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"));
    read_events(&dir(home, &account(owner), session))
        .expect("会话在他名下")
        .into_iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.by)
        .collect()
}

#[tokio::test]
async fn a_member_s_own_numbers_are_him_only_in_his_own_copy() {
    let home = Home::new();
    let program = Program::new();
    let capabilities = ["act_for_external", "sessions.drive"];
    install_asking(
        &home,
        "each",
        &program.name(),
        "always",
        &steps(&[
            "record:calls.log",
            "hello",
            r#"ask:provide:{"tools":[]}"#,
            r#"ask:venue.session:{"venue":"qq:group:1","kind":"group"}"#,
            r#"ask:session.send:{"session":"{session}","text":"我的号","as":{"external":"qq:20002"}}"#,
            r#"ask:venue.binding:{"id":"qq:20002"}"#,
            r#"ask:package.methods:{"methods":[{"name":"whoami"}]}"#,
            "serve",
        ]),
        &capabilities,
    );
    approve(&home, "each", &capabilities);
    let manifest = home
        .root
        .system_packages()
        .join("each")
        .join("package.toml");
    let text = read(&manifest)
        .replacen("[package]\n", "[package]\nmembers = true\n", 1)
        .replacen("[process]\n", "[process]\nscope = \"account\"\n", 1)
        + "\n[connection]\nplatform = \"qq\"\n";
    std::fs::write(&manifest, text).expect("写得进");
    let says = || Play::Says("嗯。");
    let core = core_with_script(&home, quick(), &Script::new([says(), says()]));
    // 机主那一份：头一回装上就起了，没人说 qq:20002 是自己。
    let mine = replies(&log_of(&home, "alice"), 5).await;

    // 成员写上自己的号，再用一下，他那一份起来。
    let mut bob = member(&core, "bob").await;
    let set = bob
        .call(
            "c-1d7f3a9b5e2c4086-1",
            "config.set",
            json!({"layer": "personal", "changes": [{"key": "external.me", "value": ["qq:20002"]}]}),
        )
        .await;
    assert!(set["result"].is_object(), "{set}");
    let called = bob
        .call(
            "c-1d7f3a9b5e2c4086-2",
            "package.call",
            json!({"package": "each", "method": "whoami"}),
        )
        .await;
    assert!(called["result"].is_object(), "{called}");
    let theirs = replies(&log_of(&home, "bob"), 5).await;

    assert_eq!(
        theirs[4]["result"]["account"], "bob",
        "他那一份认得出他：{theirs:?}"
    );
    let said = speakers(&home, "bob", &theirs[2]);
    assert!(
        matches!(said.as_slice(), [By::Person(person)] if person.account == account("bob")
            && person.via.as_ref().map(|via| via.as_str()) == Some("qq:20002")),
        "他群里这个号是他本人：{said:?}"
    );
    assert!(
        mine[4]["result"]["account"].is_null(),
        "机主那一份不认：{mine:?}"
    );
    let said = speakers(&home, "alice", &mine[2]);
    assert!(
        matches!(said.as_slice(), [By::External(external)] if external.account.is_none()),
        "{said:?}"
    );
    core.stop_extensions().await;
}
