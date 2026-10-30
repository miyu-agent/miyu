//! 真跑 `miyu ask`，标准输入接管道、伪终端（施工 AP-2，设计 50 第二、三节）：没写消息的读到头；消息里有 `-` 的读标准输入
//! 接在后面，空一行隔开；写了消息、没写 `-` 的不读，留着一个不关的管道也不卡住；不是 UTF-8 的、空的、终端里没写消息的照
//! 参数写错报，退出码 2；新会话的几样和 `-c`、`-s` 一起写的也是。配的模型用不了（[`UNUSABLE_MODEL`]）：会话照常造、话照常
//! 记下，这一轮说「没有可用的模型」，退出码 5；记下的话从会话日志里读。

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::support::{Home, MIYU, UNUSABLE_MODEL, offline, resources, within};
use miyu_ipc::connect_or_start;
use miyu_kernel::block::Block;
use miyu_kernel::event::Body;
use miyu_store::log::read_events;

/// 在 `root` 上跑 `miyu ask <args>` 的命令：中文。
fn ask(root: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(MIYU);
    command
        .arg("ask")
        .args(args)
        .env("MIYU_HOME", root)
        .envs(offline(root))
        .env("MIYU_RESOURCES", resources())
        .env("LANG", "zh_CN.UTF-8")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR");
    command
}

/// 跑 `command`，标准输入写进 `input`；`close` 的写完关上，不关的一直开着到它退出。标准输出扔掉，标准错误接到 `dir` 里的文件：
/// 不用管道，拉起的核心继承了也不卡。最多等一分钟，到点没退出的杀掉、报出来。交回退出码和标准错误。
fn run(dir: &Path, mut command: Command, input: &[u8], close: bool) -> (Option<i32>, String) {
    let err = dir.join(format!("err-{}.txt", stamp()));
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&err).expect("建得了"))
        .spawn()
        .expect("跑得起来");
    let mut stdin = child.stdin.take().expect("有标准输入");
    // 不读标准输入就退出的，写不进去也不要紧。
    let _written = stdin.write_all(input);
    let held = (!close).then_some(stdin);
    let code = finish(&mut child);
    drop(held);
    (code, std::fs::read_to_string(&err).unwrap_or_default())
}

/// 等 `child` 退出，最多一分钟：到点没退出的杀掉，当它在等标准输入。
fn finish(child: &mut std::process::Child) -> Option<i32> {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if let Some(status) = child.try_wait().expect("看得了") {
            return status.code();
        }
        if Instant::now() > deadline {
            let _killed = child.kill();
            let _waited = child.wait();
            panic!("一分钟没退出：在等标准输入");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// 文件名的尾巴：纳秒时刻。
fn stamp() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos())
}

/// 起好核心（配的模型用不了），在它上面跑 `body`，跑完等核心走。
async fn with_core(body: impl FnOnce(&Home) + Send + 'static) {
    let home = Home::new();
    home.system_config(UNUSABLE_MODEL);
    let (held, _) = within("拉起", connect_or_start(&home.root, || home.core()))
        .await
        .expect("拉得起");
    let home = tokio::task::spawn_blocking(move || {
        body(&home);
        home
    })
    .await
    .expect("没 panic");
    drop(held);
    home.until_stopped().await;
}

/// 数据根里唯一一个会话里记下的人说的话，照先后。
fn said(home: &Home) -> Vec<String> {
    let sessions = home.root.sessions().expect("列得出");
    assert_eq!(sessions.len(), 1, "{sessions:?}");
    read_events(&home.root.session_dir(&sessions[0]))
        .expect("读得到")
        .into_iter()
        .filter_map(|event| match event.body {
            Body::MessageUser(message) => Some(message.blocks),
            _ => None,
        })
        .map(|blocks| {
            blocks
                .into_iter()
                .filter_map(|block| match block {
                    Block::Text(text) => Some(text.text),
                    _ => None,
                })
                .collect::<String>()
        })
        .collect()
}

#[tokio::test]
async fn a_piped_message_without_words_is_the_message() {
    with_core(|home| {
        let (code, err) = run(
            &home.dir,
            ask(home.root.path(), &[]),
            "在吗\n".as_bytes(),
            true,
        );
        assert_eq!(code, Some(5), "{err}");
        assert_eq!(said(home), ["在吗\n"]);
    })
    .await;
}

#[tokio::test]
async fn a_dash_puts_the_pipe_after_the_words() {
    with_core(|home| {
        let (code, err) = run(
            &home.dir,
            ask(home.root.path(), &["为什么", "-", "失败"]),
            b"error: x\n",
            true,
        );
        assert_eq!(code, Some(5), "{err}");
        assert_eq!(said(home), ["为什么 失败\n\nerror: x\n"]);
    })
    .await;
}

#[tokio::test]
async fn words_without_a_dash_leave_an_open_pipe_alone() {
    // 别的程序拉起 `miyu` 时留着一个不关的管道：写了消息、没写 `-` 的不读，不会一直等下去；写进管道的也不算进消息。
    with_core(|home| {
        let (code, err) = run(
            &home.dir,
            ask(home.root.path(), &["在吗"]),
            b"not this",
            false,
        );
        assert_eq!(code, Some(5), "{err}");
        assert_eq!(said(home), ["在吗"]);
    })
    .await;
}

#[test]
fn stdin_that_is_not_utf8_is_exit_code_2() {
    let home = Home::new();
    for args in [&[][..], &["看看", "-"]] {
        let (code, err) = run(&home.dir, ask(home.root.path(), args), &[0xc3, 0x28], true);
        assert_eq!(code, Some(2), "{args:?}：{err}");
        assert_eq!(err, "miyu: 标准输入不是 UTF-8 文本\n", "{args:?}");
    }
}

#[test]
fn empty_stdin_is_a_missing_message() {
    let home = Home::new();
    for (args, input) in [
        (&[][..], &b""[..]),
        (&[], b" \n\t\n"),
        (&["-"], b""),
        (&[""], b""),
    ] {
        let (code, err) = run(&home.dir, ask(home.root.path(), args), input, true);
        assert_eq!(code, Some(2), "{args:?} {input:?}：{err}");
        assert_eq!(
            err, "miyu: 缺少消息\n提示：用法：miyu ask \"<消息>\"\n",
            "{args:?} {input:?}"
        );
    }
}

#[test]
fn new_session_options_with_a_continued_session_are_exit_code_2() {
    let home = Home::new();
    let cases: [(&[&str], &str); 7] = [
        (
            &["--persona", "engineer", "-c", "在吗"],
            "miyu: '--persona' 不能和 '--continue' 同时使用\n",
        ),
        (
            &["--no-persona", "-s", "x", "在吗"],
            "miyu: '--no-persona' 不能和 '--session' 同时使用\n",
        ),
        (
            &["--preset", "full", "-c", "在吗"],
            "miyu: '--preset' 不能和 '--continue' 同时使用\n",
        ),
        (
            &["--permission", "full", "-c", "在吗"],
            "miyu: '--permission' 不能和 '--continue' 同时使用\n",
        ),
        (
            &["-C", ".", "-c", "在吗"],
            "miyu: '--cd' 不能和 '--continue' 同时使用\n",
        ),
        (
            &["--persona", "engineer", "--no-persona", "在吗"],
            "miyu: '--persona' 不能和 '--no-persona' 同时使用\n",
        ),
        (
            &["--permission", "root", "在吗"],
            "miyu: '--permission' 的值无效：'root'\n提示：可选的值：read-only、workspace 或 full\n",
        ),
    ];
    for (args, expected) in cases {
        let (code, err) = run(&home.dir, ask(home.root.path(), args), b"", true);
        assert_eq!(code, Some(2), "{args:?}：{err}");
        assert_eq!(err, expected, "{args:?}");
    }
}

/// 标准输入是终端、又没写消息：照参数写错报，不等人敲（施工 AP-2）。只在 Unix 上有伪终端。
#[cfg(unix)]
#[test]
fn a_terminal_without_words_is_a_missing_message() {
    use crate::support::pty::{pty, secondary};
    let home = Home::new();
    let (_main, slave) = pty();
    let err = home.dir.join("tty-err.txt");
    let mut child = ask(home.root.path(), &[])
        .stdin(secondary(&slave))
        .stdout(Stdio::null())
        .stderr(std::fs::File::create(&err).expect("建得了"))
        .spawn()
        .expect("跑得起来");
    let code = finish(&mut child);
    let said = std::fs::read_to_string(&err).unwrap_or_default();
    assert_eq!(code, Some(2), "{said}");
    assert_eq!(said, "miyu: 缺少消息\n提示：用法：miyu ask \"<消息>\"\n");
}
