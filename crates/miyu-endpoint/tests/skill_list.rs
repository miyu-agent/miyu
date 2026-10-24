//! `skill.list`（施工 X-26，`docs/blueprint/skills.md`「技能」）：三种来处各列一个（管理员自己放的、出厂的包带的、装的包带的），
//! 同名的只列核心实际用的那一份；给人看的名字、说明照连接的语言挑，没写的不带；路径在家目录下的写成 `~/…`。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::{Client, Home};

/// 一份 `SKILL.md`：名字、给模型的说明，`metadata` 里另几行（每行两个空格开头）。
fn skill(name: &str, metadata: &[&str]) -> String {
    let metadata = match metadata.is_empty() {
        true => String::new(),
        false => format!("metadata:\n{}\n", metadata.join("\n")),
    };
    format!("---\nname: {name}\ndescription: Do {name}.\n{metadata}---\n\nBody.\n")
}

/// 写好三种来处的技能：家目录里的 `notes`、盖住出厂同名的 `package-creator`，管理员装的包 `kit` 带的 `kit-notes`。
fn written(home: &Home) {
    home.write("home/alice/skills/notes/SKILL.md", &skill("notes", &[]));
    home.write(
        "home/alice/skills/package-creator/SKILL.md",
        &skill("package-creator", &["  display_name: My packages"]),
    );
    home.write(
        "system/packages/kit/package.toml",
        "[package]\nname = { en = \"kit\" }\n",
    );
    home.write(
        "system/packages/kit/skills/kit-notes/SKILL.md",
        &skill(
            "kit-notes",
            &[
                "  display_name: Kit notes",
                "  display_name_ja: キットメモ",
                "  summary_zh: 只有中文的说明",
            ],
        ),
    );
}

/// 照个人设置的语言 `language` 起一份核心（系统的家目录当是数据根），列一次。
async fn listed(home: &Home, language: &str) -> Vec<Value> {
    home.write(
        "home/alice/settings.toml",
        &format!("[ui]\nlanguage = \"{language}\"\n"),
    );
    let core = home.core_configured(&Script::new([]), Some(home.root.path().to_path_buf()), &[]);
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client.call("s", "skill.list", json!({})).await;
    reply["result"]["skills"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .clone()
}

/// 名字是 `name` 的那一项。
fn named<'a>(skills: &'a [Value], name: &str) -> &'a Value {
    skills
        .iter()
        .find(|one| one["name"] == name)
        .unwrap_or_else(|| panic!("没有 {name}：{skills:?}"))
}

#[tokio::test]
async fn skills_are_listed_once_each_with_where_they_come_from() {
    let home = Home::new();
    written(&home);
    let skills = listed(&home, "ja").await;
    let names: Vec<&str> = skills
        .iter()
        .map(|one| one["name"].as_str().unwrap_or_default())
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "照名字排");
    assert_eq!(
        names
            .iter()
            .filter(|name| **name == "package-creator")
            .count(),
        1,
        "同名的只列一次：{names:?}"
    );

    assert_eq!(
        named(&skills, "notes"),
        &json!({"name": "notes", "from": "home", "path": "~/home/alice/skills/notes/SKILL.md", "on": true}),
        "自己放的：没有包，没写给人看的两格的不带"
    );
    let mine = named(&skills, "package-creator");
    assert_eq!(
        (mine["from"].clone(), mine.get("package")),
        (json!("home"), None),
        "自己放的盖住出厂同名的：{mine}"
    );
    assert_eq!(mine["display_name"], "My packages", "{mine}");

    let shipped = named(&skills, "skill-creator");
    assert_eq!(shipped["from"], "shipped", "{shipped}");
    assert_eq!(shipped["package"], "authoring", "{shipped}");
    assert_eq!(
        shipped["display_name"], "Write a skill",
        "没写日文的照英文：{shipped}"
    );
    assert!(
        shipped["path"].as_str().is_some_and(|path| path
            .replace('\\', "/")
            .ends_with("authoring/skills/skill-creator/SKILL.md")),
        "{shipped}"
    );

    let kit = named(&skills, "kit-notes");
    assert_eq!(
        kit,
        &json!({
            "name": "kit-notes",
            "from": "package",
            "package": "kit",
            "display_name": "キットメモ",
            "summary": "只有中文的说明",
            "path": "~/system/packages/kit/skills/kit-notes/SKILL.md",
            "on": true,
        }),
        "装的包带的：照日文挑，没写日文、英文的说明照中文"
    );
}

#[tokio::test]
async fn names_follow_the_connection_language() {
    let home = Home::new();
    written(&home);
    let english = listed(&home, "en").await;
    let kit = named(&english, "kit-notes");
    assert_eq!(kit["display_name"], "Kit notes", "{kit}");
    assert_eq!(kit["summary"], "只有中文的说明", "{kit}");
    let chinese = listed(&home, "zh").await;
    assert_eq!(
        named(&chinese, "skill-creator")["display_name"],
        "写技能",
        "出厂的照中文"
    );
    assert_eq!(named(&chinese, "kit-notes")["display_name"], "Kit notes");
}
