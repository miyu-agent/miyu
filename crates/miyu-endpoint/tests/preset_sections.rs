//! `preset.get` 的 `sections`（施工 FG-1，设计 `52-功能分组.md` 第一节到第三节，`docs/blueprint/presets.md`「功能分组」）：出厂的
//! 四个预设，组的先后、组里的功能和名字、`by`；带几件工具的列工具、一件的不列；出厂预设开的功能和改前一样。装的扩展、重名、
//! 语言在 `preset_sections_more.rs`。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::tool::Access;
use miyu_session::testkit::Script;
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::venues::configured_core;
use crate::support::*;

/// 一组：编号、中文名、组里的功能（编号、中文名）。
type Shipped = (
    &'static str,
    &'static str,
    &'static [(&'static str, &'static str)],
);

/// 出厂的组和组里的功能（设计 52 第一节的表），照先后。
const SHIPPED: [Shipped; 5] = [
    (
        "basesystem",
        "基础系统",
        &[
            ("read", "读文件"),
            ("write", "新建文件"),
            ("edit", "编辑文件"),
            ("glob", "找文件"),
            ("grep", "搜内容"),
            ("trash", "删到回收站"),
            ("shell", "运行命令"),
            ("jobs", "后台运行"),
            ("clicall", "命令行调用工具"),
            ("subagent", "子代理"),
            ("ask_user", "提问"),
            ("show", "展示给用户"),
            ("sessions", "跨会话消息"),
            ("history", "翻查本会话"),
            ("session_usage", "用量查询"),
            ("packages", "软件包"),
        ],
    ),
    (
        "tasks",
        "任务和定时",
        &[
            ("todowrite", "待办清单"),
            ("goal", "长期目标"),
            ("schedule", "定时任务"),
            ("loop", "循环任务"),
        ],
    ),
    (
        "environment",
        "注入环境信息",
        &[
            ("env-time", "时间"),
            ("env-workdir", "工作目录"),
            ("env-permission", "权限级别"),
            ("env-session", "会话编号"),
            ("env-model", "模型信息"),
            ("env-harness", "Harness 版本"),
            ("env-machine", "本机系统"),
            ("env-profile", "用户身份"),
            ("project-docs", "自动注入 AGENTS 文档"),
        ],
    ),
    (
        "persona",
        "人格",
        &[("memory", "人格记忆"), ("roleplay", "人设遵循提醒")],
    ),
    (
        "research",
        "查资料",
        &[
            ("web-search", "网络搜索"),
            ("web-fetch", "抓取网页"),
            ("knowledge", "知识库"),
        ],
    ),
];

/// 出厂包带的两个技能（authoring），照列出来的先后。
const SKILLS: [(&str, &str); 2] = [
    ("skill-package-creator", "写脚本和包"),
    ("skill-skill-creator", "写技能"),
];

/// 工具目录：带几件工具的功能和几个一件工具的，照包登记。
fn catalog() -> Catalog {
    let fake = |name: &str| -> Arc<dyn Tool> { Fake::new(name, Access::Read, Act::Echo) };
    let all = |names: &[&str]| names.iter().map(|name| fake(name)).collect::<Vec<_>>();
    Catalog::in_packages([
        ("miyu-base-read", all(&["read"])),
        ("miyu-base-write", all(&["write"])),
        ("miyu-base-sessions", all(&["sessions", "send_message"])),
        ("miyu-base-packages", all(&["packages", "package"])),
        ("miyu-base-todowrite", all(&["todowrite"])),
        (
            "miyu-base-schedule",
            all(&["cron_create", "cron_delete", "cron_list"]),
        ),
        ("miyu-base-loop", all(&["schedule_wakeup"])),
        (
            "miyu-base-memory",
            all(&["remember", "forget", "memory_search"]),
        ),
        ("miyu-base-net", all(&["web_search", "web_fetch"])),
        (
            "miyu-base-knowledge",
            all(&["kb_search", "kb_read", "kb_save", "kb_edit", "kb_delete"]),
        ),
    ])
    .expect("合写法")
}

/// 握手用 `locale` 的连接。
pub(crate) async fn connected(home: &Home, locale: &str) -> Client {
    let mut client = Client::connect(configured_core(home, &Script::new([]), catalog()));
    let hello = json!({
        "protocol": [1, 1],
        "head": {"kind": "test", "version": "0.0.0"},
        "locale": locale,
        "caps": {"input": true},
        "token": TOKEN,
    });
    client.call("hello-1", "hello", hello).await;
    client
}

/// `preset.get` 的结果。
pub(crate) async fn get(client: &mut Client, preset: &str) -> Value {
    let reply = client
        .call(
            &format!("g-{preset}"),
            "preset.get",
            json!({"preset": preset}),
        )
        .await;
    reply["result"].clone()
}

/// 一组的样子：`(kind, id, name, on, total)`。
pub(crate) fn heads(result: &Value) -> Vec<(String, String, String, u64, u64)> {
    sections(result)
        .iter()
        .map(|one| {
            let text = |key: &str| one[key].as_str().unwrap_or_default().to_string();
            let count = |key: &str| one[key].as_u64().unwrap_or(u64::MAX);
            (
                text("kind"),
                text("id"),
                text("name"),
                count("on"),
                count("total"),
            )
        })
        .collect()
}

pub(crate) fn sections(result: &Value) -> &Vec<Value> {
    result["sections"]
        .as_array()
        .unwrap_or_else(|| panic!("有 sections：{result}"))
}

/// 编号是 `id` 的那一组。
pub(crate) fn section<'a>(result: &'a Value, id: &str) -> &'a Value {
    sections(result)
        .iter()
        .find(|one| one["id"] == id)
        .unwrap_or_else(|| panic!("没有组 {id}：{result}"))
}

/// 一组里每一项的 `(id, name, on, by)`。
pub(crate) fn rows(section: &Value) -> Vec<(String, String, bool, String)> {
    section["items"]
        .as_array()
        .expect("一项项")
        .iter()
        .map(|item| {
            assert_eq!(item["kind"], "feature", "{item}");
            let text = |key: &str| item[key].as_str().unwrap_or_default().to_string();
            (
                text("id"),
                text("name"),
                item["on"].as_bool().expect("有 on"),
                text("by"),
            )
        })
        .collect()
}

/// 编号是 `id` 的那一项。
pub(crate) fn item<'a>(result: &'a Value, id: &str) -> &'a Value {
    sections(result)
        .iter()
        .flat_map(|one| one["items"].as_array().expect("一项项"))
        .find(|one| one["id"] == id)
        .unwrap_or_else(|| panic!("没有 {id}：{result}"))
}

/// 照出厂的表、技能，`decide` 定每个功能开不开、谁定的，比 `result` 的每一组；交回开着的功能编号，照先后。
fn matches_shipped(
    result: &Value,
    decide: impl Fn(&str, &str) -> (bool, &'static str),
) -> Vec<String> {
    let mut wanted_heads = Vec::new();
    let mut opened = Vec::new();
    let mut check = |kind: &str, id: &str, name: &str, features: &[(&str, &str)]| {
        let wanted: Vec<(String, String, bool, String)> = features
            .iter()
            .map(|(feature, label)| {
                let (on, by) = decide(id, feature);
                (feature.to_string(), label.to_string(), on, by.to_string())
            })
            .collect();
        assert_eq!(rows(section(result, id)), wanted, "组 {id}");
        let on = wanted.iter().filter(|row| row.2).count() as u64;
        opened.extend(wanted.iter().filter(|row| row.2).map(|row| row.0.clone()));
        wanted_heads.push((kind.into(), id.into(), name.into(), on, wanted.len() as u64));
    };
    for (id, name, features) in SHIPPED {
        check("group", id, name, features);
    }
    check("skills", "skills", "技能", &SKILLS);
    check("scripts", "scripts", "脚本", &[]);
    check("mcp", "mcp", "MCP 服务", &[]);
    assert_eq!(
        heads(result),
        wanted_heads,
        "组的先后：出厂组、技能、脚本、MCP 服务"
    );
    opened
}

/// 出厂的四个预设：组的先后、组里的功能和中文名、开不开、`by`；开着的功能和改前一样（施工 FG-1 以前照组 basesystem 开的待办、
/// 定时、循环、项目文档，常规开发照旧开着）。
#[tokio::test]
async fn shipped_presets_are_sectioned_like_the_table() {
    let home = Home::new();
    let mut client = connected(&home, "zh-CN").await;

    let full = get(&mut client, "full").await;
    let opened = matches_shipped(&full, |_, _| (true, "default"));
    assert_eq!(opened.len(), 34 + 2, "全部功能：都开着");

    let dev = get(&mut client, "dev").await;
    let opened = matches_shipped(&dev, |group, feature| match (group, feature) {
        (_, "roleplay" | "knowledge") => (false, "default"),
        ("basesystem" | "tasks" | "environment" | "persona" | "research" | "skills", _) => {
            (true, "group")
        }
        _ => unreachable!(),
    });
    let mut before: Vec<&str> = SHIPPED
        .iter()
        .flat_map(|(_, _, features)| features.iter().map(|(id, _)| *id))
        .filter(|id| !["roleplay", "knowledge"].contains(id))
        .collect();
    before.extend(SKILLS.map(|(id, _)| id));
    assert_eq!(opened, before, "常规开发开的功能和改前一样");

    let minimal = get(&mut client, "minimal").await;
    let opened = matches_shipped(&minimal, |_, feature| match feature {
        "read" | "write" | "edit" | "shell" => (true, "group"),
        _ if feature.starts_with("env-") => (true, "preset"),
        _ => (false, "default"),
    });
    assert_eq!(
        opened,
        [
            "read",
            "write",
            "edit",
            "shell",
            "env-time",
            "env-workdir",
            "env-permission",
            "env-session",
            "env-model",
            "env-harness",
            "env-machine",
            "env-profile"
        ],
        "极简开发开的功能和改前一样"
    );

    let none = get(&mut client, "none").await;
    let opened = matches_shipped(&none, |_, _| (false, "default"));
    assert!(opened.is_empty(), "无功能：都关着");

    // 原来的格一个不动：`features` 还在，写了包、组的 `by` 照旧是 `preset`。
    let read = dev["features"]
        .as_array()
        .expect("一个个功能")
        .iter()
        .find(|one| one["id"] == "read")
        .expect("有 read");
    assert_eq!(read["by"], "preset", "{read}");
    assert_eq!(
        read["tools"],
        json!([{"name": "read", "label": "读取", "on": true}])
    );
}

/// 带几件工具的功能列出工具（照清单写的先后、显示名照连接的语言、开不开照 `[tools]` 和功能），一件工具的不列。
#[tokio::test]
async fn features_with_several_tools_list_them() {
    let home = Home::new();
    home.write(
        "presets/quiet.toml",
        "[preset]\nname = { en = \"Quiet\" }\n\n[features]\nschedule = false\n\n[tools]\nkb_edit = false\n",
    );
    let mut client = connected(&home, "zh-CN").await;
    let quiet = get(&mut client, "quiet").await;
    let tools = |id: &str| item(&quiet, id)["tools"].clone();
    assert_eq!(
        tools("sessions"),
        json!([
            {"id": "sessions", "name": "列出会话", "on": true},
            {"id": "send_message", "name": "发消息", "on": true},
        ])
    );
    assert_eq!(
        tools("knowledge"),
        json!([
            {"id": "kb_search", "name": "搜索", "on": true},
            {"id": "kb_read", "name": "阅读", "on": true},
            {"id": "kb_save", "name": "新建", "on": true},
            {"id": "kb_edit", "name": "编辑", "on": false},
            {"id": "kb_delete", "name": "删除", "on": true},
        ]),
        "知识库五件照清单的先后，[tools] 关了的关着"
    );
    assert_eq!(
        tools("memory"),
        json!([
            {"id": "remember", "name": "记住", "on": true},
            {"id": "memory_search", "name": "回忆", "on": true},
            {"id": "forget", "name": "忘掉", "on": true},
        ])
    );
    assert_eq!(
        tools("schedule"),
        json!([
            {"id": "cron_create", "name": "新建定时", "on": false},
            {"id": "cron_delete", "name": "删除定时", "on": false},
            {"id": "cron_list", "name": "查看定时", "on": false},
        ]),
        "功能关着的工具都关着"
    );
    assert_eq!(item(&quiet, "schedule")["by"], "preset");
    for one in [
        "read",
        "write",
        "todowrite",
        "loop",
        "web-search",
        "roleplay",
    ] {
        assert!(
            item(&quiet, one).get("tools").is_none(),
            "{one} 一件或没有工具，不列"
        );
    }
}
