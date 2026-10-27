//! 每人一份的包在场所里（施工 IM-2，接入通讯平台设计 06 第 4.6 节第 2 样）：替谁跑，场所会话就归谁，私聊对方在主人对应表里的
//! 也一样；主人对应表只在主人自己那一份里认（成员那一份问对应表是 `null`、群里说话的记成外部身份不带账号）；列场所会话只列他
//! 自己名下的、这个包的平台的；它自己开的会话里不带 `as` 跑的命令记成他本人。

use std::path::{Path, PathBuf};

use serde_json::Value;

use miyu_kernel::event::Body;
use miyu_kernel::id::AccountId;
use miyu_kernel::origin::By;
use miyu_session::testkit::{Play, Script};
use miyu_store::log::read_events;

use crate::support::extensions::*;
use crate::support::login::member;
use crate::support::served::replies;
use crate::support::venues::BINDINGS;
use crate::support::*;

/// 包 `each` 替 `account` 的那一份记的文件。
fn log_of(home: &Home, account: &str) -> PathBuf {
    home.root
        .account_dir(&account_id(account))
        .join("state")
        .join("packages")
        .join("each")
        .join("calls.log")
}

fn account_id(name: &str) -> AccountId {
    AccountId::parse(name).expect("合写法")
}

/// 会话 `session`（属主 `owner`）里人说的话是谁说的，照先后。
fn speakers(home: &Home, owner: &str, session: &Value) -> Vec<By> {
    let dir = dir(home, &account_id(owner), session.as_str().expect("有编号"));
    read_events(&dir)
        .expect("会话在他名下")
        .into_iter()
        .filter(|event| matches!(event.body, Body::MessageUser(_)))
        .map(|event| event.by)
        .collect()
}

/// 记下的回应里，`venue.sessions` 列出的会话编号。
fn listed(reply: &Value) -> Vec<&str> {
    let mut found: Vec<&str> = reply["result"]["sessions"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|one| one["session"].as_str())
        .collect();
    found.sort_unstable();
    found
}

/// 等 `path` 里的回应齐了：握手、登记工具、群、一句话、私聊、问对应表、别的平台的群、列场所会话、开会话、在那里跑一个命令、登记
/// 方法。
async fn answered(path: &Path) -> Vec<Value> {
    replies(path, 11).await
}

#[tokio::test]
async fn each_person_s_copy_owns_its_venues_and_reads_the_owner_s_table_only_for_the_owner() {
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
            r#"ask:session.send:{"session":"{session}","text":"主人的号说的","as":{"external":"qq:10001"}}"#,
            r#"ask:venue.session:{"venue":"qq:private:10001","kind":"private","peer":"qq:10001"}"#,
            r#"ask:venue.binding:{"id":"qq:10001"}"#,
            r#"ask:venue.session:{"venue":"tg:group:9","kind":"group"}"#,
            "ask:venue.sessions:{}",
            r#"ask:session.create:{"cwd":"~"}"#,
            r#"ask:command.run:{"session":"{session}","text":"/stop"}"#,
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
    home.write("system/config.toml", BINDINGS);
    let says = || Play::Says("嗯。");
    let core = core_with_script(
        &home,
        quick(),
        &Script::new([says(), says(), says(), says()]),
    );

    // 机主那一份：头一回装上替他起的。
    let mine = answered(&log_of(&home, "alice")).await;
    assert_eq!(mine[2]["result"]["account"], "alice", "{mine:?}");
    assert_eq!(mine[4]["result"]["account"], "alice", "私聊归他：{mine:?}");
    assert_eq!(
        mine[5]["result"]["account"], "alice",
        "主人对应表在他那一份里认"
    );
    let owned = [&mine[2], &mine[4]].map(|reply| reply["result"]["session"].as_str().unwrap());
    let mut owned = owned.to_vec();
    owned.sort_unstable();
    assert_eq!(listed(&mine[7]), owned, "{:?}", mine[7]);

    // 成员那一份：用了一下后台页起来的。
    let mut bob = member(&core, "bob").await;
    let called = bob
        .call(
            "c-3e8a1d5f7b2c4096-1",
            "package.call",
            serde_json::json!({"package": "each", "method": "whoami"}),
        )
        .await;
    assert!(called["result"].is_object(), "{called}");
    let theirs = answered(&log_of(&home, "bob")).await;
    assert_eq!(theirs[2]["result"]["account"], "bob", "{theirs:?}");
    assert_eq!(
        theirs[4]["result"]["account"], "bob",
        "对方在主人对应表里也归他：{theirs:?}"
    );
    assert!(
        theirs[5]["result"]["account"].is_null(),
        "主人对应表不给他：{theirs:?}"
    );
    let group = &theirs[2]["result"]["session"];
    let private = &theirs[4]["result"]["session"];
    assert_ne!(group, &mine[2]["result"]["session"], "各是各的");
    let mut own = vec![group.as_str().unwrap(), private.as_str().unwrap()];
    own.sort_unstable();
    assert_eq!(
        listed(&theirs[7]),
        own,
        "只列他自己的、这个包的平台的（不列 tg 的群）：{:?}",
        theirs[7]
    );

    // 说话的人：他的群里主人的号是外部身份、不带账号；不带 as 的是他本人。机主的群里是机主本人。
    let said = speakers(&home, "bob", group);
    assert!(
        matches!(&said[0], By::External(external) if external.account.is_none()),
        "{said:?}"
    );
    // 它自己开的会话在他名下，不带 as 跑的命令记成他本人（原来一律是管理员）。
    let made = theirs[8]["result"]["session"].as_str().expect("有编号");
    let ran: Vec<By> = read_events(&dir(&home, &account_id("bob"), made))
        .expect("在他名下")
        .into_iter()
        .filter(|event| matches!(event.body, Body::CommandRan(_)))
        .map(|event| event.by)
        .collect();
    assert!(
        matches!(ran.as_slice(), [By::Person(person)] if person.account == account_id("bob")),
        "{ran:?} {theirs:?}"
    );
    let said = speakers(&home, "alice", &mine[2]["result"]["session"]);
    assert!(
        matches!(&said[0], By::Person(person) if person.account == account_id("alice")),
        "{said:?}"
    );
    core.stop_extensions().await;
}
