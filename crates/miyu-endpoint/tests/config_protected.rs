//! 设置页「受保护的命令」（施工 HZ-1，设计 `11-权限与沙盒.md` 的 A18，`docs/blueprint/config.md`）：`config.schema` 里五个开关和
//! 那一组的名字、说明照连接的语言，组带 `description`；没写说明的组不多出这一格。`config.set` 写得进，`config.check`
//! （`miyu check` 同一条路）不报。

use serde_json::{Value, json};

use miyu_session::settings::ConfirmSettings;
use miyu_session::testkit::Script;

use crate::support::*;

/// 五个开关的键，照设置页上的先后。
const KEYS: [&str; 5] = [
    "permission.confirm.git",
    "permission.confirm.delete",
    "permission.confirm.power",
    "permission.confirm.pipe_to_shell",
    "permission.confirm.container",
];

/// 握手，头报的系统语言是 `locale`。
async fn hello(client: &mut Client, locale: &str) {
    let params = json!({
        "protocol": [1, 1],
        "head": {"kind": "test", "version": "0.0.0"},
        "token": TOKEN,
        "locale": locale,
    });
    let reply = client.call("hello-1", "hello", params).await;
    assert!(reply.get("error").is_none(), "{reply}");
}

/// 清单里有这五项和私密目录那三项的核心。
fn core(home: &Home) -> std::sync::Arc<miyu_endpoint::Core> {
    let items = [ConfirmSettings::ITEMS, miyu_tool::private::ITEMS].concat();
    home.core_with_items(&Script::new([]), None, &[], &items)
}

#[tokio::test]
async fn the_switches_and_their_group_are_named_in_the_language_of_the_connection() {
    let expected = [
        (
            "zh-CN",
            "受保护的命令",
            "下面几类命令在完全放开时也会先问你，关掉就不再问。另有几类永远拒绝、不能关：删根目录、家目录、系统目录、Miyu 的数据，格式化磁盘、往裸设备写，容器拿走整个宿主。",
            [
                "破坏性 git 操作",
                "递归删除",
                "关机和重启",
                "下载后直接执行",
                "容器使用宿主设备",
            ],
        ),
        (
            "en",
            "Protected commands",
            "These kinds of commands ask you first even with full access. Turn one off to stop asking. Some are always refused and can't be turned off: deleting the root, home or system directories or Miyu's data, formatting a disk or writing to a raw device, and giving a container the whole host.",
            [
                "Destructive git operations",
                "Recursive deletes",
                "Shutdown and restart",
                "Running a download directly",
                "Containers using host devices",
            ],
        ),
        (
            "ja",
            "保護されたコマンド",
            "次の種類のコマンドは、フルアクセスでも実行前に確認します。オフにすると確認しません。常に拒否され、オフにできないものもあります：ルート、ホーム、システムディレクトリ、Miyu のデータの削除、ディスクのフォーマット、raw デバイスへの書き込み、コンテナへのホスト全体の引き渡し。",
            [
                "破壊的な git 操作",
                "再帰的な削除",
                "シャットダウンと再起動",
                "ダウンロードして直接実行",
                "コンテナによるホストデバイスの使用",
            ],
        ),
    ];
    for (locale, group, description, names) in expected {
        let home = Home::new();
        let mut client = Client::connect(core(&home));
        hello(&mut client, locale).await;
        let reply = client
            .call("c1", "config.schema", json!({"keys": KEYS}))
            .await;
        assert_eq!(
            reply["result"]["groups"],
            json!([{"description": description, "id": "protected", "name": group, "page": "privacy"}]),
            "{locale}：{reply}"
        );
        let items = reply["result"]["items"].as_array().expect("有项");
        let keys: Vec<&str> = items
            .iter()
            .filter_map(|item| item["key"].as_str())
            .collect();
        assert_eq!(keys, KEYS, "{locale}");
        for (item, name) in items.iter().zip(names) {
            assert_eq!(item["name"], name, "{locale}：{item}");
            assert!(
                item["description"]
                    .as_str()
                    .is_some_and(|said| !said.trim().is_empty()),
                "{locale}：{item}"
            );
            assert_eq!(item["type"], "bool", "{item}");
            assert_eq!(item["default"], true, "出厂照旧问：{item}");
            assert_eq!(item["control"], "toggle", "{item}");
            assert_eq!(item["applies"], "next_turn", "{item}");
            assert_eq!(item["layers"], json!(["system"]), "{item}");
            assert_eq!(
                (&item["page"], &item["group"]),
                (&json!("privacy"), &json!("protected"))
            );
        }
    }
}

/// 没写说明的组不多出 `description` 这一格（施工 HZ-1：组的说明是可选的，原来的组一个字节不变）。
#[tokio::test]
async fn a_group_without_a_description_has_no_such_field() {
    let home = Home::new();
    let mut client = Client::connect(core(&home));
    hello(&mut client, "zh-CN").await;
    let reply = client.call("c1", "config.schema", json!({})).await;
    let groups = reply["result"]["groups"].as_array().expect("有组");
    assert!(groups.len() > 2, "{reply}");
    for group in groups {
        let described = group.get("description").is_some();
        assert_eq!(described, group["id"] == "protected", "{group}");
    }
    let private: Vec<&Value> = groups
        .iter()
        .filter(|group| group["id"] == "private")
        .collect();
    assert_eq!(
        private,
        [&json!({"id": "private", "name": "私密目录", "page": "privacy"})],
        "{reply}"
    );
}

/// 关一类：`config.set` 写得进系统配置，读回来是关着的、来处是那份文件；手写的 `config.check` 不报；不是开关的值照类型报。
#[tokio::test]
async fn a_switch_can_be_set_and_checks_clean() {
    let home = Home::new();
    let mut client = Client::connect(core(&home));
    hello(&mut client, "zh-CN").await;
    let set = client
        .call(
            "c1",
            "config.set",
            json!({"layer": "system", "changes": [
                {"key": "permission.confirm.git", "value": false},
                {"key": "permission.confirm.container", "value": false},
            ]}),
        )
        .await;
    assert!(set.get("error").is_none(), "{set}");
    let written = std::fs::read_to_string(home.root.path().join("etc/config.toml")).expect("写了");
    assert!(written.contains("git = false"), "{written}");
    let got = client.call("c2", "config.get", json!({"keys": KEYS})).await;
    let value = |key: &str| got["result"]["items"][key]["value"].clone();
    assert_eq!(value("permission.confirm.git"), false, "{got}");
    assert_eq!(value("permission.confirm.container"), false, "{got}");
    assert_eq!(
        value("permission.confirm.delete"),
        true,
        "没改的照出厂：{got}"
    );
    assert_eq!(got["result"]["problems"], json!([]), "{got}");
    let check = client
        .call(
            "c3",
            "config.check",
            json!({"layer": "system", "text": "[permission.confirm]\ngit = false\ndelete = false\npower = false\npipe_to_shell = false\ncontainer = false\n"}),
        )
        .await;
    assert_eq!(check["result"]["problems"], json!([]), "{check}");
    // 只有系统一层：项目配置里写了的报。
    let project = client
        .call(
            "c4",
            "config.check",
            json!({"layer": "project", "text": "[permission.confirm]\ngit = false\n"}),
        )
        .await;
    assert_eq!(
        project["result"]["problems"].as_array().map(Vec::len),
        Some(1),
        "{project}"
    );
}
