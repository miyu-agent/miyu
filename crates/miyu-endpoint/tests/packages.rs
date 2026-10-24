//! 软件包清单（施工 9-1 上，`docs/blueprint/packages.md`）：真核心走一遍。`package.list` 列出出厂的、管理员家目录里的，
//! 名字、说明照连接的语言挑；写错的、撞了的、协议版本对不上的带代码和给人看的一句；核心起来时读一次；`check` 查清单。
//!
//! 测试用的是仓库的资源目录：出厂的清单会越来越多（终端界面、桥），这里只断言出厂的网页和测试自己放的那几份，家目录里
//! 放的编号、子命令名都避开出厂会有的。

use serde_json::{Value, json};

use miyu_session::testkit::Script;

use crate::support::*;

/// 终端界面的会话 2026-10-07 给的那份草稿：编号、子命令名改成 `term`，出厂以后才有的 `tui` 撞不上它。
const TERM: &str = r#"[package]
version = "0.0.1"
protocol = [1, 1]
name = { en = "Terminal interface", zh = "终端界面", ja = "ターミナル画面" }
summary = { en = "Chat with the AI in the terminal", zh = "在终端里和 AI 对话" }

[command]
name = "term"
program = "miyu-tui"
about = { en = "Open the terminal interface", zh = "打开终端界面", ja = "ターミナル画面を開く" }

[ui]
opens = ["config"]
"#;

/// 管理员（测试里是 alice）家目录里的一份清单。
fn mine(home: &Home, id: &str, text: &str) {
    home.write(&format!("system/packages/{id}/package.toml"), text);
}

/// 只留这几个编号的，照列出的先后。
fn only(packages: &[Value], ids: &[&str]) -> Vec<Value> {
    packages
        .iter()
        .filter(|package| ids.iter().any(|id| package["package"] == *id))
        .cloned()
        .collect()
}

async fn listed(home: &Home) -> Vec<Value> {
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    let reply = client.call("p1", "package.list", json!({})).await;
    reply["result"]["packages"]
        .as_array()
        .unwrap_or_else(|| panic!("{reply}"))
        .clone()
}

#[tokio::test]
async fn shipped_and_home_packages_are_listed_in_the_connections_language() {
    let home = Home::new();
    mine(&home, "term", TERM);
    let packages = listed(&home).await;
    let state = home.root.path().join("state").join("packages").join("term");
    assert_eq!(
        only(&packages, &["mermaid", "term"]),
        [
            json!({
                "package": "mermaid",
                "layer": "shipped",
                "from": "shipped",
                "author": "miyu-agent",
                "upstream": "https://github.com/miyu-agent/miyu",
                "kind": "builtin",
                "protocol": [1, 1],
                "name": "画 mermaid 图",
                "features": [],
                "state": home.root.path().join("state").join("packages").join("mermaid").to_string_lossy(),
                "status": "ready",
                "enabled": true,
            }),
            json!({
                "package": "term",
                "layer": "home",
                "from": "local",
                "kind": "ui",
                "version": "0.0.1",
                "protocol": [1, 1],
                "name": "终端界面",
                "summary": "在终端里和 AI 对话",
                "command": {"name": "term", "program": "miyu-tui", "about": "打开终端界面"},
                "commands": [{"name": "term", "program": "miyu-tui", "about": "打开终端界面"}],
                "opens": ["config"],
                "state": state.to_string_lossy(),
                "status": "program_missing",
            }),
        ],
        "握手说的是中文，照中文挑；没写的格不写"
    );
}

/// 种类多的两种、功能、平台接入、依赖、小程序（施工 F-1，设计 30）：照连接的语言列出来；没写功能的扩展包列出照包算的那一个，
/// 界面、小程序没有 `features`；必需的才有 `required`。
#[tokio::test]
async fn features_connections_and_workers_are_listed() {
    let home = Home::new();
    mine(
        &home,
        "xbase",
        r#"[package]
required = true
protocol = [1, 1]
name = { en = "Base", zh = "基础" }

[features.xfiles]
name = { en = "Files", zh = "文件读写" }
summary = { zh = "读写文件" }
tools = ["read"]

[features.xcmd]
name = { en = "Commands" }

[recommends]
workers = ["xembed"]

[builtin]
"#,
    );
    mine(
        &home,
        "xbridge",
        r#"[package]
protocol = [1, 1]
name = { en = "Connect X", zh = "接入X" }

[command]
name = "xbridge"
program = "miyu-xbridge"
about = { en = "Connect X", zh = "接入X" }

[connection]
platform = "x"

[depends]
workers = ["xembed"]

[process]
"#,
    );
    mine(
        &home,
        "xembed",
        r#"[package]
protocol = [1, 1]
name = { en = "Model" }

[worker]
program = "miyu-xembed"
args = ["serve"]
"#,
    );
    let packages = listed(&home).await;
    let mut got = only(&packages, &["xbase", "xbridge", "xembed"]);
    for package in &mut got {
        package.as_object_mut().unwrap().remove("state");
    }
    assert_eq!(
        got,
        [
            json!({
                "package": "xbase",
                "layer": "home",
                "from": "local",
                "kind": "builtin",
                "protocol": [1, 1],
                "name": "基础",
                "required": true,
                "features": [
                    {"id": "xfiles", "name": "文件读写", "summary": "读写文件"},
                    {"id": "xcmd", "name": "Commands"},
                ],
                "recommends": {"packages": ["xembed"]},
                "status": "ready",
            }),
            json!({
                "package": "xbridge",
                "layer": "home",
                "from": "local",
                "kind": "process",
                "protocol": [1, 1],
                "name": "接入X",
                "command": {"name": "xbridge", "program": "miyu-xbridge", "about": "接入X"},
                "commands": [{"name": "xbridge", "program": "miyu-xbridge", "about": "接入X"}],
                "process": {"args": [], "start": "manual"},
                "features": [{"id": "xbridge", "name": "接入X"}],
                "connection": {"platform": "x"},
                "depends": {"packages": ["xembed"]},
                "status": "program_missing",
                "enabled": false,
            }),
            json!({
                "package": "xembed",
                "layer": "home",
                "from": "local",
                "kind": "worker",
                "protocol": [1, 1],
                "name": "Model",
                "worker": {"program": "miyu-xembed", "args": ["serve"]},
                "status": "program_missing",
            }),
        ]
    );
}

/// 清单是内置包、核心里却没编进它的代码（施工 F-2，设计 30 第二节第 3 条）：照读坏了的清单报 `not_built_in`，别的不动。
#[test]
fn a_builtin_the_core_lacks_is_not_built_in() {
    let home = Home::new();
    mine(
        &home,
        "xghost",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Ghost\" }\n\n[builtin]\n",
    );
    mine(&home, "term", TERM);
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let mut found = miyu_endpoint::packages::load(&resources, &home.root, &alice());
    miyu_endpoint::packages::compiled(
        &mut found,
        &["basesystem", "memory", "mermaid", "net", "roleplay"],
    );
    let codes: Vec<(&str, Option<&str>)> = found
        .iter()
        .map(|one| {
            let code = match &one.read {
                Ok(_) => None,
                Err(miyu_store::packages::Issue::Wrong(problem)) => Some(problem.code.as_str()),
                Err(miyu_store::packages::Issue::Unreadable(_)) => Some("unreadable"),
            };
            (one.id.as_str(), code)
        })
        .collect();
    assert!(
        codes.contains(&("xghost", Some("not_built_in"))),
        "{codes:?}"
    );
    assert!(codes.contains(&("term", None)), "不是内置的不管");
    assert!(codes.contains(&("basesystem", None)), "编进来了的照常");
    assert!(
        miyu_endpoint::packages::is_installed(&found, "memory"),
        "读成了的内置包算装了"
    );
    assert!(!miyu_endpoint::packages::is_installed(&found, "xghost"));
    assert!(
        !miyu_endpoint::packages::is_installed(&found, "term"),
        "只认内置包"
    );
}

#[tokio::test]
async fn broken_taken_and_mismatched_ones_carry_a_code_and_a_sentence() {
    let home = Home::new();
    mine(&home, "bad", "[package]\n\n[ui]\n\n[process]\n");
    mine(&home, "mermaid", TERM);
    // 子命令撞了先读到的 `term`（施工 S-6 下：出厂的包都不带命令了，撞的是家目录里编号排在前面的那一份；出厂的先于家目录由
    // miyu-store 的单元测试守着）。
    mine(&home, "term", TERM);
    mine(&home, "web2", TERM);
    mine(
        &home,
        "later",
        &TERM
            .replace("[1, 1]", "[2, 3]")
            .replace("\"term\"", "\"later\""),
    );
    mine(
        &home,
        "bridge",
        "[package]\nprotocol = [1, 1]\nname = { en = \"Bridge\" }\n\n[command]\nname = \"bridge\"\nprogram = \"miyu-bridge\"\nabout = { en = \"QQ\" }\n\n[process]\nargs = [\"serve\"]\n\n[check]\nargs = [\"check\"]\n",
    );
    let packages = listed(&home).await;
    let by_id = |id: &str| {
        packages
            .iter()
            .filter(|package| package["package"] == id)
            .cloned()
            .collect::<Vec<_>>()
    };
    assert_eq!(
        by_id("bad"),
        [json!({
            "package": "bad",
            "layer": "home",
            "code": "two_programs",
            "line": 5,
            "problem": "一个包只带一个程序：[ui], [process] 只能留一张",
        })]
    );
    let mermaid = by_id("mermaid");
    assert_eq!(mermaid.len(), 2, "出厂的照常，家目录里同编号的那一份报错");
    assert_eq!(mermaid[1]["code"], "duplicate", "{mermaid:?}");
    assert_eq!(by_id("web2")[0]["code"], "command_taken");
    let later = &by_id("later")[0];
    assert_eq!(later["code"], "protocol_mismatch", "{later}");
    assert_eq!(later["protocol"], json!([2, 3]));
    assert_eq!(later["command"]["name"], "later", "对不上的照样列全");
    let bridge = &by_id("bridge")[0];
    assert_eq!(
        bridge["process"],
        json!({"args": ["serve"], "start": "manual"})
    );
    assert_eq!(bridge["check"], json!({"args": ["check"]}));
}

#[tokio::test]
async fn manifests_are_read_once_when_the_core_starts() {
    let home = Home::new();
    let core = home.core(&Script::new([]));
    mine(&home, "term", TERM);
    let mut client = Client::connect(core);
    client.hello().await;
    let reply = client.call("p1", "package.list", json!({})).await;
    let ids: Vec<&str> = reply["result"]["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|package| package["package"].as_str())
        .collect();
    assert!(ids.contains(&"mermaid"), "{reply}");
    assert!(!ids.contains(&"term"), "起来以后才放的，重启才认：{reply}");
    assert_eq!(only(&listed(&home).await, &["term"]).len(), 1);
}

#[tokio::test]
async fn check_reads_the_manifests_from_disk() {
    let home = Home::new();
    let mut client = Client::connect(home.core(&Script::new([])));
    client.hello().await;
    mine(&home, "bad", "[package]\n\n[ui]\n\n[process]\n");
    mine(&home, "term", TERM);
    let reply = client.call("c1", "check", json!({})).await;
    let packages: Vec<&Value> = reply["result"]["problems"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|problem| problem["kind"] == "package")
        .collect();
    assert_eq!(
        packages,
        [&json!({
            "kind": "package",
            "file": "system/packages/bad/package.toml",
            "code": "two_programs",
            "level": "error",
            "line": 5,
            "message": "一个包只带一个程序：[ui], [process] 只能留一张",
        })],
        "{reply}"
    );
    let file = home.root.path().join("system/packages/term/package.toml");
    let reply = client
        .call("c2", "check", json!({"file": file.to_string_lossy()}))
        .await;
    assert_eq!(reply["result"], json!({"problems": []}), "{reply}");
    let file = home.root.path().join("system/packages/bad/package.toml");
    let reply = client
        .call("c3", "check", json!({"file": file.to_string_lossy()}))
        .await;
    assert_eq!(
        reply["result"]["problems"][0]["code"], "two_programs",
        "{reply}"
    );
    // 包目录里别的文件、以前那种放在 `packages/` 下的 `<编号>.toml` 都不是清单（施工 F-8 上）。
    for stray in ["term/notes.toml", "old.toml"] {
        home.write(&format!("system/packages/{stray}"), "x");
        let file = home.root.path().join("system/packages").join(stray);
        let reply = client
            .call("c4", "check", json!({"file": file.to_string_lossy()}))
            .await;
        assert_eq!(reason(&reply), Some("unknown_file"), "{stray}: {reply}");
    }
}

/// 家目录里以前的写法（`<编号>.toml` 加同名目录，施工 F-8 上，设计 `31-软件包.md` 第七节第 2 条）：读清单时挪成一个文件夹一个包，
/// 照样读得成，包自己的文件照旧在。
#[test]
fn an_old_layout_in_the_home_is_moved_when_read() {
    let home = Home::new();
    // 施工 F-8 上补以前的清单还写着 `kind`：挪的同时改成照表认的。
    home.write(
        "system/packages/term.toml",
        &TERM.replace("[package]\n", "[package]\nkind = \"ui\"\n"),
    );
    home.write("system/packages/term/page/index.html", "<p>");
    let resources = miyu_store::resources::ResourceRoot::at(default_resources());
    let found = miyu_endpoint::packages::load(&resources, &home.root, &alice());
    assert!(
        found.iter().any(|one| one.id == "term" && one.read.is_ok()),
        "挪了照样读得成"
    );
    let dir = home.root.path().join("system/packages");
    assert!(dir.join("term/package.toml").is_file());
    assert!(!dir.join("term.toml").exists());
    assert!(
        dir.join("term/page/index.html").is_file(),
        "包自己的文件照旧在"
    );
    let text = std::fs::read_to_string(dir.join("term/package.toml")).expect("在");
    assert!(!text.contains("kind"), "{text}");
}

/// 只带 MCP 服务的包（施工 X-2）：种类是 `bundle`，不说协议；一个服务一个功能 `mcp-<服务名>`，名字是服务名；`mcp` 列出名字、
/// 拉起的命令、给哪种会话，不列环境变量；装之前看一眼的那一份列出服务名、没有程序那一格。
#[tokio::test]
async fn a_bundle_lists_its_mcp_servers_as_features() {
    let home = Home::new();
    mine(
        &home,
        "xgh",
        r#"[package]
name = { en = "GitHub tools", zh = "GitHub 工具" }

[[mcp]]
name = "xgithub"
command = ["npx", "-y", "server-github"]
env = { TOKEN = { secret = "github" } }
venues = ["local", "group"]
"#,
    );
    let packages = listed(&home).await;
    let mine = packages
        .iter()
        .find(|one| one["package"] == "xgh")
        .cloned()
        .expect("列得出");
    assert_eq!(mine["kind"], "bundle", "{mine}");
    assert!(mine.get("protocol").is_none(), "{mine}");
    assert_eq!(
        mine["features"],
        json!([{"id": "mcp-xgithub", "name": "xgithub"}])
    );
    assert_eq!(
        mine["mcp"],
        json!([{"name": "xgithub", "command": ["npx", "-y", "server-github"], "venues": ["local", "group"],
                "state": "off", "failures": 0, "need_load": {"value": true, "by": "source"}, "on": true}]),
        "关着的服务也带状态（施工 X-4）、加载方式和全局开关（施工 X-27 补）"
    );
    assert!(!mine.to_string().contains("TOKEN"), "环境变量不列：{mine}");
}
