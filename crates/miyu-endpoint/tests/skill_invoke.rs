//! 人用 `$` 点名技能（施工 SN-1，发版前检查第 26 条；原来 `/技能名` 是施工 PT-2 下）：真核心走一遍。`session.send` 的 `skills`
//! 照先后读那几份 `SKILL.md`，去掉开头，各套一层 `<skill name=… path=…>` 外壳，排在人说的那句前面；时间线上是人敲的原文。
//! `user-invocable: false` 的认不出（`unknown_skill`）、不进 `$` 菜单；只许人点名用的（`disable-model-invocation`）点名用得了、
//! 不列进 system。技能和模板命令同名也不撞：`/review` 是命令、`$review` 是技能。`command.catalog` 另列 `skills`；`/技能名`
//! 认不出了；预设关着的拒、说清哪个预设。

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

/// 说一句 `text`，点名 `skills`。
async fn say(client: &mut Client, id: &str, session: &str, text: &str, skills: &[&str]) -> Value {
    client
        .call(
            id,
            "session.send",
            json!({"session": session, "text": text, "skills": skills}),
        )
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
    home.write("root/skills/notes/SKILL.md", &skill("notes", ""));
    home.write(
        "root/skills/release/SKILL.md",
        &skill("release", "disable-model-invocation: true\n"),
    );
    home.write(
        "root/skills/style/SKILL.md",
        &skill("style", "user-invocable: false\n"),
    );
    home.write("root/skills/review/SKILL.md", &skill("review", ""));
    home.write("root/commands/review.md", "The template review.\n");
}

#[tokio::test]
async fn named_skills_go_before_what_was_said_in_wrappers() {
    let home = Home::new();
    written(&home);
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = say(
        &mut client,
        "k1",
        &session,
        "  $notes $release the meeting  ",
        &["notes", "release"],
    )
    .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    let wrapper = |name: &str| {
        let file = home.root.skills().join(format!("{name}/SKILL.md"));
        let path = miyu_session::path_text(&file);
        format!("<skill name=\"{name}\" path=\"{path}\">\n# {name}\n\nStep one.\n</skill>")
    };
    assert_eq!(
        said(&home.log(&session)),
        [(
            format!(
                "{}{}  $notes $release the meeting  ",
                wrapper("notes"),
                wrapper("release")
            ),
            Some(TypedCommand {
                name: "$notes $release".to_string(),
                text: "$notes $release the meeting".to_string(),
            })
        )],
        "照先后排在人说的前面，时间线照人敲的原文"
    );
    let request = format!("{:?}", script.requests()[0].1.messages);
    assert!(
        request.contains("Step one.") && !request.contains("description: Do notes"),
        "{request}"
    );
}

#[tokio::test]
async fn who_may_be_named_and_commands_no_longer_take_skills() {
    let home = Home::new();
    written(&home);
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    // 带 `read` 的工具面：system 里才有技能那一块。
    let mut client = Client::connect(home.core_with_tools(&script, base_tools(), TOKEN));
    client.hello().await;
    let session = client.create("c1", "~").await;

    let style = say(&mut client, "k1", &session, "$style", &["style"]).await;
    assert_eq!(reason(&style), Some("unknown_skill"), "{style}");
    let twice = say(&mut client, "k2", &session, "x", &["notes", "notes"]).await;
    assert_eq!(reason(&twice), Some("bad_params"), "{twice}");
    let shell = client
        .call(
            "k3",
            "session.send",
            json!({"session": session, "text": "ls", "shell": true, "skills": ["notes"]}),
        )
        .await;
    assert_eq!(reason(&shell), Some("bad_params"), "{shell}");
    let old = run(&mut client, "k4", &session, "/notes").await;
    assert_eq!(
        reason(&old),
        Some("unknown_command"),
        "/技能名 不再认：{old}"
    );

    say(&mut client, "k5", &session, "$release v2", &["release"]).await;
    home.until_turns(&session, 1).await;
    let (text, _) = said(&home.log(&session)).pop().expect("有一句");
    assert!(text.starts_with("<skill name=\"release\""), "{text}");
    let system = latest_system(&home, &session);
    assert!(
        system.contains("- notes: ") && !system.contains("- release: "),
        "{system}"
    );

    run(&mut client, "k6", &session, "/review").await;
    home.until_turns(&session, 2).await;
    let (text, command) = said(&home.log(&session)).pop().expect("有一句");
    assert_eq!(text, "The template review.", "/review 是模板命令");
    assert_eq!(command.map(|typed| typed.name), Some("review".to_string()));

    let catalog = client
        .call("c", "command.catalog", json!({"session": session}))
        .await;
    let names = |key: &str| -> Vec<String> {
        catalog["result"][key]
            .as_array()
            .unwrap_or_else(|| panic!("{catalog}"))
            .iter()
            .filter_map(|one| one["name"].as_str().map(str::to_string))
            .collect()
    };
    let skills = names("skills");
    for name in ["notes", "release", "review"] {
        assert!(skills.contains(&name.to_string()), "{name}：{skills:?}");
    }
    assert!(!skills.contains(&"style".to_string()), "不许人点名的不列");
    assert!(
        !names("commands").contains(&"notes".to_string()),
        "技能不在命令里"
    );
    assert!(
        names("commands").contains(&"review".to_string()),
        "同名的模板命令照列"
    );
    let notes = catalog["result"]["skills"]
        .as_array()
        .and_then(|all| all.iter().find(|one| one["name"] == "notes"))
        .expect("有 notes");
    assert_eq!(
        notes,
        &json!({"name": "notes", "summary": "做 notes"}),
        "给人看的那一句照连接的语言挑"
    );
}

/// `miyu check`：技能和命令同名不再报（施工 SN-1：`$` 和 `/` 是两个名字空间）。
#[tokio::test]
async fn check_no_longer_says_a_skill_loses_to_a_command() {
    let home = Home::new();
    written(&home);
    home.write("root/skills/stop/SKILL.md", &skill("stop", ""));
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client.call("k1", "check", json!({})).await;
    assert!(
        !reply.to_string().contains("skill_command_clash"),
        "{reply}"
    );
}

/// 会话的预设关着这个技能（功能 `skill-<名字>`）：点名用拒 `command_not_allowed`、说清哪个预设，什么都不发；`$` 菜单里不列。
#[tokio::test]
async fn a_preset_that_turns_a_skill_off_refuses_it() {
    let home = Home::new();
    written(&home);
    home.write(
        "root/presets/quiet.toml",
        "[preset]\nname = \"安静\"\n\n[features]\nskill-notes = false\n",
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "preset": "quiet"}))
        .await;
    let reply = say(&mut client, "k1", &session, "$notes x", &["notes"]).await;
    assert_eq!(reason(&reply), Some("command_not_allowed"), "{reply}");
    assert_eq!(
        reply["error"]["data"]["why"],
        "预设「安静」未启用技能 $notes。"
    );
    assert!(said(&home.log(&session)).is_empty());
    let listed = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    let skills = listed["result"]["skills"].to_string();
    assert!(
        !skills.contains("\"notes\"") && skills.contains("\"release\""),
        "{listed}"
    );
}
