//! 扩展的工具跑命令那条通道的对抗式复核（施工 XG-1 下，设计 `63-扩展工具的效果和声明.md` 第六节第 2 条；测试会话 e0）：真核心
//! 拉起测试扩展走一遍。
//!
//! - 只认扩展的子孙：扩展进程自己连上来、测试进程另起的进程（扩展的兄弟）拿着令牌连上来，都是退出码 2；扩展起的子进程照常调得通；
//! - 这一次调用超时、被掐以后，扩展的子孙再连，退出码 3；
//! - 命令里调的工具照会话判：不许在脚本里调的退出码 2，写工作区外面的要人批、通道里问不了，退出码 4，会话改成只读以后写的退出码
//!   1；被拒的那几次工具没跑。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::json;

use miyu_kernel::tool::Access;
use miyu_session::testkit::{Play, Script};
use miyu_tool::testkit::{Act, Fake};
use miyu_tool::{Catalog, Tool};

use crate::provide_commands::{calls, install_runner, knock, lines, runner};
use crate::support::extensions::*;
use crate::support::*;

/// 测试扩展的程序在哪：同 [`Program::new`] 放的那一份。
fn program_path(program: &Program) -> PathBuf {
    let exe = std::fs::canonicalize(std::env::current_exe().expect("找得到测试程序"))
        .expect("测试程序在");
    exe.parent().expect("有上一级").join(format!(
        "{}{}",
        program.name(),
        std::env::consts::EXE_SUFFIX
    ))
}

/// 日志里每一次工具结果的字，照先后。
fn results(home: &Home, session: &str) -> Vec<String> {
    logged(home, session)
        .iter()
        .filter(|event| event["kind"] == json!("tool.result"))
        .map(|event| event["body"]["blocks"].to_string())
        .collect()
}

/// 照 `call` 那一格起的核心：工具目录是 `tools`，扩展的时限设短。
fn core(home: &Home, script: &Script, tools: Catalog) -> Arc<miyu_endpoint::Core> {
    let core = Arc::new(
        home.core_full(script, tools, None, TOKEN)
            .with_extension_timing(quick()),
    );
    core.start_extensions();
    core
}

/// 能在脚本里调的 `probe`。
fn probe() -> Catalog {
    Catalog::new(vec![
        Fake::callable("probe", Access::Read, Act::Echo) as Arc<dyn Tool>
    ])
    .expect("合写法")
}

/// 照 `path` 写的路径：Windows 上反斜杠换成 `/`，测试扩展照字原样拼进参数。
fn slashed(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

#[tokio::test]
async fn neither_the_extension_itself_nor_a_stranger_with_the_token_gets_in() {
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
        Play::calls(&[("ext_run", r#"{"call":"@self:probe"}"#)]),
        Play::calls(&[("ext_run", r#"{"call":"probe"}"#)]),
        Play::Says("好了。"),
    ]);
    let core = core(&home, &script, probe());
    lines(&path, 3).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "跑一下").await;
    until("扩展收到 tool.call", || !calls(&path).is_empty()).await;
    let channel = calls(&path).remove(0)["params"]["channel"].clone();
    let (sock, token) = (
        channel["sock"].as_str().expect("带了端点").to_string(),
        channel["token"].as_str().expect("带了令牌").to_string(),
    );

    // 扩展还拿着这次调用：测试进程另起一个（扩展的兄弟，不是它的子孙）拿着令牌连。
    let exe = program_path(&program);
    let stranger = tokio::task::spawn_blocking(move || {
        std::process::Command::new(exe)
            .args(["client", "probe"])
            .env("MIYU_CALL_SOCK", &sock)
            .env("MIYU_CALL_TOKEN", &token)
            .output()
            .expect("起得来")
    })
    .await
    .expect("跑完");
    assert_eq!(
        stranger.status.code(),
        Some(2),
        "不是扩展的子孙：{}",
        String::from_utf8_lossy(&stranger.stdout)
    );

    std::fs::write(&hold, "go").expect("写得进");
    home.until_turns(&session, 1).await;
    let said = results(&home, &session);
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(said[0].contains("code=2"), "扩展进程自己不算：{}", said[0]);
    assert!(
        said[1].contains("code=0"),
        "扩展起的子进程调得通：{}",
        said[1]
    );
}

/// 扩展等测试的信号才去调：`flag` 有了才起子进程，结局写进 `out`。
fn late(home: &Home) -> (PathBuf, PathBuf, String) {
    let flag = home.work.join("late.flag");
    let out = home.work.join("late.out");
    let step = format!("hold:late:{}|{}", flag.display(), out.display());
    (flag, out, step)
}

/// 这一次调用的端点已经关了：测试进程拿着 `tool.call` 里的令牌再连，连不上（不是答了一句「不是自己人」）。
async fn closed(path: &Path) {
    let channel = calls(path).remove(0)["params"]["channel"].clone();
    let sock = channel["sock"].as_str().expect("带了端点").to_string();
    let token = channel["token"].as_str().expect("带了令牌").to_string();
    let knocked = tokio::task::spawn_blocking(move || knock(&sock, &token))
        .await
        .expect("跑完");
    assert!(knocked.is_none(), "端点关了，连不上：{knocked:?}");
}

/// 写 `flag` 放扩展去调，等它写下结局。
async fn after(flag: &Path, out: &Path) -> String {
    std::fs::write(flag, "go").expect("写得进");
    until("扩展写下结局", || out.exists()).await;
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    std::fs::read_to_string(out).expect("读得了")
}

#[tokio::test]
async fn after_a_timeout_even_a_descendant_is_told_the_call_has_ended() {
    let home = Home::new();
    let program = Program::new();
    let (flag, out, step) = late(&home);
    let mut offered = runner(json!(true));
    offered["tools"][0]["timeout_ms"] = json!(1000);
    let path = install_runner(
        &home,
        &program,
        &["tools", "tools.commands"],
        &offered,
        &[&step, "serve"],
    );
    let script = Script::new([
        Play::calls(&[("ext_run", r#"{"call":"probe"}"#)]),
        Play::Says("超时了。"),
    ]);
    let core = core(&home, &script, probe());
    lines(&path, 3).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "跑一下").await;
    home.until_turns(&session, 1).await;
    let said = results(&home, &session);
    assert!(
        said[0].contains("did not answer"),
        "这次调用超时了：{said:?}"
    );
    closed(&path).await;
    let late = after(&flag, &out).await;
    assert!(late.ends_with("code=3"), "超时以后子孙也调不成：{late}");
}

#[tokio::test]
async fn after_an_interrupt_even_a_descendant_is_told_the_call_has_ended() {
    let home = Home::new();
    let program = Program::new();
    let (flag, out, step) = late(&home);
    let path = install_runner(
        &home,
        &program,
        &["tools", "tools.commands"],
        &runner(json!(true)),
        &[&step, "serve"],
    );
    let script = Script::new([Play::calls(&[("ext_run", r#"{"call":"probe"}"#)])]);
    let core = core(&home, &script, probe());
    lines(&path, 3).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "跑一下").await;
    until("扩展收到 tool.call", || !calls(&path).is_empty()).await;
    let reply = client
        .call(
            "i1",
            "session.interrupt",
            json!({"session": session, "queued": "keep"}),
        )
        .await;
    assert!(reply.get("error").is_none(), "{reply}");
    home.until_turns(&session, 1).await;
    closed(&path).await;
    let late = after(&flag, &out).await;
    assert!(late.ends_with("code=3"), "被掐以后子孙也调不成：{late}");
}

#[tokio::test]
async fn a_command_s_calls_are_judged_as_the_session_would_judge_them() {
    let home = Home::new();
    let program = Program::new();
    let path = install_runner(
        &home,
        &program,
        &["tools", "tools.commands"],
        &runner(json!(true)),
        &["serve"],
    );
    let hidden = Fake::new("hidden", Access::Read, Act::Echo);
    let writer = Fake::callable("writer", Access::Write, Act::Echo);
    let reader = Fake::callable("reader", Access::Read, Act::Echo);
    let tools = Catalog::new(vec![
        Arc::clone(&hidden) as Arc<dyn Tool>,
        Arc::clone(&writer) as Arc<dyn Tool>,
        Arc::clone(&reader) as Arc<dyn Tool>,
    ])
    .expect("合写法");
    let inside = slashed(&home.work.join("in.txt"));
    let data_root = slashed(&home.root.path().join("secret.txt"));
    let run = |call: String| {
        let args = json!({ "call": call }).to_string();
        Play::calls(&[("ext_run", args.as_str())])
    };
    let script = Script::new([
        run("hidden".to_string()),
        run(format!("writer?path={inside}")),
        run(format!("reader?path={inside}")),
        run(format!("reader?path={data_root}")),
        Play::Says("好了。"),
    ]);
    let core = core(&home, &script, tools);
    lines(&path, 3).await;
    let mut client = Client::connect(Arc::clone(&core));
    client.hello().await;
    let session = client.create("c1", &home.work.to_string_lossy()).await;
    client.say("c2", &session, "跑一下").await;
    home.until_turns(&session, 1).await;

    let said = results(&home, &session);
    let codes: Vec<Option<&str>> = said
        .iter()
        .map(|text| {
            ["code=0", "code=1", "code=2", "code=3", "code=4"]
                .into_iter()
                .find(|code| text.contains(code))
        })
        .collect();
    assert_eq!(
        codes,
        [
            Some("code=2"),
            Some("code=2"),
            Some("code=0"),
            Some("code=1")
        ],
        "不许在脚本里调的、写的、读工作区里的、读数据根里的：{said:?}"
    );
    assert!(hidden.calls().is_empty(), "不许在脚本里调的没跑");
    assert!(writer.calls().is_empty(), "写的一律不经通道");
    assert_eq!(reader.calls().len(), 1, "只有读工作区里那一次跑了");
}
