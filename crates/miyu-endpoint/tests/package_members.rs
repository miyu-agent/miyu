//! 成员也能用（施工 IM-1 上，设计 06 第八再又补第 2 条）：系统装的带进程的包默认只有管理员组的人能用，成员新开的会话没有它的
//! 工具；管理员 `package.members` 勾上以后成员有了，再取消又没了，`package.list` 那一项照它写 `members`（只给管理员）。清单写了
//! `[package] members = true` 的默认就有，管理员改过的照他的。不带进程的包不管这一格，`package.members` 拒 `not_switchable`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::scripts::{core, echo};
use crate::support::login::member;
use crate::support::*;

/// 新会话会拿到的工具名（`session.tools` 不写会话）。
async fn face(client: &mut Client) -> Vec<String> {
    let reply = client.call("t", "session.tools", json!({})).await;
    reply["result"]["tools"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter_map(|one| one["name"].as_str().map(str::to_string))
        .collect()
}

/// 系统装一个带进程的包 `id`，带一个脚本 `tool`；`members` 是清单里写不写 `members = true`，写了的同时是每人一份的。
fn extension(home: &Home, id: &str, tool: &str, members: bool) {
    let (line, scope) = match members {
        true => ("members = true\n", "scope = \"account\"\n"),
        false => ("", ""),
    };
    home.write(
        &format!("system/packages/{id}/package.toml"),
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"{id}\" }}\n{line}\n[command]\nname = \"{id}\"\nprogram = \"miyu-{id}\"\nabout = {{ en = \"X\" }}\n\n[process]\n{scope}"
        ),
    );
    home.write(
        &format!("system/packages/{id}/scripts/{tool}.py"),
        &echo(tool),
    );
}

/// 包 `id` 在 `package.list` 里的那一项。
async fn listed(client: &mut Client, id: &str) -> Value {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["package"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("列着 {id}：{reply}"))
}

#[tokio::test]
async fn members_get_a_process_package_only_when_it_is_ticked() {
    let home = Home::new();
    extension(&home, "ext", "dice", false);
    extension(&home, "open", "coin", true);
    home.write(
        "system/packages/kit/package.toml",
        "[package]\nname = { en = \"Kit\" }\n",
    );
    home.write("system/packages/kit/scripts/card.py", &echo("card"));
    let core = core(&home, &Arc::new(Script::new([])));
    let mut alice = Client::connect(Arc::clone(&core));
    alice.hello().await;
    let mut bob = member(&core, "bob").await;

    let mine = face(&mut alice).await;
    assert!(mine.contains(&"dice".to_string()), "管理员有：{mine:?}");
    let his = face(&mut bob).await;
    assert!(
        !his.contains(&"dice".to_string()),
        "没勾的成员没有：{his:?}"
    );
    assert!(
        his.contains(&"coin".to_string()),
        "清单写了的默认有：{his:?}"
    );
    assert!(his.contains(&"card".to_string()), "不带进程的不管：{his:?}");
    assert_eq!(listed(&mut alice, "ext").await["members"], false);
    assert_eq!(listed(&mut alice, "open").await["members"], true);
    assert!(
        listed(&mut alice, "kit").await.get("members").is_none(),
        "不带进程的没有这一格"
    );
    assert!(
        listed(&mut bob, "ext").await.get("members").is_none(),
        "只给管理员"
    );
    assert_eq!(
        listed(&mut alice, "open").await["process"]["scope"],
        "account"
    );
    assert!(
        listed(&mut alice, "ext").await["process"]
            .get("scope")
            .is_none(),
        "一份的不写 scope"
    );

    let ticked = alice
        .call(
            "m1",
            "package.members",
            json!({"package": "ext", "on": true}),
        )
        .await;
    assert_eq!(ticked["result"]["members"], true, "{ticked}");
    assert!(
        face(&mut bob).await.contains(&"dice".to_string()),
        "勾上了有"
    );
    let unticked = alice
        .call(
            "m2",
            "package.members",
            json!({"package": "open", "on": false}),
        )
        .await;
    assert_eq!(unticked["result"]["members"], false, "{unticked}");
    assert!(
        !face(&mut bob).await.contains(&"coin".to_string()),
        "管理员取消的照他的"
    );
    assert!(
        face(&mut alice).await.contains(&"coin".to_string()),
        "管理员照旧有"
    );

    let plain = alice
        .call(
            "m3",
            "package.members",
            json!({"package": "kit", "on": true}),
        )
        .await;
    assert_eq!(
        plain["error"]["data"]["reason"], "not_switchable",
        "{plain}"
    );
    let theirs = bob
        .call(
            "m4",
            "package.members",
            json!({"package": "ext", "on": false}),
        )
        .await;
    assert!(theirs["error"].is_object(), "成员改不了：{theirs}");
    assert!(face(&mut bob).await.contains(&"dice".to_string()), "没改成");
}
