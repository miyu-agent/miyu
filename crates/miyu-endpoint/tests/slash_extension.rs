//! 扩展声明的斜杠命令（施工 X-14 下，`docs/blueprint/slash.md`「扩展的命令」）：真核心拉起测试用的扩展，清单 `[[slash]]` 声明
//! 了 `/weather`。人敲了核心反向调用它的 `command.call`，它答的一段话（可以多行）原样当回执，记一条 `command.ran`；它答错的
//! `command_failed`、`data.why` 是原话，没连上的 `command_unavailable`。菜单、预设页列出来；自己放的同名模板盖住它；在后台答。

use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_session::testkit::Script;
use miyu_tool::Catalog;

use crate::support::extensions::*;
use crate::support::*;

/// 包 `wx` 的清单：程序 `program`、参数 `args`，要 `commands` 能力，声明 `/weather`。
fn manifest(program: &str, args: &[String]) -> String {
    let args: Vec<String> = args.iter().map(|arg| format!("{arg:?}")).collect();
    format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Weather\" }}\n\n[command]\nname = \"wx\"\nprogram = \"{program}\"\nabout = {{ en = \"W\" }}\n\n[process]\nargs = [{}]\nstart = \"manual\"\ncapabilities = [\"commands\"]\n\n[[slash]]\nname = \"weather\"\nsummary = {{ en = \"Today's weather\", zh = \"今天的天气\" }}\nargument = {{ zh = \"<城市>\" }}\n",
        args.join(", ")
    )
}

/// 等到 `path` 里记下的有 `n` 行。
async fn lines(path: &std::path::Path, n: usize) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(60);
    while read(path).lines().count() < n {
        assert!(tokio::time::Instant::now() < deadline, "等不到 {n} 行");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 真核心、开好 `wx`（批了 `commands`、它登记了一次空的工具），一个本机的会话。
async fn started(home: &Home, program: &Program) -> (Arc<miyu_endpoint::Core>, Client, String) {
    let (path, step) = record(home, "wx");
    let steps = steps(&[&step, "hello", "ask:provide:{\"tools\":[]}", "serve"]);
    home.write(
        "home/alice/packages/wx/package.toml",
        &manifest(&program.name(), &steps),
    );
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let enabled = client
        .call(
            "e1",
            "extension.enable",
            json!({"package": "wx", "approve": ["commands"]}),
        )
        .await;
    assert!(enabled["result"].is_object(), "{enabled}");
    lines(&path, 3).await;
    let session = client.create("c1", "~").await;
    (core, client, session)
}

async fn run(client: &mut Client, id: &str, session: &str, text: &str) -> Value {
    client
        .call(id, "command.run", json!({"session": session, "text": text}))
        .await
}

#[tokio::test]
async fn the_extension_answers_and_its_words_are_the_receipt() {
    let home = Home::new();
    let program = Program::new();
    let (_core, mut client, session) = started(&home, &program).await;
    let reply = run(&mut client, "k1", &session, " /weather tokyo ").await;
    assert_eq!(reply["result"]["command"], "weather", "{reply}");
    assert_eq!(reply["result"]["said"], "weather tokyo\nok", "多行原样");
    let log = home.log(&session);
    let ran: Vec<(String, String)> = log
        .iter()
        .filter_map(|event| match &event.body {
            Body::CommandRan(ran) => Some((ran.command.clone(), ran.text.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        ran,
        [("weather".to_string(), " /weather tokyo ".to_string())]
    );
    let (path, _) = record(&home, "wx");
    let asked = read(&path)
        .lines()
        .find(|line| line.contains("\"command.call\""))
        .map(str::to_string)
        .expect("核心发了 command.call");
    let asked: Value = serde_json::from_str(&asked).expect("是 JSON");
    let params = &asked["params"];
    assert_eq!(
        (
            params["command"].clone(),
            params["argument"].clone(),
            params["text"].clone()
        ),
        (json!("weather"), json!("tokyo"), json!("/weather tokyo"))
    );
    assert_eq!(params["session"], json!(session));
    assert_eq!(params["by"]["kind"], "person");
    assert_eq!(params["owner"], "alice");
}

#[tokio::test]
async fn an_error_or_a_wrong_answer_is_command_failed_and_nothing_is_noted() {
    let home = Home::new();
    let program = Program::new();
    let (_core, mut client, session) = started(&home, &program).await;
    let reply = run(&mut client, "k1", &session, "/weather boom").await;
    assert_eq!(reason(&reply), Some("command_failed"), "{reply}");
    assert_eq!(reply["error"]["data"]["why"], "boom");
    let reply = run(&mut client, "k2", &session, "/weather garbled").await;
    assert_eq!(reason(&reply), Some("command_failed"), "{reply}");
    assert!(
        !home
            .log(&session)
            .iter()
            .any(|event| matches!(event.body, Body::CommandRan(_))),
        "没办成的不记"
    );
}

#[tokio::test]
async fn not_running_is_unavailable() {
    let home = Home::new();
    let program = Program::new();
    home.write(
        "home/alice/packages/wx/package.toml",
        &manifest(&program.name(), &steps(&["hello", "wait"])),
    );
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let session = client.create("c1", "~").await;
    let reply = run(&mut client, "k1", &session, "/weather tokyo").await;
    assert_eq!(reason(&reply), Some("command_unavailable"), "{reply}");
}

#[tokio::test]
async fn menus_list_it_and_ones_own_template_covers_it() {
    let home = Home::new();
    let program = Program::new();
    let (_core, mut client, session) = started(&home, &program).await;
    let catalog = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    let weather = catalog["result"]["commands"]
        .as_array()
        .expect("有")
        .iter()
        .find(|one| one["name"] == "weather")
        .cloned()
        .unwrap_or_else(|| panic!("列了：{catalog}"));
    assert_eq!(
        weather,
        json!({"name": "weather", "aliases": [], "summary": "今天的天气", "argument": "<城市>"})
    );
    let preset = client
        .call("p1", "preset.get", json!({"preset": "full"}))
        .await;
    assert!(
        preset["result"]["features"]
            .as_array()
            .expect("有")
            .iter()
            .any(|one| one["id"] == "command-weather" && one["name"] == "/weather"),
        "{preset}"
    );
    home.write("home/alice/commands/weather.md", "Mine $ARGUMENTS.\n");
    let reply = run(&mut client, "k1", &session, "/weather tokyo").await;
    assert_eq!(
        reply["result"]["said"], "",
        "自己放的模板盖住扩展的：{reply}"
    );
    let catalog = client
        .call("c3", "command.catalog", json!({"session": session}))
        .await;
    let listed: Vec<Value> = catalog["result"]["commands"]
        .as_array()
        .expect("有")
        .iter()
        .filter(|one| one["name"] == "weather")
        .cloned()
        .collect();
    assert_eq!(listed.len(), 1, "菜单里只列盖住它的那一条：{catalog}");
    assert_eq!(listed[0]["summary"], "Mine $ARGUMENTS.");
}

/// 在后台答：扩展答得慢，同一个连接后面的请求不等它（同 `/dream`）。
#[tokio::test]
async fn it_is_answered_in_the_background() {
    let home = Home::new();
    let program = Program::new();
    let (_core, mut client, session) = started(&home, &program).await;
    let slow = json!({"jsonrpc": "2.0", "id": "k1", "method": "command.run",
                      "params": {"session": session, "text": "/weather silent"}});
    client.line(&slow.to_string()).await;
    let quick = tokio::time::timeout(
        Duration::from_secs(10),
        client.call("c9", "command.catalog", json!({})),
    )
    .await
    .expect("不等扩展答");
    assert_eq!(quick["id"], "c9", "{quick}");
}

/// 清单的 `[[slash]]` 能写出厂的 `access`（施工 O-6 六补）：写了 `off` 的谁都用不了，回 `command_not_allowed`，什么都不交给扩展；
/// 菜单里不列。系统配置写了的压过它（`command_access.rs`）。
#[tokio::test]
async fn a_shipped_access_off_turns_the_command_off() {
    let home = Home::new();
    let program = Program::new();
    let (path, step) = record(&home, "wx");
    let steps = steps(&[&step, "hello", "ask:provide:{\"tools\":[]}", "serve"]);
    let manifest = manifest(&program.name(), &steps).replace(
        "argument = { zh = \"<城市>\" }\n",
        "argument = { zh = \"<城市>\" }\naccess = \"off\"\n",
    );
    assert!(manifest.contains("access = \"off\""), "{manifest}");
    home.write("home/alice/packages/wx/package.toml", &manifest);
    let core = Arc::new(
        home.core_full(&Script::new([]), Catalog::default(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let enabled = client
        .call(
            "e1",
            "extension.enable",
            json!({"package": "wx", "approve": ["commands"]}),
        )
        .await;
    assert!(enabled["result"].is_object(), "{enabled}");
    lines(&path, 3).await;
    let session = client.create("c1", "~").await;
    let reply = run(&mut client, "k1", &session, "/weather tokyo").await;
    assert_eq!(reason(&reply), Some("command_not_allowed"), "{reply}");
    assert!(!read(&path).contains("\"command.call\""), "没交给扩展");
    let catalog = client
        .call("c2", "command.catalog", json!({"session": session}))
        .await;
    assert!(!catalog.to_string().contains("weather"), "{catalog}");
}
