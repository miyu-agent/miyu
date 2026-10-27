//! 接几个平台的包（施工 IM-3，接入通讯平台设计 07 第 4.6 节第 1 样）：清单 `[connection] platform` 写列表；它登记的、名字以
//! `<平台>_` 开头的工具只给那个平台的场所会话（`tg:` 的群里不给 `qq_*`），本机的会话都给；不带平台前缀的照旧都给。这几个平台
//! 的场所都算它的：主人在第二个平台的私聊归主人，桥碰得到、列得出。软件包列表的 `connection` 照旧带第一个，另带全部 `platforms`。

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_session::testkit::{Play, Script};

use crate::support::extensions::*;
use crate::support::served::{replies, served_core_with};
use crate::support::venues::{BINDINGS, snapshot};
use crate::support::*;

/// 桥登记的几件：两个平台各一件，一件不分平台，一件名字像但没有下划线。
fn tools() -> Value {
    let one = |name: &str| {
        json!({"name": name, "description": "A test tool.", "input_schema": {"type": "object"},
               "access": "read", "venues": ["local", "group", "private"]})
    };
    json!({"tools": [one("qq_send"), one("tg_send"), one("note"), one("qqnote")]})
}

/// 账号 `account` 名下会话 `reply` 的工具面上的工具名，照名字排。
fn face(home: &Home, account: &AccountId, reply: &Value) -> Vec<String> {
    let session = reply["result"]["session"]
        .as_str()
        .unwrap_or_else(|| panic!("{reply}"));
    let mut names: Vec<String> = snapshot(home, account, &dir(home, account, session))
        .tools
        .into_iter()
        .map(|tool| tool.name)
        .filter(|name| ["qq_send", "tg_send", "note", "qqnote"].contains(&name.as_str()))
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn a_platform_s_tools_are_only_given_in_its_own_venues() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "bridge");
    install_serving(
        &home,
        "bridge",
        &program.name(),
        &steps(&[
            &step,
            "hello",
            &format!("ask:provide:{}", tools()),
            r#"ask:venue.session:{"venue":"tg:group:1","kind":"group"}"#,
            r#"ask:venue.session:{"venue":"qq:group:2","kind":"group"}"#,
            r#"ask:session.create:{"cwd":"~"}"#,
            r#"ask:venue.plugins:{"venue":"tg:group:1","on":[]}"#,
            r#"ask:venue.plugins:{"venue":"discord:group:3","on":[]}"#,
            // 主人在第二个平台的号：私聊归主人，桥碰得到、列得出。
            r#"ask:venue.session:{"venue":"tg:private:777","kind":"private","peer":"tg:777"}"#,
            r#"ask:session.send:{"session":"{session}","text":"在吗","as":{"external":"tg:777"}}"#,
            "ask:venue.sessions:{}",
            "wait",
        ]),
        "always",
        true,
    );
    let manifest = home
        .root
        .system_packages()
        .join("bridge")
        .join("package.toml");
    let text = read(&manifest) + "\n[connection]\nplatform = [\"qq\", \"tg\"]\n";
    std::fs::write(&manifest, text).expect("写得进");
    let config = format!("{BINDINGS}\"tg:777\" = \"alice\"\n");
    let core = served_core_with(&home, &Script::new([Play::Says("在。")]), &config);
    let got = replies(&path, 10).await;
    let bridge = AccountId::parse("bridge").expect("合写法");
    assert_eq!(got[1]["result"]["tools"], 4, "{got:?}");
    assert_eq!(
        face(&home, &bridge, &got[2]),
        ["note", "qqnote", "tg_send"],
        "tg 的群"
    );
    assert_eq!(
        face(&home, &bridge, &got[3]),
        ["note", "qq_send", "qqnote"],
        "qq 的群"
    );
    assert_eq!(
        face(&home, &bridge, &got[4]),
        ["note", "qq_send", "qqnote", "tg_send"],
        "本机的会话都给"
    );
    // 两个平台的场所都是它的，别的平台的不是。
    assert_eq!(got[5]["result"], json!({}), "{got:?}");
    assert_eq!(got[6]["error"]["data"]["field"], "venue", "{got:?}");
    assert_eq!(got[7]["result"]["account"], "alice", "{got:?}");
    assert!(
        got[8].get("error").is_none(),
        "桥碰得到主人 tg 的私聊：{got:?}"
    );
    let private = &got[7]["result"]["session"];
    assert!(
        got[9]["result"]["sessions"]
            .as_array()
            .is_some_and(|listed| listed.iter().any(|one| &one["session"] == private)),
        "列得出主人 tg 的私聊：{got:?}"
    );
    // 软件包列表：`platform` 照旧是第一个，另带全部。
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    let listed = client.call("l1", "package.list", json!({})).await;
    let item = listed["result"]["packages"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["package"] == "bridge"))
        .cloned()
        .unwrap_or_else(|| panic!("{listed}"));
    assert_eq!(
        item["connection"],
        json!({"platform": "qq", "platforms": ["qq", "tg"]}),
        "{item}"
    );
    core.stop_extensions().await;
}
