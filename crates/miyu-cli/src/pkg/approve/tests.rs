//! 装、升级的时候就批能力（施工 PK-3 下）：认不认 `approve` 的核心、照看一眼拼表、批了哪些、能力一项一行、「注意」。

use serde_json::{Value, json};

use super::{
    asks, carried, installing, needs, notice, prefixed, said, table, understood, upgrading,
};
use crate::language::Language;

/// 从软件源装 app（社区，要联网、提供工具，联网批过），一起装 lib（要搜索），推荐 rec（要联网）。
fn preview() -> Value {
    json!({
        "package": "app", "version": "1.0", "source": "community", "feed": "official",
        "capabilities": [
            {"id": "network", "name": "联网", "summary": "访问网络、开端口", "held": true},
            {"id": "tools", "name": "提供工具", "summary": "给 AI 加能调用的工具"},
        ],
        "with": [{"package": "lib", "version": "1.0", "source": "official", "feed": "official",
                  "capabilities": [{"id": "search", "name": "提供联网服务"}]},
                 {"package": "bare", "version": "1.0", "source": "official", "feed": "official"}],
        "recommends": [{"package": "rec", "version": "1.0",
                        "capabilities": [{"id": "network", "name": "联网"}]}],
        "notice": ["app 是社区包，没有经过官方审核，可能存在风险。", "kit 来自本机文件，没有经过官方审核，可能存在风险。"],
    })
}

#[test]
fn only_a_core_that_says_where_a_package_comes_from_gets_approve() {
    assert!(understood(&preview()), "从软件源装的写了 feed");
    assert!(understood(&json!({"package": "x", "source": "file"})));
    assert!(
        !understood(&json!({"package": "x", "source": "official", "download": 1})),
        "PK-3 上以前从软件源装的：source 是源的名字、没有 feed"
    );
    assert!(
        !understood(&json!({"package": "x", "capabilities": [{"id": "network"}]})),
        "PK-3 上以前从包文件夹装的：有能力、没有 source"
    );
    assert!(!understood(
        &json!({"package": "net", "source": "shipped", "restores": true})
    ));
}

#[test]
fn the_table_carries_every_listed_capability_of_what_is_installed() {
    let shown = preview();
    let items = installing(&shown);
    let ids: Vec<&str> = items.iter().filter_map(|i| i["package"].as_str()).collect();
    assert_eq!(ids, ["lib", "bare", "app"], "依赖在前，要装的最后");
    let approve = table(&items);
    assert_eq!(
        Value::Object(approve.clone()),
        json!({"lib": ["search"], "app": ["network", "tools"]}),
        "批过的也照原样带；没列能力的、推荐的不写"
    );
    assert_eq!(
        carried(json!({"source": "app"}), &approve),
        json!({"source": "app", "approve": {"lib": ["search"], "app": ["network", "tools"]}})
    );
    assert_eq!(
        carried(json!({"source": "x"}), &table(&[])),
        json!({"source": "x"}),
        "空的不带"
    );
    let upgrade = json!({"upgrades": [{"package": "app", "capabilities": [{"id": "tools"}]},
                                      {"package": "old", "version": "2.0"}],
                         "with": [{"package": "lib", "capabilities": [{"id": "search"}]}]});
    assert_eq!(
        Value::Object(table(&upgrading(&upgrade))),
        json!({"app": ["tools"], "lib": ["search"]})
    );
}

#[test]
fn only_what_is_not_held_asks_and_is_said_as_approved() {
    let shown = preview();
    let items = installing(&shown);
    assert!(asks(&items));
    let mut held = preview();
    held["with"] = json!([]);
    held["capabilities"][1]["held"] = json!(true);
    assert!(!asks(&installing(&held)), "全批过的不问批准");
    assert_eq!(
        said(&items, &Language::Chinese),
        [
            "已批准 lib 的能力：提供联网服务",
            "已批准 app 的能力：提供工具"
        ]
    );
    assert_eq!(
        said(&items[2..], &Language::English),
        ["approved capabilities for app: 提供工具"]
    );
    assert!(said(&installing(&held), &Language::Chinese).is_empty());
}

#[test]
fn each_capability_is_a_line_with_its_summary_and_held_ones_are_marked() {
    let shown = preview();
    assert_eq!(
        needs(&shown, false, &Language::Chinese),
        [
            "联网：访问网络、开端口（已批准）",
            "提供工具：给 AI 加能调用的工具"
        ]
    );
    assert_eq!(
        needs(&shown, true, &Language::English),
        ["提供工具: 给 AI 加能调用的工具"],
        "升级只列新要的"
    );
    assert_eq!(prefixed("lib", Some(5), "x"), "lib    x");
    assert_eq!(prefixed("lib", None, "x"), "x");
}

#[test]
fn the_notice_is_the_core_words_as_they_are() {
    let shown = preview();
    assert_eq!(
        notice(&shown["notice"], &Language::Chinese).as_deref(),
        Some(
            "注意：app 是社区包，没有经过官方审核，可能存在风险。kit 来自本机文件，没有经过官方审核，可能存在风险。"
        )
    );
    let english = json!(["This is a community package.", "Outside."]);
    assert_eq!(
        notice(&english, &Language::English).as_deref(),
        Some("Note: This is a community package. Outside.")
    );
    // 整个写成一个字符串的是以前的写法，不认（施工 AUD-3）：命令行和核心一起发，核心给的一直是数组。
    assert_eq!(notice(&json!("一句。"), &Language::Chinese), None);
    assert_eq!(notice(&Value::Null, &Language::Chinese), None);
}
