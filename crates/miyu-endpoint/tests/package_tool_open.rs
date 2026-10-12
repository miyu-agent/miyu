//! 她打开装着的包、装上的包在会话的预设里打开（施工 F-10 下下，`docs/blueprint/tools/package.md`，设计 `31-软件包.md` 第六节、
//! 16 第三节）：真核心的会话调 `package`。打开扩展问人一次，卡片里有它要的能力，允许了就打开、能力一并批；程序不在的不问、当场
//! 拒。不开没列出来的软件的预设里装的包，往那一份预设的同名覆盖里写上打开；开着没列出来的预设不写。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::extensions::Program;
use crate::support::*;

/// 真核心，工具目录是基础系统；在工作区里造预设是 `preset` 的会话，说一句。
async fn started(home: &Home, plays: Vec<Play>, preset: &str) -> (Client, String) {
    let tools = Catalog::new(miyu_basesystem::tools(&default_resources()).expect("出厂的工具"))
        .expect("合写法");
    let core = Arc::new(home.core_full(&Script::new(plays), tools, None, TOKEN));
    let mut client = Client::connect(core);
    client.hello().await;
    let cwd = home.work.to_string_lossy().into_owned();
    let session = client
        .create_with("c1", json!({"cwd": cwd, "preset": preset}))
        .await;
    client.say("s1", &session, "开吧").await;
    (client, session)
}

/// 日志里的确认请求，照先后。
fn requests(home: &Home, session: &str) -> Vec<Value> {
    logged(home, session)
        .into_iter()
        .filter(|event| event["kind"] == json!("tool.approval_requested"))
        .map(|event| event["body"].clone())
        .collect()
}

/// 等到第 `n` 条确认请求，允许这一次，交回它。
async fn allow(client: &mut Client, home: &Home, session: &str, n: usize) -> Value {
    until("确认请求", || requests(home, session).len() >= n).await;
    let asked = requests(home, session).remove(n - 1);
    let call = asked["call_id"].as_str().expect("有编号").to_string();
    let reply = client
        .call(
            "a",
            "session.answer",
            json!({"session": session, "call": call, "decision": "once"}),
        )
        .await;
    assert!(reply["result"].is_object(), "{reply}");
    asked
}

/// `package.list` 里编号是 `id` 的那一项。
async fn entry(client: &mut Client, id: &str) -> Value {
    let reply = client.call("l", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .iter()
        .find(|one| one["package"] == id)
        .cloned()
        .unwrap_or_else(|| panic!("没有 {id}：{reply}"))
}

#[tokio::test]
async fn opening_an_extension_asks_with_its_capabilities() {
    let home = Home::new();
    let program = Program::new();
    home.write(
        "home/alice/packages/relay/package.toml",
        &format!(
            "[package]\nprotocol = [1, 1]\nname = {{ en = \"Relay\" }}\n\n[command]\nname = \"relay\"\nprogram = \"{}\"\nabout = {{ en = \"Relay\" }}\n\n[process]\nargs = [\"hello\", \"wait\"]\nstart = \"manual\"\ncapabilities = [\"tools\"]\n",
            program.name()
        ),
    );
    let plays = vec![
        Play::calls(&[("package", r#"{"enable":"relay"}"#)]),
        Play::Says("开了。"),
    ];
    let (mut client, session) = started(&home, plays, "full").await;
    let asked = allow(&mut client, &home, &session, 1).await;
    assert_eq!(asked["access"], "install", "{asked}");
    let preview = &asked["detail"]["preview"];
    assert_eq!(preview["program"], "process", "{asked}");
    assert_eq!(
        preview["capabilities"][0]["id"], "tools",
        "卡片里有要的能力：{asked}"
    );
    home.until_turns(&session, 1).await;
    assert_eq!(
        entry(&mut client, "relay").await["enabled"],
        true,
        "允许了就打开"
    );
}

#[tokio::test]
async fn a_package_whose_program_is_missing_is_refused_without_asking() {
    let home = Home::new();
    home.write(
        "home/alice/packages/ghost/package.toml",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Ghost\" }\n\n[command]\nname = \"ghost\"\nprogram = \"miyu-no-such-program-anywhere\"\nabout = { en = \"G\" }\n\n[process]\n",
    );
    let plays = vec![
        Play::calls(&[("package", r#"{"enable":"ghost"}"#)]),
        Play::Says("打不开。"),
    ];
    let (_client, session) = started(&home, plays, "full").await;
    home.until_turns(&session, 1).await;
    assert!(requests(&home, &session).is_empty(), "看不了的不问");
}

#[tokio::test]
async fn installing_in_a_preset_that_keeps_unlisted_off_opens_it_there() {
    for (preset, writes) in [("dev", true), ("full", false)] {
        // 一趟一个数据根、一个包：编号照文件夹名，`kit-dev`、`kit-full`。
        let home = Home::new();
        let id = format!("kit-{preset}");
        let folder = home.work.join(&id);
        std::fs::create_dir_all(folder.join("skills/notes")).expect("建得了");
        std::fs::write(
            folder.join("package.toml"),
            "[package]\nname = { en = \"Kit\" }\n",
        )
        .expect("写得进");
        std::fs::write(
            folder.join("skills/notes/SKILL.md"),
            "---\nname: notes\ndescription: Take notes.\n---\nbody\n",
        )
        .expect("写得进");
        let install = json!({"install": folder}).to_string();
        let plays = vec![
            Play::calls(&[("package", &install)]),
            Play::Says("装好了。"),
        ];
        let (mut client, session) = started(&home, plays, preset).await;
        allow(&mut client, &home, &session, 1).await;
        home.until_turns(&session, 1).await;
        assert!(
            home.root
                .path()
                .join("home/alice/packages")
                .join(&id)
                .exists(),
            "装上了 {id}"
        );
        let file = home
            .root
            .path()
            .join(format!("home/alice/presets/{preset}.toml"));
        let written = std::fs::read_to_string(&file).unwrap_or_default();
        assert_eq!(
            written.contains(&format!("{id} = true")),
            writes,
            "预设 {preset}：不开没列出来的才写：{written}"
        );
    }
}
