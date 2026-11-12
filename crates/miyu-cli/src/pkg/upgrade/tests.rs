//! 升级的确认屏（施工 PK-3 下，设计 `61-装包说清来源和能力.md` 第三节「升级」、定了的 5）：每个包下面列新多要的能力（批过的不
//! 印），一起装的新依赖列它要的；社区的照样有「注意」；有新要的问「并批准新要的能力」，没有的照旧。

use serde_json::{Value, json};

use super::{question, rows};
use crate::language::Language;

/// 盘点：app 1.0 → 2.0，old 1.0 → 1.1。
fn survey() -> Vec<Value> {
    vec![
        json!({"package": "app", "from": "1.0", "to": "2.0"}),
        json!({"package": "old", "from": "1.0", "to": "1.1"}),
    ]
}

/// 看一眼：app 2.0 要联网（新的）、提供工具（批过的）；old 照索引看不成；一起装社区的 lib，要搜索。
fn preview() -> Value {
    json!({
        "upgrades": [
            {"package": "app", "version": "2.0", "source": "official", "feed": "official",
             "capabilities": [{"id": "network", "name": "联网", "summary": "访问网络、开端口"},
                              {"id": "tools", "name": "提供工具", "summary": "给 AI 加能调用的工具", "held": true}]},
            {"package": "old", "version": "1.1"},
        ],
        "with": [{"package": "lib", "version": "1.0", "source": "community", "feed": "official",
                  "capabilities": [{"id": "search", "name": "当搜索服务供应商", "summary": "在搜索服务供应商里列出来"},
                                   {"id": "network", "name": "联网", "summary": "访问网络、开端口"}]}],
        "download": 2048,
        "notice": ["lib 是社区包，没有经过官方审核，可能存在风险。"],
    })
}

#[test]
fn an_upgrade_lists_only_the_new_capabilities_under_each_package() {
    assert_eq!(
        rows(&survey(), &preview(), &Language::Chinese),
        [
            "将升级 app 1.0 → 2.0",
            "  新要：联网：访问网络、开端口",
            "将升级 old 1.0 → 1.1",
            "将安装 lib 1.0",
            "  需要：当搜索服务供应商：在搜索服务供应商里列出来",
            "        联网：访问网络、开端口",
            "下载：2.0 KiB",
        ]
    );
    assert_eq!(
        rows(&survey(), &preview(), &Language::English)[..2],
        [
            "Will upgrade app 1.0 → 2.0",
            "  New: 联网: 访问网络、开端口"
        ]
    );
    assert_eq!(
        question(&survey(), &preview(), &Language::Chinese),
        "注意：lib 是社区包，没有经过官方审核，可能存在风险。\n升级 2 个软件包并批准新要的能力？[Y/n] "
    );
    assert_eq!(
        question(&survey()[..1], &preview(), &Language::English),
        "Note: lib 是社区包，没有经过官方审核，可能存在风险。\nUpgrade 'app' and approve the new capabilities? [Y/n] "
    );
}

#[test]
fn an_upgrade_with_nothing_new_asks_as_before() {
    let mut held = preview();
    held["with"] = json!([]);
    held["notice"] = Value::Null;
    held["upgrades"][0]["capabilities"][0]["held"] = json!(true);
    assert_eq!(
        rows(&survey()[..1], &held, &Language::Chinese),
        ["将升级 app 1.0 → 2.0", "下载：2.0 KiB"],
        "批过的不印"
    );
    assert_eq!(
        question(&survey()[..1], &held, &Language::Chinese),
        "升级 'app'？[Y/n] ",
        "官方的没有「注意」，没有新要的不问批准"
    );
    // 旧的核心：看一眼只有下多少。
    let older =
        json!({"upgrades": [{"package": "app", "from": "1.0", "to": "2.0"}], "download": 9});
    assert_eq!(
        rows(&survey()[..1], &older, &Language::Chinese),
        ["将升级 app 1.0 → 2.0", "下载：9 B"]
    );
    assert_eq!(
        question(&survey(), &older, &Language::Chinese),
        "升级 2 个软件包？[Y/n] "
    );
}
