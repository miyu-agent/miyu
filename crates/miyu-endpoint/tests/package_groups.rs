//! 软件包列表里的组（施工 BS-1 下，设计 `44-基础系统拆包.md` 第三节第 2 条）：每一项写它归哪几个组，回应顶上列出组表里的组，
//! 名字照连接的语言；出厂清单写的组都在组表里，没有哪个包的编号和组名撞。

use serde_json::{Value, json};

use crate::support::*;

/// 出厂的组表和清单。
fn shipped() -> (
    Vec<miyu_config::package::groups::Group>,
    Vec<miyu_store::packages::Found>,
) {
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let text =
        std::fs::read_to_string(default_resources().join("packages/groups.toml")).expect("有组表");
    let groups = miyu_config::package::groups::read(&text).expect("组表写对了");
    (
        groups,
        miyu_store::packages::Packages::shipped(&resources).read(),
    )
}

#[test]
fn shipped_manifests_name_only_groups_in_the_table() {
    let (groups, found) = shipped();
    let ids: Vec<&str> = groups.iter().map(|group| group.id.as_str()).collect();
    for one in &found {
        let manifest = one
            .read
            .as_ref()
            .unwrap_or_else(|_| panic!("{} 读得成", one.id));
        for group in &manifest.groups {
            assert!(
                ids.contains(&group.as_str()),
                "{} 写的组 {group} 在组表里",
                one.id
            );
        }
        assert!(!ids.contains(&one.id.as_str()), "包 {} 和组撞名", one.id);
    }
    // 组表的 `order`（施工 PG-1）：写到的都是这个组里的出厂包，这个组里的出厂包都写到了。
    for group in &groups {
        let members: Vec<&str> = found
            .iter()
            .filter(|one| {
                one.read
                    .as_ref()
                    .is_ok_and(|manifest| manifest.groups.contains(&group.id))
            })
            .map(|one| one.id.as_str())
            .collect();
        for id in &group.order {
            assert!(
                members.contains(&id.as_str()),
                "{} 的 order 写了不在组里的 {id}",
                group.id
            );
        }
        for id in &members {
            assert!(
                group.order.iter().any(|one| one == id),
                "{} 的 order 漏了 {id}",
                group.id
            );
        }
    }
}

#[tokio::test]
async fn the_list_says_which_group_each_package_is_in() {
    let home = Home::new();
    let core = packaged::core(&home);
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    let listed = client.call("l", "package.list", json!({})).await;
    let result = &listed["result"];
    let mut group = result["groups"][0].clone();
    let packages = group
        .as_object_mut()
        .and_then(|fields| fields.remove("packages"))
        .unwrap_or_else(|| panic!("组带 packages（施工 PG-1）：{listed}"));
    assert_eq!(
        result["groups"].as_array().map(Vec::len),
        Some(5),
        "出厂五个组（施工 FG-1，设计 52 第一节）：{listed}"
    );
    assert_eq!(
        group,
        json!({"id": "basesystem", "name": "基础系统", "summary": "读写文件、跑命令、查以前说过的话"}),
        "名字照连接的语言：{listed}"
    );
    let packages: Vec<&str> = packages
        .as_array()
        .expect("一串")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    assert_eq!(
        packages[..7],
        [
            "miyu-base-read",
            "miyu-base-write",
            "miyu-base-edit",
            "miyu-base-glob",
            "miyu-base-grep",
            "miyu-base-trash",
            "miyu-base-shell"
        ],
        "照用途的先后：{packages:?}"
    );
    assert!(!packages.contains(&"miyu-base-memory"), "别的组里的不列");
    let order: Vec<&Value> = result["groups"]
        .as_array()
        .expect("一串")
        .iter()
        .map(|group| &group["id"])
        .collect();
    assert_eq!(
        order,
        ["basesystem", "tasks", "environment", "persona", "research"],
        "照组表的先后"
    );
    let groups_of = |id: &str| -> Value {
        result["packages"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["package"] == id))
            .map(|item| item["groups"].clone())
            .unwrap_or_else(|| panic!("有 {id}：{listed}"))
    };
    assert_eq!(groups_of("miyu-base-shell"), json!(["basesystem"]));
    assert_eq!(groups_of("miyu-base-memory"), json!(["persona"]));
    assert_eq!(
        groups_of("miyu-base-authoring"),
        Value::Null,
        "不在组里的不写"
    );
}

/// 装的时刻（施工 PG-1，预设页照装的先后排）：装上的包带 `installed_at`，出厂的不写。
#[tokio::test]
async fn installed_packages_say_when_they_were_installed() {
    let home = Home::new();
    let core = packaged::core(&home);
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    let folder = home.work.join("extra");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(
        folder.join("package.toml"),
        "[package]\nname = { en = \"Extra\" }\n",
    )
    .expect("写得进");
    let installed = client
        .call("i", "package.install", json!({"path": folder}))
        .await;
    assert_eq!(installed["result"]["package"], "extra", "{installed}");
    let item = |listed: &Value, id: &str| -> Value {
        listed["result"]["packages"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["package"] == id))
            .cloned()
            .unwrap_or_else(|| panic!("有 {id}：{listed}"))
    };
    let listed = client.call("l", "package.list", json!({})).await;
    let at = item(&listed, "extra")["installed_at"].clone();
    assert!(
        at.as_str()
            .and_then(|text| miyu_kernel::time::Timestamp::parse(text).ok())
            .is_some(),
        "装上的带时刻：{listed}"
    );
    assert!(
        item(&listed, "miyu-base-shell")
            .get("installed_at")
            .is_none(),
        "出厂的不写"
    );
}
