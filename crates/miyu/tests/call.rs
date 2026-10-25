//! 脚本调工具（施工 M-3，设计 `39-脚本调工具.md` 第五节、第六节）：真的 `miyu core`，模型是本机回环上的假服务器，照 DeepSeek
//! 的写法回一次调 `shell`、再说一句。命令里经 `PATH` 最前面的 `miyu` 调这个会话的工具，每次的标准输出、标准错误、退出码写进
//! 工作目录里的文件，测试读它们：
//!
//! - 能调的（`memory_search`）印一行 JSON、退出码 0；子 shell 里再起一层照样调得成；
//! - 调不了的（`shell` 自己）、不认识的退出码 2，说怎么改；`--schema` 印说明和参数格式；
//! - `setsid` 出去、离开了这条命令进程组的（Linux），退出码 2；
//! - 命令结束以后，同一个端点再连不上；在自己的终端里（没有那两个变量）退出码 2。
//! - 每次调用都记一条 `tool.nested`，挂在那次 `shell` 下面，排在它的结果前面。
//!
//! 三个平台都跑（Windows 的通道施工 M-3 下）：Unix 上命令是 bash、zsh，Windows 上是 PowerShell，几步照各自的写法拼；
//! Windows 上用 `Start-Process` 接住输出和退出码，免得 PowerShell 把程序的输出换了编码。

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader as AsyncReader};

use crate::support::{Home, MIYU, within};
use miyu_http::testkit::{Piece, Reply, Server};

/// 一段事件流：照 DeepSeek 的写法，一块 `data:` 一行，最后 `[DONE]`。
fn stream(chunks: &[Value]) -> Reply {
    let mut body = String::new();
    for chunk in chunks {
        body.push_str(&format!("data: {chunk}\n\n"));
    }
    body.push_str("data: [DONE]\n\n");
    Reply::stream(vec![Piece::Bytes(body.into_bytes())])
}

/// 模型的两次回复：先照 `args` 调 `shell`，再说一句。
fn replies(args: &Value) -> Vec<Reply> {
    let call = json!({"choices": [{"index": 0, "delta": {"role": "assistant", "tool_calls": [{
        "index": 0, "id": "call_a", "type": "function",
        "function": {"name": "shell", "arguments": args.to_string()}}]}, "finish_reason": null}]});
    let done = json!({"choices": [{"index": 0, "delta": {}, "finish_reason": "tool_calls"}]});
    let said = json!({"choices": [{"index": 0, "delta": {"role": "assistant", "content": "好了。"},
        "finish_reason": "stop"}]});
    vec![stream(&[call, done]), stream(&[said])]
}

/// 进工作目录 `work`：工作目录在数据根里，会话照 5-14 换成它自己的工作区，命令里自己进。
#[cfg(unix)]
fn enter(work: &Path) -> String {
    format!("cd '{}' || exit 1\n", work.display())
}

/// 同上，PowerShell。
#[cfg(windows)]
fn enter(work: &Path) -> String {
    format!("Set-Location -LiteralPath '{}'\n", work.display())
}

/// 一次 `miyu call`：参数是 `args`，结果写进 `<name>.out`、`<name>.err`、`<name>.code`。
#[cfg(unix)]
fn step(name: &str, args: &str) -> String {
    format!("miyu call {args} > {name}.out 2> {name}.err; echo $? > {name}.code\n")
}

/// 同上，PowerShell：输出原样接进文件，退出码自己写。
#[cfg(windows)]
fn step(name: &str, args: &str) -> String {
    format!(
        "$p = Start-Process -FilePath (Get-Command miyu).Source -ArgumentList 'call {args}' -NoNewWindow -Wait -PassThru -RedirectStandardOutput {name}.out -RedirectStandardError {name}.err; [IO.File]::WriteAllText(\"$PWD\\{name}.code\", \"$($p.ExitCode)\")\n"
    )
}

/// 子进程里再起一层调：退出码写进 `child.code`。
#[cfg(unix)]
fn child() -> &'static str {
    "bash -c 'miyu call memory_search --query nested' > child.out 2> child.err; echo $? > child.code\n"
}

/// 同上，经 `cmd` 再起一层。
#[cfg(windows)]
fn child() -> &'static str {
    "cmd /c \"miyu call memory_search --query nested >NUL 2>NUL\"; [IO.File]::WriteAllText(\"$PWD\\child.code\", \"$LASTEXITCODE\")\n"
}

/// 等三秒。
fn pause() -> &'static str {
    if cfg!(windows) {
        "& { Start-Sleep -Seconds 3 }\n"
    } else {
        "sh -c 'sleep 3'\n"
    }
}

/// 收尾：端点的两个变量记进 `endpoint.txt`（命令结束以后测试自己连），写一个结束的记号。
#[cfg(unix)]
fn finish() -> &'static str {
    "printf '%s\\n%s\\n' \"$MIYU_CALL_SOCK\" \"$MIYU_CALL_TOKEN\" > endpoint.txt\necho done > done.flag\n"
}

/// 同上，PowerShell。
#[cfg(windows)]
fn finish() -> &'static str {
    "[IO.File]::WriteAllText(\"$PWD\\endpoint.txt\", \"$env:MIYU_CALL_SOCK`n$env:MIYU_CALL_TOKEN\")\n[IO.File]::WriteAllText(\"$PWD\\done.flag\", 'done')\n"
}

/// 命令：进 `work`，几次调用挨着跑，收尾。
fn script(work: &Path) -> String {
    let mut script = enter(work);
    script.push_str(&step("found", "memory_search --query zzz"));
    script.push_str(&step("itself", "shell --command ls"));
    script.push_str(&step("typo", "memory_serch --query zzz"));
    script.push_str(&step("schema", "--schema memory_search"));
    script.push_str(child());
    if cfg!(target_os = "linux") {
        script.push_str("setsid miyu call memory_search --query away > away.out 2> away.err; echo $? > away.code\n");
    }
    script.push_str(finish());
    script
}

/// 在连接上说 JSON-RPC：发一条，读到它的回应为止（推送跳过）。
struct Rpc {
    read: AsyncReader<tokio::io::ReadHalf<miyu_ipc::Connection>>,
    write: tokio::io::WriteHalf<miyu_ipc::Connection>,
}

impl Rpc {
    async fn call(&mut self, id: &str, method: &str, params: Value) -> Value {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.write
            .write_all(format!("{request}\n").as_bytes())
            .await
            .expect("写得进");
        loop {
            let mut line = String::new();
            within(&format!("{method} 的回应"), self.read.read_line(&mut line))
                .await
                .expect("读得到");
            let reply: Value = serde_json::from_str(&line).expect("是 JSON");
            if reply["id"] == json!(id) {
                assert!(reply.get("error").is_none(), "{method}：{reply}");
                return reply["result"].clone();
            }
        }
    }
}

/// 拉起核心：模型是 `server`，等它写来 `ready`。
fn start(home: &Home, server: &Server) -> Child {
    let config = format!(
        "[providers.dev]\ndriver = \"openai-chat\"\nbase_url = \"{}\"\ncatalog = \"deepseek\"\nkey = {{ env = \"MIYU_TEST_KEY\" }}\n\n[models]\nchat = \"dev/deepseek-chat\"\n",
        server.base_url
    );
    std::fs::write(home.root.path().join("system").join("config.toml"), config).expect("写得进");
    let mut core = home
        .core()
        .env("MIYU_TEST_KEY", "test")
        .env("NO_PROXY", "127.0.0.1")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .stdout(Stdio::piped())
        .spawn()
        .expect("起得来");
    let mut line = String::new();
    BufReader::new(core.stdout.take().expect("接了管道"))
        .read_line(&mut line)
        .expect("读得到那一行");
    assert_eq!(line, "ready\n", "核心起来了：{}", home.core_log());
    core
}

/// 工作目录里的一个文件，去掉末尾的换行。
fn read(work: &Path, name: &str) -> String {
    std::fs::read_to_string(work.join(name))
        .unwrap_or_else(|error| panic!("{name}：{error}"))
        .trim_end()
        .to_string()
}

/// 会话日志里的事件，照先后。
fn log(home: &Home, session: &str) -> Vec<Value> {
    let dir = home.root.path().join("home");
    let mut events = Vec::new();
    for owner in std::fs::read_dir(&dir).expect("有家目录").flatten() {
        let sessions = owner.path().join("sessions").join(session);
        let Ok(segments) = std::fs::read_dir(&sessions) else {
            continue;
        };
        let mut files: Vec<_> = segments
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "jsonl"))
            .collect();
        files.sort();
        for file in files {
            for line in std::fs::read_to_string(file).expect("读得出").lines() {
                events.push(serde_json::from_str(line).expect("一行一条 JSON"));
            }
        }
    }
    events
}

/// 拉起核心、开一个会话（权限全开：这里测通道，沙盒在 `miyu-sandbox` 测）、说一句；模型照 `args` 调 `shell`。交回核心、会话
/// 编号，和留着的假模型（丢掉就停了）。
async fn run(home: &Home, work: &Path, args: &Value) -> (Child, String, Server) {
    let server = Server::start(replies(args)).await;
    let core = start(home, &server);
    let mut rpc = connect(home).await;
    let created = rpc
        .call(
            "c1",
            "session.create",
            json!({"cwd": work.to_string_lossy()}),
        )
        .await;
    let session = created["session"].as_str().expect("有编号").to_string();
    rpc.call(
        "c2",
        "session.set_permission_level",
        json!({"session": session, "level": "full"}),
    )
    .await;
    rpc.call(
        "c3",
        "session.send",
        json!({"session": session, "text": "数一下"}),
    )
    .await;
    (core, session, server)
}

/// 连上核心、握过手（说中文）。
async fn connect(home: &Home) -> Rpc {
    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let (reader, write) = tokio::io::split(connection);
    let mut rpc = Rpc {
        read: AsyncReader::new(reader),
        write,
    };
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
        "caps": {"input": false}, "token": token, "locale": "zh-CN"});
    rpc.call("hello-1", "hello", hello).await;
    rpc
}

/// 等到工作目录里有了 `done.flag`，最多一分钟；没等到的说 `shell` 交回了什么。
async fn until_done(home: &Home, session: &str, work: &Path) {
    let started = Instant::now();
    while !work.join("done.flag").exists() && started.elapsed() < Duration::from_secs(60) {
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    assert!(
        work.join("done.flag").exists(),
        "脚本没跑完：{:#?}\n{}",
        log(home, session)
            .iter()
            .filter(|event| event["kind"] == "tool.result")
            .collect::<Vec<_>>(),
        home.core_log()
    );
}

/// 等到日志里有了 `kind` 那一条，最多一分钟；交回这时的日志。
async fn until_logged(home: &Home, session: &str, kind: &str) -> Vec<Value> {
    let started = Instant::now();
    loop {
        let events = log(home, session);
        if events.iter().any(|event| event["kind"] == kind)
            || started.elapsed() > Duration::from_secs(60)
        {
            return events;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn a_script_calls_this_sessions_tools_through_its_own_endpoint() {
    let home = Home::new();
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let args = json!({"command": script(&work), "description": "脚本调工具"});
    let (mut core, session, _server) = run(&home, &work, &args).await;
    until_done(&home, &session, &work).await;

    // 能调的：一行 JSON，退出码 0；子 shell 里再起一层照样。
    assert_eq!(
        read(&work, "found.code"),
        "0",
        "{}",
        read(&work, "found.err")
    );
    let found: Value = serde_json::from_str(&read(&work, "found.out")).expect("一行 JSON");
    assert!(found.get("memories").is_some(), "{found}");
    assert_eq!(
        read(&work, "child.code"),
        "0",
        "{}",
        read(&work, "child.err")
    );
    // 调不了的、不认识的：退出码 2，说怎么改。
    assert_eq!(read(&work, "itself.code"), "2");
    assert!(
        read(&work, "itself.err")
            .starts_with("miyu call does not work for shell; call it directly."),
        "{}",
        read(&work, "itself.err")
    );
    assert_eq!(read(&work, "typo.code"), "2");
    assert_eq!(
        read(&work, "typo.err"),
        "No tool named memory_serch. Did you mean memory_search?"
    );
    assert_eq!(read(&work, "schema.code"), "0");
    assert!(
        read(&work, "schema.out").starts_with("memory_search: "),
        "{}",
        read(&work, "schema.out")
    );
    if cfg!(target_os = "linux") {
        assert_eq!(read(&work, "away.code"), "2", "离开了进程组的调不成");
        assert_eq!(
            read(&work, "away.err"),
            "miyu call only works from processes started by the command Miyu ran."
        );
    }

    // 命令结束了：端点关了，同一个套接字连不上，退出码 3。
    let events = until_logged(&home, &session, "turn.ended").await;
    let endpoint = read(&work, "endpoint.txt");
    let (sock, call_token) = endpoint.split_once('\n').expect("两行");
    let after = std::process::Command::new(MIYU)
        .args(["call", "memory_search", "--query", "late"])
        .env("MIYU_CALL_SOCK", sock)
        .env("MIYU_CALL_TOKEN", call_token)
        .output()
        .expect("跑得起来");
    assert_eq!(after.status.code(), Some(3));
    assert_eq!(
        String::from_utf8_lossy(&after.stderr),
        "The command this call belongs to has ended.\n"
    );
    // 自己的终端里：没有那两个变量。
    let outside = std::process::Command::new(MIYU)
        .args(["call", "memory_search", "--query", "x"])
        .env_remove("MIYU_CALL_SOCK")
        .env_remove("MIYU_CALL_TOKEN")
        .output()
        .expect("跑得起来");
    assert_eq!(outside.status.code(), Some(2));

    // 日志：调成了的、拒了的都照内核记（不认识的、调不了的不记），挂在那次 shell 下面，排在它的结果前面。
    let result = events
        .iter()
        .position(|event| event["kind"] == "tool.result")
        .expect("shell 有结果");
    let nested: Vec<&Value> = events
        .iter()
        .filter(|event| event["kind"] == "tool.nested")
        .collect();
    let call = events[result]["body"]["call_id"].as_str().expect("有编号");
    assert!(!nested.is_empty(), "记了嵌套调用");
    assert!(
        nested.iter().all(|event| {
            event["body"]["id"]
                .as_str()
                .is_some_and(|id| id.starts_with(&format!("{call}#")))
                && event["body"]["name"] == "memory_search"
                && event["body"]["status"] == "ok"
        }),
        "{nested:#?}"
    );
    let last = events
        .iter()
        .rposition(|event| event["kind"] == "tool.nested")
        .expect("有");
    assert!(last < result, "都在 shell 的结果前面");
    // 头上看得到（施工 M-5 上）：那一步带个数和最近一个，点开是整份。
    let mut rpc = connect(&home).await;
    let detail = rpc
        .call(
            "d",
            "view.detail",
            json!({"session": session, "call": call}),
        )
        .await;
    let listed = detail["nested"].as_array().expect("有整份");
    assert_eq!(listed.len(), nested.len(), "{detail}");
    assert!(
        listed
            .iter()
            .all(|item| item["state"] == "ok" && item["title"]["name"] == "回忆"),
        "{detail}"
    );
    let page = rpc
        .call("p", "view.page", json!({"session": session, "view": true}))
        .await;
    let step = page["entries"]
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["call"] == call))
        .expect("有那一步");
    assert_eq!(step["nested"]["count"], nested.len(), "{step}");

    core.kill().expect("杀得了核心");
    core.wait().expect("收得了");
}

/// 放到后台的命令：脚本照样调，挂在任务下面（`j1#1`）、不带回合；任务的报告排在它后面。
#[tokio::test]
async fn a_background_command_calls_under_its_job() {
    let home = Home::new();
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let command = format!(
        "{}{}{}",
        enter(&work),
        step("bg", "memory_search --query bg"),
        finish()
    );
    let args = json!({"command": command, "description": "后台", "run_in_background": true});
    let (mut core, session, _server) = run(&home, &work, &args).await;
    until_done(&home, &session, &work).await;
    assert_eq!(read(&work, "bg.code"), "0", "{}", read(&work, "bg.err"));
    let events = until_logged(&home, &session, "job.reported").await;
    let nested: Vec<&Value> = events
        .iter()
        .filter(|event| event["kind"] == "tool.nested")
        .collect();
    assert_eq!(nested.len(), 1, "{nested:#?}");
    assert_eq!(nested[0]["body"]["id"], "j1#1");
    assert!(nested[0].get("turn").is_none(), "后台的不带回合");
    let reported = events
        .iter()
        .position(|event| event["kind"] == "job.reported")
        .expect("任务报了结束");
    let at = events
        .iter()
        .position(|event| event["kind"] == "tool.nested")
        .expect("有");
    assert!(at < reported);
    // 头上看得到（施工 M-5 上）：任务了结那条旁白带最后那一份，点开照任务要整份。
    let mut rpc = connect(&home).await;
    let detail = rpc
        .call("d", "view.detail", json!({"session": session, "job": "j1"}))
        .await;
    assert_eq!(detail["nested"][0]["n"], 1, "{detail}");
    assert_eq!(detail["nested"][0]["name"], "memory_search");
    let page = rpc
        .call("p", "view.page", json!({"session": session, "view": true}))
        .await;
    let notice = page["entries"]
        .as_array()
        .and_then(|entries| entries.iter().find(|entry| entry["what"] == "job"))
        .expect("有任务的旁白");
    assert_eq!(notice["nested"]["count"], 1, "{notice}");
    core.kill().expect("杀得了核心");
    core.wait().expect("收得了");
}

/// 到时挪到后台的命令（施工 M-3 拍板的第 1 条）：通道跟着走，挪过去以后的调用挂到那个任务下面。
#[tokio::test]
async fn a_command_moved_to_the_background_keeps_calling() {
    let home = Home::new();
    let work = home.dir.join("work");
    std::fs::create_dir_all(&work).expect("建得了");
    let command = format!(
        "{}{}{}{}",
        enter(&work),
        pause(),
        step("late", "memory_search --query late"),
        finish()
    );
    let args = json!({"command": command, "description": "挪过去", "timeout": 1000});
    let (mut core, session, _server) = run(&home, &work, &args).await;
    until_done(&home, &session, &work).await;
    assert_eq!(read(&work, "late.code"), "0", "{}", read(&work, "late.err"));
    let events = until_logged(&home, &session, "job.reported").await;
    let ids: Vec<&str> = events
        .iter()
        .filter(|event| event["kind"] == "tool.nested")
        .filter_map(|event| event["body"]["id"].as_str())
        .collect();
    assert_eq!(ids, ["j1#1"], "挪过去以后的挂在任务下面");
    core.kill().expect("杀得了核心");
    core.wait().expect("收得了");
}
