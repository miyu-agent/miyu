//! 斜杠命令模板（施工 X-14 上，`docs/blueprint/slash.md`）：真核心走一遍。家目录、装了的包里的 `commands/<名字>.md` 照参数展开
//! 成一句发出去：日志里 `blocks` 是展开的那段、`command` 是人敲的那一行，视图、撤销照那一行；不另回执、不记 `command.ran`。
//! 自己放的盖住包里的，内置的赢；预设关了的拒 `command_not_allowed`、说清哪个预设；`command.catalog`、`preset.get` 列出来。

use serde_json::{Value, json};

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event, MessageUser, TypedCommand};
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::venues::bound_core;
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

fn typed(name: &str, text: &str) -> Option<TypedCommand> {
    Some(TypedCommand {
        name: name.to_string(),
        text: text.to_string(),
    })
}

const REVIEW: &str = "---\ndescription: Review a file\nargument-hint: <file>\nmetadata:\n  summary_zh: 审一个文件\n---\nReview $ARGUMENTS carefully.\n";

#[tokio::test]
async fn a_template_is_said_as_one_line_and_shown_as_typed() {
    let home = Home::new();
    home.write("commands/review.md", REVIEW);
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = run(&mut client, "k1", &session, "  /review src/a.rs  ").await;
    assert_eq!(reply["result"]["command"], "review", "{reply}");
    assert_eq!(reply["result"]["said"], "", "时间线上有那一行，没有回执");
    assert_eq!(reply["result"]["events"].as_array().map(Vec::len), Some(1));
    home.until_turns(&session, 1).await;
    let log = home.log(&session);
    assert_eq!(
        said(&log),
        [(
            "Review src/a.rs carefully.".to_string(),
            typed("review", "/review src/a.rs")
        )]
    );
    assert!(
        !log.iter()
            .any(|event| matches!(event.body, Body::CommandRan(_))),
        "不另记 command.ran"
    );
    let request = format!("{:?}", script.requests()[0].1.messages);
    assert!(
        request.contains("Review src/a.rs carefully.") && !request.contains("/review"),
        "模型看到的是展开的那段：{request}"
    );
}

#[tokio::test]
async fn ones_own_covers_a_packages_and_a_builtin_name_stays_builtin() {
    let home = Home::new();
    home.write(
        "packages/kit/package.toml",
        "[package]\nname = { en = \"Kit\" }\n",
    );
    home.write("packages/kit/commands/review.md", "From the kit.\n");
    home.write("packages/kit/commands/lint.md", "Lint $ARGUMENTS[0].\n");
    home.write("commands/review.md", "My own review.\n");
    home.write("commands/stop.md", "Not the builtin.\n");
    let script = Script::new([Play::Says("好。"), Play::Says("嗯。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    run(&mut client, "k1", &session, "/review").await;
    home.until_turns(&session, 1).await;
    run(&mut client, "k2", &session, "/lint \"my file.rs\" b.rs").await;
    home.until_turns(&session, 2).await;
    let stopped = run(&mut client, "k3", &session, "/stop").await;
    assert_eq!(stopped["result"]["command"], "stop");
    assert_ne!(stopped["result"]["said"], "", "内置的照旧回执");
    assert_eq!(
        said(&home.log(&session)),
        [
            ("My own review.".to_string(), typed("review", "/review")),
            (
                "Lint my file.rs.".to_string(),
                typed("lint", "/lint \"my file.rs\" b.rs")
            ),
        ]
    );
}

#[tokio::test]
async fn unknown_and_empty_ones_are_refused_and_nothing_is_said() {
    let home = Home::new();
    home.write("commands/blank.md", "---\ndescription: x\n---\n  \n");
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = run(&mut client, "k1", &session, "/nothing-here").await;
    assert_eq!(reason(&reply), Some("unknown_command"), "{reply}");
    let reply = run(&mut client, "k2", &session, "/blank").await;
    assert_eq!(reason(&reply), Some("empty_message"), "{reply}");
    assert!(said(&home.log(&session)).is_empty());
}

#[tokio::test]
async fn a_preset_can_turn_one_off() {
    let home = Home::new();
    home.write("commands/review.md", REVIEW);
    home.write(
        "presets/quiet.toml",
        "[preset]\nname = \"安静\"\n\n[features]\ncommand-review = false\n",
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "preset": "quiet"}))
        .await;
    let reply = run(&mut client, "k1", &session, "/review a.rs").await;
    assert_eq!(reason(&reply), Some("command_not_allowed"), "{reply}");
    assert_eq!(
        reply["error"]["data"]["why"],
        "预设「安静」未启用 /review。"
    );
    assert!(said(&home.log(&session)).is_empty());
    let listed = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    assert!(!listed.to_string().contains("\"review\""), "{listed}");
    let preset = client
        .call("p1", "preset.get", json!({"preset": "quiet"}))
        .await;
    let feature = preset["result"]["features"]
        .as_array()
        .expect("有")
        .iter()
        .find(|one| one["id"] == "command-review")
        .cloned()
        .unwrap_or_else(|| panic!("列出来了：{preset}"));
    assert_eq!(
        feature,
        json!({"id": "command-review", "name": "/review", "summary": "审一个文件", "package": "~home", "on": false, "by": "preset", "installed": true, "tools": []})
    );
}

#[tokio::test]
async fn the_catalog_lists_templates_among_the_builtins() {
    let home = Home::new();
    home.write("commands/review.md", REVIEW);
    home.write("commands/plain.md", "\n  First line here.\nSecond.\n");
    home.write("commands/stop.md", "Not the builtin.\n");
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client.call("c1", "command.catalog", json!({})).await;
    let commands = reply["result"]["commands"].as_array().expect("有").clone();
    let names: Vec<&str> = commands
        .iter()
        .map(|one| one["name"].as_str().unwrap_or_default())
        .collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted, "照名字排");
    assert_eq!(
        names.iter().filter(|name| **name == "stop").count(),
        1,
        "和内置同名的模板不列"
    );
    let find = |name: &str| {
        commands
            .iter()
            .find(|one| one["name"] == name)
            .cloned()
            .unwrap_or_else(|| panic!("没列 {name}：{reply}"))
    };
    assert_eq!(
        find("review"),
        json!({"name": "review", "aliases": [], "summary": "审一个文件", "argument": "<file>"})
    );
    assert_eq!(
        find("plain"),
        json!({"name": "plain", "aliases": [], "summary": "First line here."}),
        "没写说明的照正文第一行"
    );
    let session = client.create("c2", "~").await;
    let listed = client
        .call("c3", "command.catalog", json!({"session": session}))
        .await;
    assert!(listed.to_string().contains("\"review\""), "{listed}");
}

/// 撤销最后一轮：回执那一句是人敲的那一行。
#[tokio::test]
async fn undoing_says_the_typed_line() {
    let home = Home::new();
    home.write("commands/review.md", REVIEW);
    let script = Script::new([Play::Says("好。")]);
    let mut client = Client::connect(home.core(&script));
    client.hello().await;
    let session = client.create("c1", "~").await;
    run(&mut client, "k1", &session, "/review a.rs").await;
    home.until_turns(&session, 1).await;
    let undone = client
        .call("u1", "session.revert", json!({"session": session}))
        .await;
    assert_eq!(undone["result"]["said"], "/review a.rs", "{undone}");
    let listed = client.call("l1", "session.list", json!({})).await;
    assert!(
        listed.to_string().contains("/review a.rs"),
        "会话列表的预览也是那一行：{listed}"
    );
}

/// 场所会话：和内置命令同一关，主人、管理的人能用，别人 `command_not_allowed`；不带 `as` 的 `venue_session`。
#[tokio::test]
async fn in_a_venue_the_same_people_as_for_builtins_may_run_one() {
    // 放在包里：场所会话的属主不一定是管理员，包里的谁都找得到。
    let home = Home::new();
    home.write(
        "packages/kit/package.toml",
        "[package]\nname = { en = \"Kit\" }\n",
    );
    home.write("packages/kit/commands/review.md", REVIEW);
    let script = Script::new([Play::Says("在。")]);
    let mut client = Client::connect(bound_core(&home, &script, Catalog::default()));
    client.hello().await;
    let made = client
        .call(
            "v1",
            "venue.session",
            json!({"venue": "qq:private:10001", "kind": "private", "peer": "qq:10001"}),
        )
        .await;
    let session = made["result"]["session"]
        .as_str()
        .expect("有编号")
        .to_string();
    let reply = run(&mut client, "k1", &session, "/review a.rs").await;
    assert_eq!(reason(&reply), Some("venue_session"), "{reply}");
    let member = json!({"external": "qq:20002", "role": "member"});
    let reply = client
        .call(
            "k2",
            "command.run",
            json!({"session": session, "text": "/review a.rs", "as": member}),
        )
        .await;
    assert_eq!(reason(&reply), Some("command_not_allowed"), "{reply}");
    // 认不出的照旧 `unknown_command`（先认再判身份）：场所里的桥照它把话当成平常说的记下。
    let reply = client
        .call(
            "k3",
            "command.run",
            json!({"session": session, "text": "/nothing-here", "as": member}),
        )
        .await;
    assert_eq!(reason(&reply), Some("unknown_command"), "{reply}");
    assert!(said(&home.log(&session)).is_empty());
}

/// `miyu check`（施工 X-14 上补）：被自己放的盖住的 `slash_covered`、和内置同名的 `slash_builtin`、写错的 `slash_skipped`。
#[tokio::test]
async fn check_says_which_templates_are_not_used() {
    let home = Home::new();
    home.write("commands/review.md", "Mine.\n");
    home.write("commands/stop.md", "Not the builtin.\n");
    home.write("commands/Bad.md", "x\n");
    home.write(
        "packages/kit/package.toml",
        "[package]\nname = { en = \"Kit\" }\n",
    );
    home.write("packages/kit/commands/review.md", "Theirs.\n");
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client.call("k1", "check", json!({})).await;
    let mut problems: Vec<(String, String, String)> = reply["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .filter(|one| one["kind"] == "slash")
        .map(|one| {
            let text = |key: &str| one[key].as_str().unwrap_or_default().to_string();
            (text("code"), text("file"), text("message"))
        })
        .collect();
    problems.sort();
    assert_eq!(problems.len(), 3, "{reply}");
    assert_eq!(problems[0].0, "slash_builtin");
    assert!(problems[0].1.ends_with("commands/stop.md"), "{problems:?}");
    assert!(problems[0].2.contains("/stop"), "{problems:?}");
    assert_eq!(problems[1].0, "slash_covered");
    assert!(
        problems[1].1.ends_with("kit/commands/review.md"),
        "{problems:?}"
    );
    assert!(problems[1].2.contains("commands/review.md"), "{problems:?}");
    assert_eq!(problems[2].0, "slash_skipped");
    assert!(problems[2].1.ends_with("commands/Bad.md"), "{problems:?}");
}
