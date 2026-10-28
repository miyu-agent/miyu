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
    home.write("skills/notes/SKILL.md", &skill("notes", &[]));
    home.write(
        "skills/package-creator/SKILL.md",
        &skill("package-creator", &["  display_name: My packages"]),
    );
    home.write(
        "packages/kit/package.toml",
        "[package]\nname = { en = \"kit\" }\n",
    );
    home.write(
        "packages/kit/skills/kit-notes/SKILL.md",
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
    home.write("config.toml", &format!("[ui]\nlanguage = \"{language}\"\n"));
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
        &json!({"name": "notes", "from": "home", "path": "~/skills/notes/SKILL.md", "on": true}),
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
            "path": "~/packages/kit/skills/kit-notes/SKILL.md",
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

/// 照收了、但不合格式的技能（施工 PT-2 上）：`skill.list` 那一项带 `warnings`，码同 `miyu check`，话照连接的语言；没警告的不带。
/// `miyu check` 报同样的码。
#[tokio::test]
async fn slips_in_a_skill_come_with_warnings_in_the_people_language() {
    let home = Home::new();
    home.write(
        "skills/pdf-main/SKILL.md",
        "---\nname: pdf\ndescription: Use this skill when: the user mentions PDFs.\n---\nBody.\n",
    );
    home.write("skills/notes/SKILL.md", &skill("notes", &[]));
    let skills = listed(&home, "zh").await;
    let pdf = named(&skills, "pdf");
    let codes: Vec<&str> = pdf["warnings"]
        .as_array()
        .unwrap_or_else(|| panic!("{pdf}"))
        .iter()
        .filter_map(|warning| warning["code"].as_str())
        .collect();
    assert_eq!(
        codes,
        ["skill_yaml_lenient", "skill_name_mismatch"],
        "{pdf}"
    );
    assert!(
        pdf["warnings"][1]["message"]
            .as_str()
            .is_some_and(|said| said.contains("照写的名字 pdf 用")),
        "照连接的语言：{pdf}"
    );
    assert!(named(&skills, "notes").get("warnings").is_none());

    let core = home.core_configured(&Script::new([]), Some(home.root.path().to_path_buf()), &[]);
    let mut client = Client::connect(core);
    client.hello().await;
    let checked = client.call("c", "check", json!({})).await;
    let found: Vec<&str> = checked["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{checked}"))
        .iter()
        .filter(|problem| problem["kind"] == "skill")
        .filter_map(|problem| problem["code"].as_str())
        .collect();
    assert_eq!(
        found,
        ["skill_yaml_lenient", "skill_name_mismatch"],
        "{checked}"
    );
}

/// 别家共用的技能目录（施工 PT-2 中）：`~/.agents/skills/` 里的也列出来，`from` 是 `agents`、不带包；同名的照 Miyu 自己的。
#[tokio::test]
async fn skills_in_the_shared_agents_folder_say_where_they_come_from() {
    let home = Home::new().with_shared_skills();
    home.write("skills/notes/SKILL.md", &skill("notes", &[]));
    for name in ["notes", "pdf"] {
        let file = home
            .work
            .join("me/.agents/skills")
            .join(name)
            .join("SKILL.md");
        std::fs::create_dir_all(file.parent().expect("有上一级")).expect("建得了");
        std::fs::write(&file, skill(name, &[])).expect("写得进");
    }
    let skills = listed(&home, "en").await;
    let pdf = named(&skills, "pdf");
    assert_eq!(pdf["from"], "agents", "{pdf}");
    assert!(pdf.get("package").is_none(), "{pdf}");
    assert!(
        pdf["path"].as_str().is_some_and(|path| path
            .replace('\\', "/")
            .ends_with("me/.agents/skills/pdf/SKILL.md")),
        "{pdf}"
    );
    assert_eq!(named(&skills, "notes")["from"], "home", "同名的照自己放的");
}
