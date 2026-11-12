//! 核心没了，它拉起的长住的子进程跟着走（施工 XH-1，`docs/blueprint/core.md`「子进程随核心退出」）：真的 `miyu core` 拉起
//! 扩展（`miyu-test-extension`，三个平台）、命令型 MCP 服务（`miyu-test-mcp`，Unix）、语义模型小程序（`miyu-test-embed`，
//! Unix），各自再起一个睡着的子进程（核心的孙进程），记下两个进程号；硬杀核心（Unix 上 `SIGKILL`，Windows 上结束进程，不连
//! 子进程），两个都要在几秒内没了：Unix 上组里看门的杀了整组，Windows 上核心所在的作业对象跟着关。
//!
//! 扩展的那一个一起来就 `hang`（不管标准输入）：靠自己读到标准输入关了退出的办法管不到它。MCP 服务（写在系统配置里）、语义
//! 模型小程序（包 `bin/` 里的程序）照一段 `sh` 起（先在后台起 `sleep`、记下进程号，再 `exec` 成真的程序），真的程序读到标准
//! 输入关了会自己退，它起的 `sleep` 不会：修之前留下来的就是它。只在 Unix 上测这两种：Windows 上三种进程都是照继承进的同一个
//! 作业对象，扩展那一条测到了。
//!
//! 三个测试程序要在 `miyu` 旁边（`cargo test --workspace` 会编它们）。没停下的（修之前的样子），测试最后照记下的进程号收拾掉。

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::support::{Home, MIYU};

/// `miyu` 旁边的测试程序 `name`：不在的说怎么编。
fn beside(name: &str) -> PathBuf {
    let path = Path::new(MIYU).with_file_name(format!("{name}{}", std::env::consts::EXE_SUFFIX));
    assert!(
        path.is_file(),
        "{} 不在：cargo test --workspace 会编它",
        path.display()
    );
    path
}

/// 拉起核心（不空闲退出），等它写来 `ready`。
fn start(home: &Home) -> Child {
    let mut core = home
        .core_staying()
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

/// 记下的两个进程号：长住的那一个、它起的睡着的那一个。丢掉时还在的照进程号收拾掉（只杀测试自己起的）。
struct Pids([String; 2]);

impl Drop for Pids {
    fn drop(&mut self) {
        for pid in &self.0 {
            if alive(pid) {
                #[cfg(unix)]
                let killed = Command::new("kill").args(["-9", pid]).status();
                #[cfg(windows)]
                let killed = Command::new("taskkill").args(["/F", "/PID", pid]).status();
                #[expect(
                    clippy::let_underscore_must_use,
                    reason = "收拾不掉也只是留下一个睡着的"
                )]
                let _ = killed;
            }
        }
    }
}

/// 等 `path` 写好两行进程号（最多 60 秒）。
async fn pids(home: &Home, path: &Path) -> Pids {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let lines: Vec<&str> = text.lines().map(str::trim).collect();
        if let [one, two] = lines.as_slice() {
            return Pids([(*one).to_string(), (*two).to_string()]);
        }
        assert!(Instant::now() < deadline, "没起来：{}", home.core_log());
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// 进程 `pid` 还在不在：Unix 上 `kill -0`；Windows 上 `tasklist` 照进程号找。
fn alive(pid: &str) -> bool {
    #[cfg(unix)]
    return Command::new("kill")
        .args(["-0", pid])
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success());
    #[cfg(windows)]
    {
        let filter = format!("PID eq {pid}");
        let out = Command::new("tasklist")
            .args(["/FI", &filter, "/NH", "/FO", "CSV"])
            .output()
            .expect("问得了");
        String::from_utf8_lossy(&out.stdout).contains(&format!("\"{pid}\""))
    }
}

/// 两个都没了（最多等 10 秒）。
fn gone(pids: &Pids) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while pids.0.iter().any(|pid| alive(pid)) {
        if Instant::now() > deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

/// 硬杀核心以前两个都在、各自的组：Unix 上两个在同一组、组长不是它们（是看门的，施工 XH-1），也不是核心的组。不对的交回哪里
/// 不对（先杀了核心再报：核心活着的话会把收拾掉的扩展再拉起来，留下新的）。
#[cfg_attr(not(unix), expect(unused_variables, reason = "Windows 上不看组"))]
fn check_before(core: &Child, pids: &Pids) -> Result<(), String> {
    if let Some(pid) = pids.0.iter().find(|pid| !alive(pid)) {
        return Err(format!("{pid} 没起来"));
    }
    #[cfg(unix)]
    {
        let group = |pid: &str| {
            let out = Command::new("ps")
                .args(["-o", "pgid=", "-p", pid])
                .output()
                .expect("问得了");
            String::from_utf8_lossy(&out.stdout).trim().to_string()
        };
        let (one, two) = (group(&pids.0[0]), group(&pids.0[1]));
        if one != two {
            return Err(format!("子进程不在它的组里：{one}、{two}"));
        }
        if pids.0.contains(&one) || one == group(&core.id().to_string()) {
            return Err(format!("组 {one} 是它自己的或者核心的"));
        }
    }
    Ok(())
}

/// 硬杀核心，两个都要没了。
fn kill_and_check(mut core: Child, pids: &Pids, what: &str) {
    let before = check_before(&core, pids);
    core.kill().expect("杀得了核心");
    core.wait().expect("收得了");
    if let Err(problem) = before {
        panic!("{what}：{problem}");
    }
    let left: Vec<&String> = match gone(pids) {
        true => Vec::new(),
        false => pids.0.iter().filter(|pid| alive(pid)).collect(),
    };
    assert!(
        left.is_empty(),
        "核心没了，{what}（{:?}）还剩 {left:?}",
        pids.0
    );
}

#[tokio::test]
async fn an_extension_and_its_child_die_with_a_killed_core() {
    let home = Home::new();
    beside("miyu-test-extension");
    let record = home.dir.join("extension.pids");
    let package = home.root.packages().join("xh-kept");
    std::fs::create_dir_all(&package).expect("建得了");
    let manifest = format!(
        "[package]\nprotocol = [1, 1]\nname = {{ en = \"Kept\" }}\n\n[command]\nname = \"xh-kept\"\nprogram = \"miyu-test-extension\"\nabout = {{ en = \"K\" }}\n\n[process]\nargs = [{:?}, \"hang\"]\nstart = \"always\"\n",
        format!("child:{}", record.display())
    );
    std::fs::write(package.join("package.toml"), manifest).expect("写得进");
    let core = start(&home);
    let pids = pids(&home, &record).await;
    kill_and_check(core, &pids, "扩展");
}

/// 一段 `sh`：后台起一个 `sleep`（三个标准流接空的，不拿着核心的管道），`$$`、`$!` 写进 `record`，再 `exec` 成 `program`。
#[cfg(unix)]
fn wrapper(record: &Path, program: &Path) -> String {
    let (record, program) = (record.display(), program.display());
    format!(
        "sleep 600 </dev/null >/dev/null 2>&1 & printf '%s\\n%s\\n' $$ $! > '{record}.part' && mv '{record}.part' '{record}'; exec '{program}' \"$@\""
    )
}

#[cfg(unix)]
#[tokio::test]
async fn an_mcp_server_and_its_child_die_with_a_killed_core() {
    let home = Home::new();
    let record = home.dir.join("mcp.pids");
    let script = wrapper(&record, &beside("miyu-test-mcp"));
    home.system_config(&format!(
        "[mcp.kept]\ncommand = [\"/bin/sh\", \"-c\", {script:?}]\ntimeout_ms = 1000\n"
    ));
    let core = start(&home);
    let pids = pids(&home, &record).await;
    kill_and_check(core, &pids, "MCP 服务");
}

/// 语义模型小程序：装一个 `embed` 小程序包（人格记忆出厂推荐它），程序是包 `bin/` 里的一段 `sh`，模型照会话的测试用的
/// 手造小模型。往无人格那一间记一条记忆，核心在后台补向量时拉起它。
#[cfg(unix)]
#[tokio::test]
async fn an_embedder_and_its_child_die_with_a_killed_core() {
    use std::os::unix::fs::PermissionsExt;

    use serde_json::json;

    use crate::crash::Rpc;
    use crate::support::within;

    let home = Home::new();
    let record = home.dir.join("embed.pids");
    let package = home.root.packages().join("embed");
    let bin = package.join("bin");
    std::fs::create_dir_all(&bin).expect("建得了");
    std::fs::write(
        package.join("package.toml"),
        "[package]\nprotocol = [1, 1]\nname = { en = \"Kept model\" }\n\n[worker]\nprogram = \"kept-embed\"\n",
    )
    .expect("写得进");
    let script = bin.join("kept-embed");
    let body = wrapper(&record, &beside("miyu-test-embed"));
    std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).expect("写得进");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).expect("改得了");
    let tiny =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../miyu-session/tests/fixtures/embed-tiny");
    for (from, to) in [
        ("manifest.toml", "model.toml"),
        ("model.onnx", "model.onnx"),
        ("vocab.txt", "vocab.txt"),
    ] {
        std::fs::copy(tiny.join(from), package.join(to)).expect("拷得了");
    }
    let core = start(&home);
    let (connection, token) = within("连上", miyu_ipc::connect(&home.root))
        .await
        .expect("连得上");
    let mut rpc = Rpc::over(connection);
    let hello = json!({"protocol": [1, 1], "head": {"kind": "test", "version": "0.0.0"},
        "caps": {"input": false}, "token": token});
    rpc.call("hello-1", "hello", hello).await;
    rpc.call(
        "r1",
        "memory.remember",
        json!({"persona": "_none", "class": "user", "text": "我的猫"}),
    )
    .await;
    let pids = pids(&home, &record).await;
    kill_and_check(core, &pids, "语义模型小程序");
}
