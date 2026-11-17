//! 关了就全没，人人有开关（施工 UX-1 补，项目主人 2026-11-17 定）：只带技能、脚本的包、给别的包带内容的包也有开关
//! （`package.enable`、`package.disable`，记在 `extensions.json` 的 `on`）；关掉以后它带来的都不在：开着的会话下一轮、新会话的
//! 工具面没有它的脚本，技能列表、预设页不列，给别的包带的内容 `content.list` 不列；开回来都回来。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};

use crate::scripts::{core, echo};
use crate::support::extensions::read;
use crate::support::*;

/// 新会话会拿到的工具名（`session.tools` 不写会话）。
async fn face(client: &mut Client) -> Vec<String> {
    let reply = client.call("t", "session.tools", json!({})).await;
    names(&reply["result"]["tools"])
}

/// 一串里每一项的 `name`。
fn names(list: &Value) -> Vec<String> {
    list.as_array()
        .unwrap_or_else(|| panic!("{list}"))
        .iter()
        .filter_map(|one| one["name"].as_str().map(str::to_string))
        .collect()
}

/// 第 `n` 次请求发给模型的工具名。
fn sent(script: &Script, n: usize) -> Vec<String> {
    script.requests()[n]
        .1
        .tools
        .iter()
        .map(|tool| tool.name.clone())
        .collect()
}

/// 开关包 `id`：`method` 是 `package.enable` 或 `package.disable`，交回那一项。
async fn switch(client: &mut Client, method: &str, id: &str) -> Value {
    let reply = client.call("w", method, json!({"package": id})).await;
    reply["result"].clone()
}

#[tokio::test]
async fn a_package_switched_off_takes_everything_it_brought() {
    let home = Home::new();
    home.write(
        "opt/kit/package.toml",
        "[package]\nname = { en = \"Kit\" }\n",
    );
    home.write("opt/kit/scripts/dice.py", &echo("dice"));
    home.write(
        "opt/kit/skills/kit-notes/SKILL.md",
        "---\nname: kit-notes\ndescription: Take notes.\n---\n\nBody.\n",
    );
    let script = Arc::new(Script::new([
        Play::Says("一。"),
        Play::Says("二。"),
        Play::Says("三。"),
    ]));
    let core = core(&home, &script);
    let mut alice = Client::connect(Arc::clone(&core));
    alice.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = alice.create("c1", &cwd).await;
    alice.say("s1", &session, "一").await;
    home.until_turns(&session, 1).await;
    assert!(sent(&script, 0).contains(&"dice".to_string()), "关之前有");

    let off = switch(&mut alice, "package.disable", "kit").await;
    assert_eq!(
        (off["enabled"].clone(), off["status"].clone()),
        (json!(false), json!("off")),
        "只带技能、脚本的包也关得掉：{off}"
    );
    alice.say("s2", &session, "二").await;
    home.until_turns(&session, 2).await;
    assert!(
        !sent(&script, 1).contains(&"dice".to_string()),
        "开着的会话下一轮就没有"
    );
    assert!(
        !face(&mut alice).await.contains(&"dice".to_string()),
        "新会话没有"
    );
    let skills = alice.call("k", "skill.list", json!({})).await;
    assert!(
        !names(&skills["result"]["skills"]).contains(&"kit-notes".to_string()),
        "技能列表不列：{skills}"
    );
    let preset = alice
        .call("p", "preset.get", json!({"preset": "full"}))
        .await;
    assert!(
        !preset["result"]["features"]
            .as_array()
            .unwrap_or_else(|| panic!("{preset}"))
            .iter()
            .any(|one| one["id"] == "script-dice" || one["id"] == "kit"),
        "预设页不列：{preset}"
    );

    let on = switch(&mut alice, "package.enable", "kit").await;
    assert_eq!(on["enabled"], true, "{on}");
    alice.say("s3", &session, "三").await;
    home.until_turns(&session, 3).await;
    assert!(
        sent(&script, 2).contains(&"dice".to_string()),
        "开回来就回来"
    );
    let skills = alice.call("k2", "skill.list", json!({})).await;
    assert!(
        names(&skills["result"]["skills"]).contains(&"kit-notes".to_string()),
        "{skills}"
    );
}

/// 记下的每一行里能读成 JSON 的。
fn said(path: &std::path::Path) -> Vec<Value> {
    read(path)
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect()
}

/// 等到记下编号是 `id` 的回应，交回它。
async fn reply(path: &std::path::Path, id: &str) -> Value {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(found) = said(path).into_iter().find(|line| line["id"] == id) {
            return found;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "等不到 {id}：{}",
            read(path)
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

#[tokio::test]
async fn a_content_package_switched_off_is_not_listed() {
    use crate::support::extensions::{Program, install, quick, record, steps};
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "memes");
    let first = home.work.join("go-1");
    let second = home.work.join("go-2");
    let (first_step, second_step) = (
        format!("pause:{}", first.display()),
        format!("pause:{}", second.display()),
    );
    // 第 2、4、5、7 步各列一次（编号 s2、s4、s5、s7），中间等测试放行。扩展一步只读一行：推来的 `content.changed` 会先被当成
    // 回应读走，真的回应留给下一步读，所以 s4 后面多列一次，好让 s4 在测试往下走以前就记下。
    let all = [
        step.as_str(),
        "hello",
        "call:content.list",
        first_step.as_str(),
        "call:content.list",
        "call:content.list",
        second_step.as_str(),
        "call:content.list",
        "listen",
    ];
    install(&home, "memes", &program.name(), "always", &steps(&all));
    home.write(
        "opt/pack/package.toml",
        "[package]\nname = { en = \"Pack\" }\n\n[content.memes]\ndir = \"stickers\"\n",
    );
    home.write("opt/pack/stickers/index.json", "{}");
    let core = crate::support::extensions::core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let listed = |reply: &Value| reply["result"]["packages"].clone();
    assert_eq!(
        listed(&reply(&path, "s2").await),
        json!([{"package": "pack"}])
    );

    let off = switch(&mut client, "package.disable", "pack").await;
    assert_eq!(off["enabled"], false, "内容包也关得掉：{off}");
    std::fs::write(&first, "").expect("写得进");
    assert_eq!(listed(&reply(&path, "s4").await), json!([]), "关了不列");

    let on = switch(&mut client, "package.enable", "pack").await;
    assert_eq!(on["enabled"], true, "{on}");
    std::fs::write(&second, "").expect("写得进");
    assert_eq!(
        listed(&reply(&path, "s7").await),
        json!([{"package": "pack"}]),
        "开回来就列"
    );
    assert!(
        said(&path)
            .iter()
            .filter(|line| line["method"] == "content.changed")
            .count()
            >= 1,
        "开关推了 content.changed：{}",
        read(&path)
    );
    core.stop_extensions().await;
}

/// 开包只有一条路（施工 UX-1修，im 换钉后三条测试红的根因）：手动开的扩展给别的包带了内容，经 `extension.enable` 打开也推
/// `content.changed`（同 `package.enable`），带内容的宿主才知道去重读；`extension.disable` 同样推，重读不再列它。
#[tokio::test]
async fn an_extension_switched_by_extension_enable_pushes_its_content() {
    use crate::support::extensions::{Program, install, quick, record, steps};
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "memes");
    let all = [step.as_str(), "hello", "call:content.list", "listen"];
    install(&home, "memes", &program.name(), "always", &steps(&all));
    install(
        &home,
        "late",
        &program.name(),
        "manual",
        &steps(&["hello", "wait"]),
    );
    let manifest = home.root.packages().join("late/package.toml");
    let text = std::fs::read_to_string(&manifest).expect("装好了");
    std::fs::write(&manifest, format!("{text}\n[content.memes]\ndir = \"x\"\n")).expect("写得进");
    home.write("opt/late/x/index.json", "{}");
    let core = crate::support::extensions::core(&home, quick());
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    assert_eq!(
        reply(&path, "s2").await["result"]["packages"],
        json!([]),
        "手动开的关着，不列"
    );
    let pushes = |path: &std::path::Path| {
        said(path)
            .iter()
            .filter(|line| line["method"] == "content.changed")
            .count()
    };
    let until = async |n: usize| {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
        while pushes(&path) < n {
            assert!(
                tokio::time::Instant::now() < deadline,
                "等不到第 {n} 条 content.changed：{}",
                read(&path)
            );
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    };
    let on = client
        .call("e", "extension.enable", json!({"package": "late"}))
        .await;
    assert_eq!(on["result"]["on"], true, "{on}");
    until(1).await;
    let off = client
        .call("d", "extension.disable", json!({"package": "late"}))
        .await;
    assert_eq!(off["result"]["on"], false, "{off}");
    until(2).await;
    core.stop_extensions().await;
}
