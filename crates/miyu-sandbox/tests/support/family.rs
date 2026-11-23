//! 生命线的两份测试（`lifeline.rs`、`lifeline_kept.rs`）共用的（Unix）：在一个组里起一条带孙进程的命令，问进程还在不在，
//! 收拾测试自己起的。

use std::io::{BufRead, BufReader, Lines};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, ChildStdout, Command, Stdio};
use std::time::{Duration, Instant};

/// 进程 `pid` 还在不在：`kill -0` 问一声（只问测试自己起的）。没收掉的僵尸进程也算在。
pub fn alive(pid: &str) -> bool {
    Command::new("/bin/sh")
        .args(["-c", &format!("kill -0 {pid} 2>/dev/null")])
        .status()
        .is_ok_and(|status| status.success())
}

/// 等 `pid` 没了，最多 5 秒；交回没没。
pub fn gone(pid: &str) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    while alive(pid) && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    !alive(pid)
}

/// 进程 `pid` 是不是僵尸进程（没了、还没被收掉）。
pub fn zombie(pid: &str) -> bool {
    let state = Command::new("ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .expect("问得了");
    String::from_utf8_lossy(&state.stdout)
        .trim()
        .starts_with('Z')
}

/// 进程 `pid` 的进程组号。
pub fn group_of(pid: &str) -> String {
    let out = Command::new("ps")
        .args(["-o", "pgid=", "-p", pid])
        .output()
        .expect("问得了");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// 收拾测试自己起的、没被收掉的进程（按进程号）。
pub fn kill(pid: &str) {
    #[expect(clippy::let_underscore_must_use, reason = "已经没了的杀不了也不要紧")]
    let _ = Command::new("kill").args(["-9", pid]).status();
}

/// 起在看门的组里的一条命令：它自己、它的进程号（`$$`）、它在后台起的孙进程的进程号，还有没读完的输出。
pub struct Running {
    pub command: Child,
    pub pid: String,
    pub grandchild: String,
    pub rest: Lines<BufReader<ChildStdout>>,
}

/// 印出自己和孙进程的那一段。
pub const FAMILY: &str = "echo $$; sleep 60 & echo $!; wait";

/// 起 `command`（最后一个参数是一段 shell，先印 `$$`、再印孙进程的进程号），读那两行。进哪个组由调的一方先设好。
pub fn run(command: &mut Command) -> Running {
    let mut command = command
        .stdout(Stdio::piped())
        .stdin(Stdio::null())
        .spawn()
        .expect("起得来");
    let mut lines = BufReader::new(command.stdout.take().expect("接了管道")).lines();
    let mut line = || lines.next().expect("还有一行").expect("读得到");
    let (pid, grandchild) = (line(), line());
    Running {
        command,
        pid,
        grandchild,
        rest: lines,
    }
}

/// 在组 `group` 里起 `program args…`（同 [`run`]）。
pub fn start(group: u32, program: &str, args: &[&str]) -> Running {
    let group = i32::try_from(group).expect("进程号放得下");
    run(Command::new(program).args(args).process_group(group))
}

/// 照 [`FAMILY`] 起的 `sh`，还没设组。
pub fn family_command() -> Command {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", FAMILY]);
    command
}

/// 还在照常跑：命令没退出，孙进程在。
pub fn running(running: &mut Running) -> bool {
    running.command.try_wait().expect("问得了").is_none() && alive(&running.grandchild)
}

/// 命令被 SIGKILL，孙进程也没了（等它退出，最多 5 秒等孙进程）；没了的孙进程以外，留下的照进程号收拾掉。
pub fn killed(running: &mut Running) -> bool {
    let status = running.command.wait().expect("等得到");
    let grandchild_gone = gone(&running.grandchild);
    if !grandchild_gone {
        kill(&running.grandchild);
    }
    status.signal() == Some(9) && grandchild_gone
}
