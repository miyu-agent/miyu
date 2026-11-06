//! 扩展的工具声明 `runs_commands`（施工 XG-1 下，设计 `63-扩展工具的效果和声明.md` 第二节、第六节第 2 条、第七节第 3 条）：真核心
//! 拉起测试扩展走一遍。
//!
//! - 扩展起的命令里 `miyu call` 调得通，照会话的链判（记一条 `tool.nested`，挂在那次调用下面）；`tool.call` 带
//!   `channel: {sock, token, bin}`；不是扩展子孙的进程拿着令牌连上来，退出码 2；这次调用答完，端点关了，连不上；
//! - 包没批 `tools.commands` 的，`provide` 回 `needs_approval`；写法不对的 `bad_tool`；
//! - 说明后面接 `miyu call` 那一段；`!命令` 不交给扩展（只认内置 `shell`）；放行出沙盒的跑命令工具 `miyu check` 照旧提醒。

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use serde_json::{Value, json};

use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::support::extensions::*;
use crate::support::venues::configured_core;
use crate::support::*;

/// 等到 `path` 里记下的有 `n` 行。
pub(crate) async fn lines(path: &Path, n: usize) {
    until(&format!("记下 {n} 行"), || {
        read(path).lines().count() >= n
    })
    .await;
}

/// 一件会跑命令的工具：参数 `call` 是命令里要调的工具名。
pub(crate) fn runner(runs: Value) -> Value {
    json!({"tools": [{
        "name": "ext_run", "description": "Run a command.",
        "input_schema": {"type": "object", "properties": {"call": {"type": "string"}}, "required": ["call"]},
        "access": "read", "venues": ["local"], "runs_commands": runs,
    }]})
}

/// 装上包 `runner`，清单声明、批过 `capabilities`；扩展照 `steps` 走（记、握手、登记 `offered` 以后的那几步）。
pub(crate) fn install_runner(
    home: &Home,
    program: &Program,
    capabilities: &[&str],
    offered: &Value,
    rest: &[&str],
) -> std::path::PathBuf {
    let (path, keep) = record(home, "runner");
    let provide = format!("ask:provide:{offered}");
    let mut all = vec![keep.as_str(), "hello", provide.as_str()];
    all.extend_from_slice(rest);
    install_asking(
        home,
        "runner",
        &program.name(),
        "always",
        &steps(&all),
        capabilities,
    );
    approve(home, "runner", capabilities);
    path
}

/// 能在脚本里调的一件：回参数原文。
fn probe() -> Catalog {
    Catalog::new(vec![
        Fake::callable("probe", Access::Read, Act::Echo) as Arc<dyn Tool>
    ])
    .expect("合写法")
}

/// 照 `channel` 那一格连上去，拿令牌 `token` 调 `probe`，读回答；连不上的没有。测试进程是核心，是扩展的父进程，不是它的子孙。
pub(crate) fn knock(sock: &str, token: &str) -> Option<Value> {
    use std::io::{Read, Write};
    #[cfg(unix)]
    let mut stream = std::os::unix::net::UnixStream::connect(sock).ok()?;
    #[cfg(windows)]
    let mut stream = open_pipe(sock).ok()?;
    let body =
        json!({"v": 1, "op": "call", "tool": "probe", "args": "{}", "token": token}).to_string();
    stream
        .write_all(&u32::try_from(body.len()).ok()?.to_be_bytes())
        .ok()?;
    stream.write_all(body.as_bytes()).ok()?;
    let mut len = [0u8; 4];
    stream.read_exact(&mut len).ok()?;
    let mut reply = vec![0u8; u32::from_be_bytes(len) as usize];
    stream.read_exact(&mut reply).ok()?;
    serde_json::from_slice(&reply).ok()
}

/// Windows：命名管道当文件开。同 `miyu call`（`miyu-cli` 的 `connect`）：几个实例都忙着的（`ERROR_PIPE_BUSY`）等一会儿再试，最多
/// 两秒。核心这一头一个连上来以后才建下一个实例，扩展起的子进程刚连上时测试来敲，正好撞在没有空实例的那一小会儿（batch-1106-r3 的
/// Windows 红过，测试会话 e0 查的）。
#[cfg(windows)]
pub(crate) fn open_pipe(sock: &str) -> std::io::Result<std::fs::File> {
    const PIPE_BUSY: i32 = 231;
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    loop {
        match std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(sock)
        {
            Err(error)
                if error.raw_os_error() == Some(PIPE_BUSY)
                    && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            opened => return opened,
        }
    }
}

/// 扩展收到的 `tool.call`，照先后。
pub(crate) fn calls(path: &Path) -> Vec<Value> {
    read(path)
        .lines()
        .filter(|line| line.contains(r#""method":"tool.call""#))
        .map(|line| serde_json::from_str(line).expect("一行 JSON"))
        .collect()
}

#[tokio::test]
async fn a_command_the_extension_runs_calls_her_tools_and_only_its_own_processes_get_in() {
    let home = Home::new();
    let program = Program::new();
    let hold = home.work.join("hold.flag");
    let path = install_runner(
        &home,
        &program,
        &["tools", "tools.commands"],
        &runner(json!(true)),
        &[&format!("hold:{}", hold.display()), "serve"],
    );
    let script = Script::new([
        Play::calls(&[("ext_run", r#"{"call":"probe"}"#)]),
        Play::Says("好了。"),
    ]);
    let core = Arc::new(
        home.core_full(&script, probe(), None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    lines(&path, 3).await;
    assert!(
        read(&path).contains(r#""tools":1"#),
        "登记上了：{}",
        read(&path)
    );

    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "跑一下").await;
    until("扩展收到 tool.call", || !calls(&path).is_empty()).await;
    let call = calls(&path).remove(0);
    let channel = call["params"]["channel"].clone();
    let sock = channel["sock"]
        .as_str()
        .unwrap_or_else(|| panic!("带了端点：{call}"))
        .to_string();
    let token = channel["token"].as_str().expect("带了令牌").to_string();
    assert_eq!(token.len(), 32, "{channel}");
    assert!(channel["bin"].is_string(), "放 `miyu` 的目录：{channel}");

    // 扩展还没答：测试进程（核心，扩展的父进程）拿着令牌连上来，照「不是这条命令的进程」拒。
    let (knocked, at) = (sock.clone(), token.clone());
    let stranger = tokio::task::spawn_blocking(move || knock(&knocked, &at))
        .await
        .expect("跑完");
    let stranger = stranger.expect("连得上、答了");
    assert_eq!(stranger["code"], 2, "{stranger}");
    assert!(
        stranger["stderr"]
            .as_str()
            .unwrap_or_default()
            .starts_with("miyu call only works"),
        "{stranger}"
    );

    std::fs::write(&hold, "go").expect("写得进");
    home.until_turns(&session, 1).await;
    let log = logged(&home, &session);
    let result = log
        .iter()
        .find(|event| event["kind"] == json!("tool.result"))
        .expect("有结果");
    let text = result.to_string();
    assert!(text.contains("code=0"), "扩展的子进程调成了：{text}");
    assert!(text.contains("cwd="), "回了 probe 的结果：{text}");
    let nested: Vec<&Value> = log
        .iter()
        .filter(|event| event["kind"] == json!("tool.nested"))
        .collect();
    assert_eq!(nested.len(), 1, "照会话的链走、记一条：{log:?}");
    assert_eq!(nested[0]["body"]["name"], json!("probe"), "{}", nested[0]);
    assert_eq!(nested[0]["body"]["status"], json!("ok"), "{}", nested[0]);
    let parent = result["body"]["call_id"].as_str().expect("有编号");
    assert_eq!(
        nested[0]["body"]["id"],
        json!(format!("{parent}#1")),
        "挂在那次 ext_run 下面"
    );

    // 答完了：端点关了。
    tokio::time::sleep(Duration::from_millis(100)).await;
    let after = tokio::task::spawn_blocking(move || knock(&sock, &token))
        .await
        .expect("跑完");
    assert!(after.is_none(), "这次调用完了，连不上：{after:?}");
}

#[tokio::test]
async fn runs_commands_needs_tools_commands_approved_and_a_boolean() {
    let unapproved = json!({"reason": "needs_approval", "capabilities": ["tools.commands"]});
    let garbled = json!({"reason": "bad_tool", "tool": "ext_run", "problem": "runs_commands"});
    for (capabilities, runs, refused) in [
        (&["tools"][..], json!(true), unapproved),
        (&["tools", "tools.commands"][..], json!("yes"), garbled),
    ] {
        let home = Home::new();
        let program = Program::new();
        let path = install_runner(&home, &program, capabilities, &runner(runs), &["serve"]);
        let core = Arc::new(
            home.core_full(&Script::new([]), probe(), None, TOKEN)
                .with_extension_timing(quick()),
        );
        core.start_extensions();
        lines(&path, 3).await;
        let provided = read(&path)
            .lines()
            .nth(2)
            .map(str::to_string)
            .unwrap_or_default();
        let reply: Value = serde_json::from_str(&provided).unwrap_or_else(|_| panic!("{provided}"));
        assert_eq!(reply["error"]["data"], refused, "{reply}");
    }
}

/// 起核心、拉起登记 `offered` 的扩展、造一个会话、说一句，交回第一次请求里 `ext_run` 的说明。
async fn described(offered: &Value) -> String {
    let home = Home::new();
    let program = Program::new();
    let path = install_runner(
        &home,
        &program,
        &["tools", "tools.commands"],
        offered,
        &["serve"],
    );
    let script = Script::new([Play::Says("嗯。")]);
    let core = configured_core(&home, &script, probe());
    core.start_extensions();
    lines(&path, 3).await;
    let mut client = Client::connect(core);
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("s1", &session, "hi").await;
    home.until_turns(&session, 1).await;
    let request = script.requests()[0].1.clone();
    request
        .tools
        .iter()
        .find(|tool| tool.name == "ext_run")
        .map(|tool| tool.description.clone())
        .expect("有 ext_run")
}

#[tokio::test]
async fn the_miyu_call_section_follows_a_tool_that_runs_commands() {
    let runs = described(&runner(json!(true))).await;
    assert!(runs.starts_with("Run a command.\n"), "{runs}");
    assert!(
        runs.contains("miyu call <tool>") && runs.contains("probe"),
        "接上那一段、列出能调的：{runs}"
    );
    let plain = described(&runner(json!(false))).await;
    assert_eq!(plain, "Run a command.", "不跑命令的说明照登记的");
}

#[tokio::test]
async fn a_person_s_command_is_not_handed_to_an_extension() {
    let home = Home::new();
    let program = Program::new();
    let path = install_runner(
        &home,
        &program,
        &["tools", "tools.commands"],
        &runner(json!(true)),
        &["serve"],
    );
    home.write("etc/config.toml", "[sandbox.tools]\next_run = \"runner\"\n");
    let core = configured_core(&home, &Script::new([]), probe());
    core.start_extensions();
    lines(&path, 3).await;
    let mut client = Client::connect(core);
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    let reply = client
        .call(
            "c2",
            "session.send",
            json!({"session": session, "text": "ls", "shell": true}),
        )
        .await;
    assert_eq!(
        reply["error"]["data"]["reason"],
        json!("shell_unavailable"),
        "只认内置 shell：{reply}"
    );
    // 放行出沙盒的跑命令工具：照声明认，照旧提醒（`sandbox_shell`）。
    let checked = client.call("c3", "check", json!({})).await;
    let codes: Vec<&str> = checked["result"]["problems"]
        .as_array()
        .unwrap_or_else(|| panic!("{checked}"))
        .iter()
        .filter_map(|problem| problem["code"].as_str())
        .filter(|code| code.starts_with("sandbox_"))
        .collect();
    assert_eq!(codes, ["sandbox_shell"], "{checked}");
}
