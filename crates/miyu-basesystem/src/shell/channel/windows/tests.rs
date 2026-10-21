//! Windows 的端点本身（施工 M-3 下）：真的命名管道。测试进程自己不在命令的作业对象里，所以从这里连上去的照「不是这条命令
//! 的进程」拒；能调的那一路在 `crates/miyu/tests/call.rs` 真跑（权限全开，不经沙盒）。经沙盒的那一路（MXC 的基础容器，施工
//! 5-9 改上）在这里真跑一条 PowerShell：只有收得住的机器（虚拟机的桌面会话）上真测，别处说一声跳过。

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::ClientOptions;

use miyu_kernel::event::NestedStatus;
use miyu_kernel::id::JobId;
use miyu_tool::{Calling, Nested, Schema, Settled};

use super::*;

/// 出厂的字。
fn texts() -> Arc<Texts> {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    Texts::load(&resources).expect("出厂的字读得出")
}

/// 调不到这里的假端口。
struct Port;

impl NestedPort for Port {
    fn call(&self, _name: String, _args: String) -> Calling<'_> {
        unreachable!("认人那一关就拒了")
    }

    fn adopt(&self, _job: JobId) {}

    fn callable(&self) -> Vec<String> {
        Vec::new()
    }

    fn schema(&self, _name: &str) -> Option<Schema> {
        None
    }
}

/// 开一个端点，记下一个别的进程的作业对象：测试进程自己不在里面。交回端点和那个进程（测完收掉）。
fn open() -> (Channel, std::process::Child) {
    let channel = Channel::open(Arc::new(Port), texts()).expect("开得了");
    let mut other = Command::new("cmd")
        .args(["/c", "ping -n 30 127.0.0.1 >NUL"])
        .spawn()
        .expect("起得来");
    match miyu_sandbox::lifeline::Job::for_child(&other) {
        Ok(job) => channel.binder().bind(job),
        Err(error) => {
            stop(&mut other);
            panic!("作业对象建不成：{error}");
        }
    }
    (channel, other)
}

/// 收掉那个别的进程：已经走了的不管。
#[expect(clippy::let_underscore_must_use, reason = "它可能已经自己走了")]
fn stop(other: &mut std::process::Child) {
    let _ = other.kill();
}

#[tokio::test]
async fn a_process_outside_the_commands_job_is_refused() {
    let (channel, mut other) = open();
    let mut pipe = ClientOptions::new().open(&channel.name).expect("连得上");
    let body =
        json!({"v": 1, "op": "call", "tool": "web_search", "args": "{}", "token": channel.token})
            .to_string();
    pipe.write_u32(u32::try_from(body.len()).unwrap())
        .await
        .unwrap();
    pipe.write_all(body.as_bytes()).await.unwrap();
    let len = pipe.read_u32().await.expect("答了");
    let mut reply = vec![0u8; len as usize];
    pipe.read_exact(&mut reply).await.unwrap();
    let reply: Value = serde_json::from_slice(&reply).unwrap();
    assert_eq!(reply["code"], 2);
    assert_eq!(
        reply["stderr"],
        "miyu call only works from processes started by the command Miyu ran.\n"
    );
    stop(&mut other);
}

#[tokio::test]
async fn an_oversized_frame_is_dropped_at_once() {
    let (channel, mut other) = open();
    let mut pipe = ClientOptions::new().open(&channel.name).expect("连得上");
    pipe.write_u32(serve::MAX_REQUEST + 1).await.unwrap();
    let mut byte = [0u8; 1];
    let read = tokio::time::timeout(Duration::from_secs(5), pipe.read(&mut byte))
        .await
        .expect("当场断开，不等后面的字");
    assert!(matches!(read, Ok(0) | Err(_)), "{read:?}");
    stop(&mut other);
}

#[tokio::test]
async fn dropping_it_removes_the_pipe() {
    let (channel, mut other) = open();
    let name = channel.name.clone();
    drop(channel);
    tokio::time::sleep(Duration::from_millis(100)).await;
    let opened = ClientOptions::new().open(&name);
    assert!(opened.is_err(), "管道没了");
    stop(&mut other);
}

#[tokio::test]
async fn the_command_gets_the_pipe_and_a_path_to_miyu() {
    let (channel, mut other) = open();
    let mut env = vec![(OsString::from("Path"), OsString::from(r"C:\Windows"))];
    channel.env(&mut env);
    let get = |name: &str| {
        env.iter()
            .find(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case(name))
            .map(|(_, value)| value.to_string_lossy().into_owned())
    };
    assert_eq!(get("MIYU_CALL_SOCK"), Some(channel.name.clone()));
    assert!(
        get("Path").unwrap().ends_with(r";C:\Windows"),
        "放 `miyu.exe` 的目录排在最前，名字照原样"
    );
    assert_eq!(
        env.iter()
            .filter(|(key, _)| key.to_string_lossy().eq_ignore_ascii_case("path"))
            .count(),
        1
    );
    stop(&mut other);
}

/// 调什么都答 `ok` 的假端口。
struct Answers;

impl NestedPort for Answers {
    fn call(&self, _name: String, _args: String) -> Calling<'_> {
        Box::pin(async {
            Nested::Settled(Settled {
                text: "ok".to_string(),
                ..Settled::without_result(NestedStatus::Ok, None)
            })
        })
    }

    fn adopt(&self, _job: JobId) {}

    fn callable(&self) -> Vec<String> {
        Vec::new()
    }

    fn schema(&self, _name: &str) -> Option<Schema> {
        None
    }
}

/// 一个用完就删的临时目录。
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("miyu-call-test-{}", serve::random_hex().unwrap()));
        std::fs::create_dir_all(&dir).expect("建得了");
        Scratch(dir)
    }
}

impl Drop for Scratch {
    #[expect(clippy::let_underscore_must_use, reason = "删不掉的留在临时目录里")]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// 沙盒里跑的脚本：先往规格外面写一个文件（写不成才是真关在容器里），再照协议连管道调一次（权限照 `miyu call` 要的那几样），
/// 印出回答；编了 `miyu.exe` 的，再经它真调一次。
const SANDBOXED: &str = r#"$ErrorActionPreference = 'Stop'
try { Set-Content -LiteralPath $env:PROBE_OUTSIDE -Value x; 'confined=False' } catch { 'confined=True' }
$rights = [System.IO.Pipes.PipeAccessRights]'ReadData, WriteData, ReadAttributes, Synchronize'
$p = New-Object System.IO.Pipes.NamedPipeClientStream('.', $env:MIYU_CALL_SOCK.Substring(9), $rights, 'None', 'None', 'None')
try { $p.Connect(5000) } catch { 'connect=' + $_.Exception.GetBaseException().Message; exit 3 }
$body = [Text.Encoding]::UTF8.GetBytes('{"v":1,"op":"call","tool":"web_search","args":"{}","token":"' + $env:MIYU_CALL_TOKEN + '"}')
$len = [BitConverter]::GetBytes([uint32]$body.Length); [Array]::Reverse($len)
$p.Write($len, 0, 4); $p.Write($body, 0, $body.Length); $p.Flush()
function Take($n) { $buf = New-Object byte[] $n; $got = 0; while ($got -lt $n) { $r = $p.Read($buf, $got, $n - $got); if ($r -le 0) { 'closed'; exit 4 }; $got += $r }; ,$buf }
$head = Take 4; [Array]::Reverse($head)
'reply=' + [Text.Encoding]::UTF8.GetString((Take ([BitConverter]::ToUInt32($head, 0))))
$p.Dispose()
if ($env:MIYU_EXE) { $o = & $env:MIYU_EXE call web_search --query x 2>&1; 'miyu=' + $LASTEXITCODE + ' ' + ($o -join ' ') }
"#;

/// 测试程序旁边编出来的程序 `name`（助手是 `cargo build -p miyu-sandbox`，`miyu.exe` 是 `cargo build -p miyu`）；没编的交回空。
fn built(name: &str) -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let built = exe.parent()?.parent()?.join(name);
    built.is_file().then_some(built)
}

#[tokio::test]
async fn a_command_in_the_sandbox_reaches_the_pipe() {
    let Some(helper) = built(miyu_sandbox::HELPER) else {
        eprintln!("助手没编，跳过：先 cargo build -p miyu-sandbox");
        return;
    };
    let channel = Channel::open(Arc::new(Answers), texts()).expect("开得了");
    let (work, outside) = (Scratch::new(), Scratch::new());
    let script = work.0.join("call.ps1");
    std::fs::write(&script, SANDBOXED).expect("写得进");
    let spec = miyu_sandbox::Spec {
        write: vec![work.0.clone()],
        hidden: Vec::new(),
        read: None,
        reveal: Vec::new(),
        mount: false,
        sockets: Vec::new(),
    };
    let sandboxed = channel.sandboxed(&Sandboxed {
        helper,
        spec,
        env: Vec::new(),
    });
    let args: Vec<OsString> = [
        "-NoProfile",
        "-NonInteractive",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ]
    .into_iter()
    .map(OsString::from)
    .chain([script.into_os_string()])
    .collect();
    let (program, argv) =
        miyu_sandbox::argv(&sandboxed, Path::new("powershell.exe"), &args).expect("拼得成");
    // 照执行器挑好的那样带上原来的 `Path`：通道的目录加在最前面。
    let mut env: Vec<(OsString, OsString)> = std::env::var_os("Path")
        .map(|path| (OsString::from("Path"), path))
        .into_iter()
        .collect();
    channel.env(&mut env);
    let miyu = built("miyu.exe");
    if let Some(miyu) = &miyu {
        env.push(("MIYU_EXE".into(), miyu.clone().into_os_string()));
    }
    let child = Command::new(program)
        .args(argv)
        .envs(env)
        .env("PROBE_OUTSIDE", outside.0.join("escaped.txt"))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("起得来");
    let job = miyu_sandbox::lifeline::Job::for_child(&child).expect("作业对象建得成");
    channel.binder().bind(job);
    let out = tokio::task::spawn_blocking(move || child.wait_with_output())
        .await
        .unwrap()
        .expect("等得了");
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    if out.status.code() == Some(i32::from(miyu_sandbox::EXIT_HELPER)) {
        assert!(stderr.contains("cannot confine"), "{stderr}");
        eprintln!("这台机器收不住命令，跳过：{stderr}");
        return;
    }
    assert!(
        stdout.contains("confined=True"),
        "真关在容器里：{stdout}{stderr}"
    );
    let reply = stdout
        .lines()
        .find_map(|line| line.strip_prefix("reply="))
        .unwrap_or_else(|| panic!("连得上、答了：{stdout}{stderr}"));
    let reply: Value = serde_json::from_str(reply).expect("是 JSON");
    assert_eq!(reply, json!({"code": 0, "stdout": "ok\n", "stderr": ""}));
    if miyu.is_some() {
        assert!(
            stdout.contains("miyu=0 ok"),
            "经 `miyu call` 也调得成：{stdout}{stderr}"
        );
    } else {
        eprintln!("miyu.exe 没编，只测了照协议连");
    }
}
