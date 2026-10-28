//! 她看软件包（施工 F-10 上，设计 `31-软件包.md` 第六节）：真核心的会话调 `packages`，经协议端点交给会话的端口看：列出、看一个、
//! 看一个包文件夹，和协议的 `package.list`、`package.info`、装之前看一眼是同一份，名字照英文；看文件夹什么都不装。施工 BS-1 中
//! 起它不上工具面，她跑的命令里的 `miyu pkg` 经通道调（`support::pkg`）；`miyu call` 调不到它。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::Body;
use miyu_session::testkit::{Play, Script};
use miyu_tool::Catalog;

use crate::support::*;

/// 一个界面包。
/// 不带程序的包（施工 U23 下上：会话里看一眼的是装进属主自己那一层的样子，带程序的照拒）。
const PANE: &str = "[package]\nversion = \"2\"\nname = { en = \"Pane\", zh = \"面板\" }\n";

/// 会话里每一条工具结果的字，照先后。
fn home_results(home: &Home, session: &str) -> Vec<String> {
    home.log(session)
        .into_iter()
        .filter_map(|event| match event.body {
            Body::ToolResult(result) => Some(
                result
                    .blocks
                    .iter()
                    .map(|block| match block {
                        Block::Text(Text { text }) => text.clone(),
                        _ => String::new(),
                    })
                    .collect::<String>(),
            ),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn a_session_looks_at_packages_through_the_core() {
    let home = Home::new();
    let folder = home.work.join("pane");
    std::fs::create_dir_all(&folder).expect("建得了");
    std::fs::write(folder.join("package.toml"), PANE).expect("写得进");
    let path = json!({"path": folder}).to_string();
    let script = Script::new([
        Play::calls(&[("shell", &pkg::run(&[("packages", "{}")]))]),
        Play::calls(&[(
            "shell",
            &pkg::run(&[("packages", r#"{"package":"shell"}"#)]),
        )]),
        Play::calls(&[("shell", &pkg::run(&[("packages", &path)]))]),
        Play::Says("看完了。"),
    ]);
    let core = Arc::new(home.core_full(&script, pkg::catalog(), None, TOKEN));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "看看装了什么").await;
    home.until_turns(&session, 1).await;
    let results: Vec<String> = home_results(&home, &session)
        .iter()
        .map(|line| pkg::text(line).to_string())
        .collect();
    assert_eq!(results.len(), 3, "{results:?}");
    assert!(
        results[0]
            .lines()
            .any(|line| line.starts_with("shell: Commands.")),
        "列出来一个一行，名字照英文：{}",
        results[0]
    );
    let info: serde_json::Value =
        serde_json::from_str(results[1].trim_end()).unwrap_or_else(|_| panic!("{}", results[1]));
    assert_eq!(
        (
            info["package"].clone(),
            info["name"].clone(),
            info["layer"].clone()
        ),
        (json!("shell"), json!("Commands"), json!("shipped")),
        "{info}"
    );
    let (lead, preview) = results[2].split_once('\n').unwrap_or_default();
    // 换进模板的字照模板的规矩转义：Windows 上路径里的反斜杠写成两个。
    let shown = miyu_kernel::template::escape(&folder.display().to_string());
    assert_eq!(
        lead,
        format!("{shown} is a valid package. Installing it gives:")
    );
    let preview: serde_json::Value =
        serde_json::from_str(preview.trim_end()).unwrap_or_else(|_| panic!("{}", results[2]));
    assert_eq!(
        (
            preview["package"].clone(),
            preview["version"].clone(),
            preview["program"].clone()
        ),
        (json!("pane"), json!("2"), Value::Null)
    );
    assert!(
        !home.root.path().join("packages/pane").exists(),
        "看一眼不装"
    );
    drop(client);
    drop(core);

    // 核心重启过，载入的会话照样看得到（端口在载入时交进来）。
    let again = Script::new([
        Play::calls(&[("shell", &pkg::run(&[("packages", r#"{"package":"net"}"#)]))]),
        Play::Says("嗯。"),
    ]);
    let mut client = Client::connect(Arc::new(home.core_full(
        &again,
        pkg::catalog(),
        None,
        TOKEN,
    )));
    client.hello().await;
    client.say("s2", &session, "再看一个").await;
    home.until_turns(&session, 2).await;
    let last = home_results(&home, &session);
    assert!(
        last.last()
            .is_some_and(|text| text.contains(r#""package":"net""#)),
        "{last:?}"
    );
}

/// 脚本里的 `miyu call` 调不到 `packages`（施工 BS-1 中）：它不上工具面，当不认识；会话里改用 `miyu pkg`。
#[tokio::test]
async fn a_script_cannot_reach_packages_with_miyu_call() {
    use miyu_kernel::tool::Access;
    use miyu_tool::Tool;
    use miyu_tool::testkit::{Act, Fake};
    let home = Home::new();
    let script = Script::new([Play::calls(&[("shell", "{}")]), Play::Says("调不到。")]);
    let shell = Fake::commanding(
        "shell",
        Access::Read,
        Some(miyu_tool::RUN_IN_BACKGROUND),
        Act::Calls(vec![("packages", "{}")]),
    );
    let mut tools: Vec<Arc<dyn Tool>> = miyu_basesystem::tools(&default_resources())
        .expect("出厂的工具")
        .into_iter()
        .filter(|tool| tool.spec().name != "shell")
        .collect();
    tools.push(shell);
    let core = Arc::new(home.core_full(&script, Catalog::new(tools).expect("合写法"), None, TOKEN));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", "~").await;
    client.say("s1", &session, "看看装了什么").await;
    home.until_turns(&session, 1).await;
    assert_eq!(home_results(&home, &session), ["packages refused Unknown"]);
}

/// 关了「脚本调工具」、开着 `packages` 的会话（施工 BS-1 中）：脚本里一件能调的都没有，通道照样开给 `miyu pkg`。
#[tokio::test]
async fn miyu_pkg_works_where_scripts_cannot_call_tools() {
    let home = Home::new();
    home.write(
        "presets/pkg.toml",
        "[preset]\nname = { en = \"Pkg\" }\nunlisted = \"off\"\n\n[software]\nshell = true\npackages = true\n",
    );
    let script = Script::new([
        Play::calls(&[(
            "shell",
            &pkg::run(&[("packages", r#"{"package":"shell"}"#)]),
        )]),
        Play::Says("看完了。"),
    ]);
    let core = Arc::new(home.core_full(&script, pkg::catalog(), None, TOKEN));
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client
        .create_with("c1", json!({"cwd": "~", "preset": "pkg"}))
        .await;
    client.say("s1", &session, "看看 shell 包").await;
    home.until_turns(&session, 1).await;
    let results = home_results(&home, &session);
    assert_eq!(results.len(), 1, "{results:?}");
    assert!(
        pkg::text(&results[0]).contains(r#""package":"shell""#),
        "{results:?}"
    );
}
