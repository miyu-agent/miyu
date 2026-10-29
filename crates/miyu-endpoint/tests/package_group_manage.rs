//! 装、卸整个组（施工 BS-1 下中，设计 `44-基础系统拆包.md` 第三节第 2 条）：`package.remove`、`package.install` 写组名
//! `basesystem`，组里的出厂包一起卸掉、一起装回来；带 `preview` 的只说会动哪几个；组外的包依赖组里的整个拒。

use serde_json::{Value, json};

use crate::support::login::local;
use crate::support::*;

/// 包列表里装着的（不是卸掉的）编号。
async fn installed(client: &mut Client) -> Vec<String> {
    let listed = client.call("l", "package.list", json!({})).await;
    listed["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{listed}"))
        .iter()
        .filter(|item| item.get("removed").is_none())
        .filter_map(|item| item["package"].as_str().map(str::to_string))
        .collect()
}

fn ids(value: &Value) -> Vec<String> {
    value
        .as_array()
        .unwrap_or_else(|| panic!("{value}"))
        .iter()
        .map(|item| match item {
            Value::String(id) => id.clone(),
            other => other["package"].as_str().unwrap_or_default().to_string(),
        })
        .collect()
}

#[tokio::test]
async fn the_base_system_goes_and_comes_back_as_a_group() {
    let home = Home::new();
    let core = packaged::core(&home);
    let mut owner = local(&core).await;
    let looked = owner
        .call(
            "p",
            "package.remove",
            json!({"package": "basesystem", "preview": true}),
        )
        .await;
    let group = ids(&looked["result"]["packages"]);
    assert_eq!(looked["result"]["group"], "basesystem", "{looked}");
    assert!(
        group.contains(&"miyu-base-shell".to_string())
            && group.contains(&"miyu-base-read".to_string()),
        "{looked}"
    );
    assert!(
        !group.contains(&"miyu-base-memory".to_string()),
        "组外的不动：{looked}"
    );
    assert!(
        installed(&mut owner)
            .await
            .contains(&"miyu-base-shell".to_string()),
        "看一眼不卸"
    );

    let removed = owner
        .call("r", "package.remove", json!({"package": "basesystem"}))
        .await;
    let mut gone = ids(&removed["result"]["removed"]);
    gone.sort();
    let mut expected = group.clone();
    expected.sort();
    assert_eq!(gone, expected, "{removed}");
    let now = installed(&mut owner).await;
    assert!(group.iter().all(|id| !now.contains(id)), "全卸了：{now:?}");
    assert!(now.contains(&"miyu-base-memory".to_string()), "组外的留着");

    let back = owner
        .call(
            "b",
            "package.install",
            json!({"package": "basesystem", "preview": true}),
        )
        .await;
    let mut restoring = ids(&back["result"]["packages"]);
    restoring.sort();
    assert_eq!(restoring, expected, "会装回的就是卸掉的那几个：{back}");
    let restored = owner
        .call("i", "package.install", json!({"package": "basesystem"}))
        .await;
    assert_eq!(restored["result"]["group"], "basesystem", "{restored}");
    let mut came = ids(&restored["result"]["packages"]);
    came.sort();
    assert_eq!(came, expected, "{restored}");
    let now = installed(&mut owner).await;
    assert!(
        group.iter().all(|id| now.contains(id)),
        "全装回来了：{now:?}"
    );
}

#[tokio::test]
async fn a_package_outside_needing_the_group_stops_its_removal() {
    let home = Home::new();
    home.write(
        "packages/kit/package.toml",
        "[package]\nname = { en = \"Kit\" }\n\n[depends]\npackages = [\"miyu-base-shell\"]\n",
    );
    home.write(
        "packages/kit/skills/notes/SKILL.md",
        "---\nname: notes\ndescription: Take notes.\n---\nbody\n",
    );
    let core = packaged::core(&home);
    let mut owner = local(&core).await;
    let refused = owner
        .call("r", "package.remove", json!({"package": "basesystem"}))
        .await;
    assert_eq!(
        refused["error"]["data"]["reason"], "package_depended",
        "{refused}"
    );
    assert_eq!(refused["error"]["data"]["by"], json!(["kit"]), "{refused}");
    let now = installed(&mut owner).await;
    assert!(
        now.contains(&"miyu-base-shell".to_string()) && now.contains(&"miyu-base-read".to_string()),
        "整个拒，组里一个都没卸：{now:?}"
    );
}

/// 组里只卸了一个的：装回组只装回它，装着的不动（施工 BS-1 下中）。
#[tokio::test]
async fn restoring_the_group_brings_back_only_what_was_removed() {
    let home = Home::new();
    let core = packaged::core(&home);
    let mut owner = local(&core).await;
    let removed = owner
        .call("r", "package.remove", json!({"package": "miyu-base-trash"}))
        .await;
    assert!(removed.get("result").is_some(), "{removed}");
    let looked = owner
        .call(
            "p",
            "package.install",
            json!({"package": "basesystem", "preview": true}),
        )
        .await;
    assert_eq!(
        looked["result"]["packages"],
        json!(["miyu-base-trash"]),
        "{looked}"
    );
    let back = owner
        .call("i", "package.install", json!({"package": "basesystem"}))
        .await;
    assert_eq!(
        ids(&back["result"]["packages"]),
        ["miyu-base-trash"],
        "{back}"
    );
}
