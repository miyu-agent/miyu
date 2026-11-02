//! 出厂带的软件工程师（施工 PE-1，2026-10-31 项目主人定，`docs/blueprint/personas.md`「软件包带的」）：真核心走一遍。全新的
//! 数据根上就列得出、名字照连接的语言，默认人格照旧不设；改了写你那一层、下一次读到改后的，删掉改动回到出厂的样子；你那一层
//! 没有的再删就是卸它的出厂包（`package.list` 里 `removed: true`），写包编号装回来又有。装上的包带的照旧删不掉。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 出厂带的人格的编号。
const ENGINEER: &str = "software-engineer";

/// 带它的出厂包。
const PACKAGE: &str = "miyu-base-persona-engineer";

/// 连上一份核心，握手时说 `locale` 这种语言。
async fn connected(home: &Home, locale: &str) -> Client {
    let mut client = Client::connect(home.core(&Script::new([])));
    let hello = client
        .call(
            "hello-1",
            "hello",
            json!({
                "protocol": [1, 1],
                "head": {"kind": "test", "version": "0.0.0"},
                "locale": locale,
                "caps": {"input": true},
                "token": TOKEN,
            }),
        )
        .await;
    assert!(hello.get("error").is_none(), "{hello}");
    client
}

/// 列表里编号是 `id` 的那一项（`key` 是 `persona` 或 `package`）；没有的是 `null`。
fn one(listed: &Value, key: &str, id: &str) -> Value {
    listed["result"][format!("{key}s")]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .find(|item| item[key] == id)
        .cloned()
        .unwrap_or(Value::Null)
}

async fn listed(client: &mut Client, id: &str) -> Value {
    one(
        &client.call(id, "persona.list", json!({})).await,
        "persona",
        ENGINEER,
    )
}

#[tokio::test]
async fn a_fresh_home_ships_the_software_engineer_named_in_your_language() {
    let home = Home::new();
    let mut client = connected(&home, "zh-CN").await;
    let item = listed(&mut client, "l1").await;
    assert_eq!(item["name"], "软件工程师", "{item}");
    assert_eq!(
        item["summary"],
        "写代码：先用最简单的办法，用独立的依据验证，做完验过就收工，说话简洁"
    );
    let got = client
        .call("g1", "persona.get", json!({"persona": ENGINEER}))
        .await;
    assert_eq!(got["result"]["subagent"], true, "{got}");
    assert_eq!(got["result"]["prompts"]["persona"], true, "{got}");
    assert_eq!(got["result"]["remove"], "delete", "删了就没了：{got}");
    assert!(
        got["result"].get("layer").is_none(),
        "来自哪一层不露：{got}"
    );

    let mut english = connected(&home, "en-US").await;
    let item = listed(&mut english, "l2").await;
    assert_eq!(item["name"], "Software Engineer", "{item}");

    // 默认人格照旧不设：不写人格开的会话是无人格的。
    let session = client.create("c1", "~").await;
    let created = home
        .log(&session)
        .into_iter()
        .find_map(|event| match event.body {
            miyu_kernel::event::Body::SessionCreated(created) => Some(created),
            _ => None,
        })
        .expect("有造会话那一条");
    assert_eq!(created.persona, None);
}

#[tokio::test]
async fn deleting_it_removes_its_shipped_package_and_installing_brings_it_back() {
    let home = Home::new();
    let mut client = connected(&home, "zh-CN").await;
    let deleted = client
        .call("d1", "persona.delete", json!({"persona": ENGINEER}))
        .await;
    assert_eq!(deleted["result"], json!({"remains": false}), "{deleted}");
    assert_eq!(listed(&mut client, "l1").await, Value::Null, "列表里没了");
    let gone = client
        .call("g1", "persona.get", json!({"persona": ENGINEER}))
        .await;
    assert_eq!(gone["error"]["data"]["reason"], "unknown_persona", "{gone}");
    let packages = client.call("p1", "package.list", json!({})).await;
    assert_eq!(
        one(&packages, "package", PACKAGE)["removed"],
        true,
        "{packages}"
    );
    assert!(
        home.root
            .path()
            .join(format!("etc/removed/{PACKAGE}"))
            .exists(),
        "同 package.remove 记一笔"
    );
    let again = client
        .call("d2", "persona.delete", json!({"persona": ENGINEER}))
        .await;
    assert_eq!(
        again["error"]["data"]["reason"], "nothing_to_delete",
        "{again}"
    );

    let installed = client
        .call("i1", "package.install", json!({"package": PACKAGE}))
        .await;
    assert!(installed["result"].is_object(), "{installed}");
    assert_eq!(listed(&mut client, "l2").await["name"], "软件工程师");
}

#[tokio::test]
async fn editing_it_writes_your_copy_and_deleting_that_goes_back_to_the_shipped_one() {
    let home = Home::new();
    let mut client = connected(&home, "zh-CN").await;
    let saved = client
        .call(
            "s1",
            "persona.set",
            json!({
                "persona": ENGINEER,
                "changes": [{"key": "persona.name", "value": "我的工程师"}],
                "prompts": {"persona": {"text": "Be terse.\n"}},
            }),
        )
        .await;
    assert_eq!(saved["result"]["remove"], "restore", "{saved}");
    let got = client
        .call("g1", "persona.get", json!({"persona": ENGINEER}))
        .await;
    assert_eq!(got["result"]["name"], "我的工程师", "{got}");
    let read = client
        .call(
            "r1",
            "persona.read",
            json!({"persona": ENGINEER, "prompt": "persona"}),
        )
        .await;
    assert_eq!(read["result"]["text"], "Be terse.\n", "{read}");

    let deleted = client
        .call("d1", "persona.delete", json!({"persona": ENGINEER}))
        .await;
    assert_eq!(deleted["result"], json!({"remains": true}), "{deleted}");
    assert_eq!(listed(&mut client, "l1").await["name"], "软件工程师");
    let packages = client.call("p1", "package.list", json!({})).await;
    assert!(
        one(&packages, "package", PACKAGE).get("removed").is_none(),
        "先删的是你那一层，包还在：{packages}"
    );
}

/// 装上的（不是出厂的）包带的照旧删不掉。
#[tokio::test]
async fn a_persona_from_an_installed_package_still_cannot_be_deleted() {
    let home = Home::new();
    home.write(
        "opt/kitty/package.toml",
        "[package]\nname = { en = \"Kitty\" }\n",
    );
    home.write("opt/kitty/personas/cat/persona.toml", "[persona]\n");
    let mut client = connected(&home, "zh-CN").await;
    let got = client
        .call("g1", "persona.get", json!({"persona": "cat"}))
        .await;
    assert_eq!(got["result"]["remove"], Value::Null, "{got}");
    let refused = client
        .call("d1", "persona.delete", json!({"persona": "cat"}))
        .await;
    assert_eq!(
        refused["error"]["data"]["reason"], "nothing_to_delete",
        "{refused}"
    );
    assert!(home.root.path().join("opt/kitty/personas/cat").is_dir());
}
