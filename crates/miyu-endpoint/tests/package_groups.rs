//! 软件包列表里的组（施工 BS-1 下，设计 `44-基础系统拆包.md` 第三节第 2 条）：每一项写它归哪几个组，回应顶上列出组表里的组，
//! 名字照连接的语言；出厂清单写的组都在组表里，没有哪个包的编号和组名撞。

use serde_json::{Value, json};

use crate::support::login::member;
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
}

#[tokio::test]
async fn the_list_says_which_group_each_package_is_in() {
    let home = Home::new();
    let core = packaged::core(&home);
    let mut client = Client::connect(std::sync::Arc::clone(&core));
    client.hello().await;
    let listed = client.call("l", "package.list", json!({})).await;
    let result = &listed["result"];
    assert_eq!(
        result["groups"],
        json!([{"id": "basesystem", "name": "基础系统", "summary": "读写文件、跑命令、查以前说过的话"}]),
        "名字照连接的语言：{listed}"
    );
    let groups_of = |id: &str| -> Value {
        result["packages"]
            .as_array()
            .and_then(|items| items.iter().find(|item| item["package"] == id))
            .map(|item| item["groups"].clone())
            .unwrap_or_else(|| panic!("有 {id}：{listed}"))
    };
    assert_eq!(groups_of("shell"), json!(["basesystem"]));
    assert_eq!(groups_of("memory"), Value::Null, "不在组里的不写");

    let mut bob = member(&core, "bob").await;
    let theirs = bob.call("l", "package.list", json!({})).await;
    assert_eq!(
        theirs["result"]["groups"], result["groups"],
        "成员也看得到组"
    );
    let shell = theirs["result"]["packages"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["package"] == "shell"))
        .cloned()
        .unwrap_or_default();
    assert_eq!(shell["groups"], json!(["basesystem"]), "{theirs}");
}
