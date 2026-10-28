//! 人点名用技能（施工 PT-2 下，`docs/blueprint/skills.md`「人点名用」）：真核心走一遍。`/技能名 参数` 读那份 `SKILL.md`，去掉
//! 开头，套 `<skill name=… path=…>` 外壳、跟一行 `ARGUMENTS: …` 当人说的一句发出去；时间线上是人敲的那一行。没写参数的没有那一
//! 行。`user-invocable: false` 的认不出、不进命令菜单；只许人点名用的（`disable-model-invocation`）点名用得了、不列进 system。
//! 和模板命令撞名的照模板命令。`command.catalog` 里技能那一类带 `"skill": true`。

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event, MessageUser, TypedCommand};
use miyu_session::testkit::{Play, Script};

use crate::prompt_override::latest_system;
use crate::support::deleting::base_tools;
use crate::support::*;

async fn run(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    client
        .call(id, "command.run", json!({"session": session, "text": text}))
        .await
}

/// 日志里人说的每一句：字、带的命令。
fn said(log: &[Event]) -> Vec<(String, Option<TypedCommand>)> {
    log.iter()
        .filter_map(|event| match &event.body {
            Body::MessageUser(MessageUser {
                blocks, command, ..
            }) => Some((
                blocks
                    .iter()
                    .filter_map(|block| match block {
                        Block::Text(text) => Some(text.text.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>()
                    .join(""),
                command.clone(),
            )),
            _ => None,
        })
        .collect()
}

/// 一份 `SKILL.md`：开头另加 `extra` 那几行。
fn skill(name: &str, extra: &str) -> String {
    format!(
        "---\nname: {name}\ndescription: Do {name}.\n{extra}metadata:\n  summary_zh: 做 {name}\n---\n\n# {name}\n\nStep one.\n"
    )
}

/// 家目录里的四个技能：平常的 `notes`、只许人点名的 `release`、不许人点名的 `style`、和模板命令撞名的 `review`。
fn written(home: &Home) {
    home.write("skills/notes/SKILL.md", &skill("notes", ""));
    home.write(
        "skills/release/SKILL.md",
        &skill("release", "disable-model-invocation: true\n"),
    );
    home.write(
        "skills/style/SKILL.md",
        &skill("style", "user-invocable: false\n"),
    );
    home.write("skills/review/SKILL.md", &skill("review", ""));
    home.write("commands/review.md", "The template review.\n");
}

#[tokio::test]
async fn a_named_skill_is_said_with_its_body_in_a_wrapper() {
    let home = Home::new();
    written(&home);
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = run(&mut client, "k1", &session, "  /notes the meeting  ").await;
    assert_eq!(reply["result"]["command"], "notes", "{reply}");
    assert_eq!(reply["result"]["said"], "", "{reply}");
    home.until_turns(&session, 1).await;
    let file = home.root.path().join("skills/notes/SKILL.md");
    let path = miyu_session::path_text(&file);
    let wrapped = format!(
        "<skill name=\"notes\" path=\"{path}\">\n# notes\n\nStep one.\n</skill>\nARGUMENTS: the meeting"
    );
    let typed = |text: &str| {
        Some(TypedCommand {
            name: "notes".to_string(),
            text: text.to_string(),
        })
    };
    assert_eq!(
        said(&home.log(&session)),
        [(wrapped, typed("/notes the meeting"))]
    );
    let request = format!("{:?}", script.requests()[0].1.messages);
    assert!(
        request.contains("Step one.") && !request.contains("description: Do notes"),
        "{request}"
    );

    run(&mut client, "k2", &session, "/notes").await;
    home.until_turns(&session, 2).await;
    let (text, _) = said(&home.log(&session)).pop().expect("有第二句");
    assert!(text.ends_with("</skill>"), "没写参数的没有那一行：{text}");
}

#[tokio::test]
async fn who_may_name_a_skill_and_which_wins_a_clash() {
    let home = Home::new();
    written(&home);
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    // 带 `read` 的工具面：system 里才有技能那一块。
    let mut client = Client::connect(home.core_with_tools(&script, base_tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", "~").await;

    let style = run(&mut client, "k1", &session, "/style").await;
    assert_eq!(reason(&style), Some("unknown_command"), "{style}");

    run(&mut client, "k2", &session, "/release v2").await;
    home.until_turns(&session, 1).await;
    let (text, _) = said(&home.log(&session)).pop().expect("有一句");
    assert!(text.starts_with("<skill name=\"release\""), "{text}");
    let system = latest_system(&home, &session);
    assert!(
        system.contains("- notes: ") && !system.contains("- release: "),
        "{system}"
    );

    run(&mut client, "k3", &session, "/review").await;
    home.until_turns(&session, 2).await;
    let (text, command) = said(&home.log(&session)).pop().expect("有一句");
    assert_eq!(text, "The template review.", "撞名的照模板命令");
    assert_eq!(command.map(|typed| typed.name), Some("review".to_string()));

    let catalog = client
        .call("c", "command.catalog", json!({"session": session}))
        .await;
    let commands = catalog["result"]["commands"].as_array().expect("有命令");
    let item = |name: &str| {
        commands
            .iter()
            .filter(|one| one["name"] == name)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        item("notes"),
        [&json!({"name": "notes", "aliases": [], "summary": "做 notes", "skill": true})],
        "给人看的那一句照连接的语言挑，没写英文的照中文（同 `skill.list`）"
    );
    assert_eq!(item("release").len(), 1);
    assert!(item("style").is_empty(), "不许人点名的不列");
    let review = item("review");
    assert_eq!(review.len(), 1, "{review:?}");
    assert!(review[0].get("skill").is_none(), "撞名的列命令那一项");
}

/// `miyu check`（施工 PT-2 下）：能点名用的技能和命令同名的报 `skill_command_clash`（内置的 `/stop`、模板命令 `/review`），说敲
/// `/名字` 用的是命令；不许人点名的同名（`lint`）不报：本来就不点名。
#[tokio::test]
async fn check_says_which_skills_lose_to_a_command() {
    let home = Home::new();
    written(&home);
    home.write("skills/stop/SKILL.md", &skill("stop", ""));
    home.write("commands/lint.md", "Lint it.\n");
    home.write(
        "skills/lint/SKILL.md",
        &skill("lint", "user-invocable: false\n"),
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client.call("k1", "check", json!({})).await;
    let mut clashes: Vec<(String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|one| one["code"] == "skill_command_clash")
        .map(|one| {
            let text = |key: &str| one[key].as_str().unwrap_or_default().to_string();
            (text("file"), text("message"))
        })
        .collect();
    clashes.sort();
    assert_eq!(clashes.len(), 2, "{reply}");
    assert!(
        clashes[0].0.ends_with("skills/review/SKILL.md"),
        "{clashes:?}"
    );
    assert!(clashes[0].1.contains("/review"), "{clashes:?}");
    assert!(
        clashes[1].0.ends_with("skills/stop/SKILL.md"),
        "{clashes:?}"
    );
}

/// 会话的预设关着这个技能（功能 `skill-<名字>`）：点名用拒 `command_not_allowed`、说清哪个预设，什么都不发；菜单里不列。
#[tokio::test]
async fn a_preset_that_turns_a_skill_off_refuses_it() {
    let home = Home::new();
    written(&home);
    home.write(
        "presets/quiet.toml",
        "[preset]\nname = \"安静\"\n\n[features]\nskill-notes = false\n",
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "preset": "quiet"}))
        .await;
    let reply = run(&mut client, "k1", &session, "/notes x").await;
    assert_eq!(reason(&reply), Some("command_not_allowed"), "{reply}");
    assert_eq!(reply["error"]["data"]["why"], "预设「安静」未启用 /notes。");
    assert!(said(&home.log(&session)).is_empty());
    let listed = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    assert!(!listed.to_string().contains("\"notes\""), "{listed}");
    assert!(listed.to_string().contains("\"release\""), "{listed}");
}
