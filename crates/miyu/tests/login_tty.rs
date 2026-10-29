//! `miyu provider login` 在真的伪终端里（施工 8-5 补，`docs/blueprint/cli/login.md`「怎么走」第 4 条）：贴 key 的时候按
//! `Ctrl+C`，进程以 130 退出、印了取消那一句，结束后终端设置和开始前一样（回显开着）；送正常的 key 加回车，读到的对，
//! 终端设置也照原样。只在 Unix 上跑：Windows 没有伪终端，`Guard`、控制台模式的读写写在一个单元测试里，在
//! `crates/miyu-cli/src/config/console/windows.rs`。
//!
//! `miyu setup` 贴 key 那一步取消的测试（配置文件、密钥文件都没动）不需要真的伪终端：那条逻辑不碰终端的设置，照
//! 剧本回的假终端在 `crates/miyu-cli/tests/setup.rs` 的 `cancelling_the_key_paste_writes_nothing` 测过了。

#![cfg(unix)]

use std::fs::OpenOptions;
use std::io::Write;
use std::process::Command;

use rustix::termios::{self, LocalModes};

use crate::support::pty::{Capture, pty, secondary};
use crate::support::{Home, MIYU, offline, resources, within};
use miyu_ipc::connect_or_start;

/// 一眼看得出是假的 key。
const FAKE: &str = "sk-FAKE-KEY-FOR-TESTS-0001";

/// 这一对现在的回显、信号键、行缓冲是不是都开着（伪终端刚建好、谁都没按过 `miyu provider login` 之前的样子）。照从端的路径
/// 另开一份短命的文件描述符来问：Linux 上主端、从端的 termios 是同一份，问哪一头都一样；macOS（BSD 的 pty）主端不认
/// `tcgetattr` 这个 ioctl（`ENOTTY`，CI 的 macOS 上跑出来的，2026-10-02），只有从端认。
fn echoing(slave: &str) -> bool {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(slave)
        .expect("开得起从端");
    let modes = termios::tcgetattr(&file).expect("读得到").local_modes;
    modes.contains(LocalModes::ECHO)
        && modes.contains(LocalModes::ISIG)
        && modes.contains(LocalModes::ICANON)
}

/// 起核心，交回数据根；核心起来以后再建伪终端，免得核心自己的日志混进屏幕（施工 8-5 补）。
async fn core() -> Home {
    let home = Home::new();
    let core = || {
        let mut core = home.core();
        core.env("MIYU_LOG", "trace");
        core
    };
    let (_held, _) = within("拉起", connect_or_start(&home.root, core))
        .await
        .expect("拉得起");
    home
}

/// 在真的伪终端上跑一次 `miyu login deepseek`：标准输入、输出、错误都是从端。
fn spawn(root: &std::path::Path, slave: &str) -> std::process::Child {
    Command::new(MIYU)
        .args(["provider", "login", "deepseek"])
        .env("MIYU_HOME", root)
        .envs(offline(root))
        .env("MIYU_RESOURCES", resources())
        .env("LANG", "zh_CN.UTF-8")
        .env("NO_COLOR", "1")
        .env_remove("LC_ALL")
        .env_remove("LC_MESSAGES")
        .env_remove("XDG_RUNTIME_DIR")
        .stdin(secondary(slave))
        .stdout(secondary(slave))
        .stderr(secondary(slave))
        .spawn()
        .expect("跑得起来")
}

/// 密钥文件现在的字。
fn secrets(home: &Home) -> String {
    std::fs::read_to_string(home.root.path().join("secrets.toml")).unwrap_or_default()
}

#[tokio::test]
async fn ctrl_c_while_pasting_cancels_exits_130_and_restores_echo() {
    let home = core().await;
    let (main, slave) = pty();
    assert!(echoing(&slave), "伪终端刚建好，回显该是开着的");

    let mut child = spawn(home.root.path(), &slave);
    let capture = Capture::start(&main);
    capture.wait_for("粘贴 deepseek 的密钥（不显示）：");

    // 贴了一截就按 Ctrl+C（0x03），不等按回车：构造 8-5 补要修的那个场景。
    (&main)
        .write_all(b"sk-FAKE-partial\x03")
        .expect("写得进主端");

    let (code, screen) = capture.finish(&mut child);
    assert_eq!(code, 130, "{screen}");
    assert!(screen.ends_with("已取消，未保存\n"), "{screen}");
    assert_eq!(secrets(&home), "", "密钥文件一个字都没写");
    assert!(
        echoing(&slave),
        "按了 Ctrl+C 以后回显也该开回来，不是停在关着的样子"
    );
}

#[tokio::test]
async fn a_normal_paste_with_enter_is_read_and_echo_stays_as_it_was() {
    let home = core().await;
    let (main, slave) = pty();
    assert!(echoing(&slave), "伪终端刚建好，回显该是开着的");

    let mut child = spawn(home.root.path(), &slave);
    let capture = Capture::start(&main);
    capture.wait_for("粘贴 deepseek 的密钥（不显示）：");

    (&main)
        .write_all(format!("{FAKE}\n").as_bytes())
        .expect("写得进主端");

    let (code, screen) = capture.finish(&mut child);
    assert_eq!(code, 0, "{screen}");
    assert!(screen.ends_with("已保存 deepseek 的密钥\n"), "{screen}");
    assert!(
        secrets(&home).contains(&format!("deepseek = \"{FAKE}\"")),
        "{}",
        secrets(&home)
    );
    assert!(echoing(&slave), "读完以后回显照原样开着");
}
