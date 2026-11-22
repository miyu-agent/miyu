//! 伪终端里跑着的一个程序（施工 LW-6）：排函数配着真的终端界面录的两段用它起终端界面（`order/head.rs`）。
//!
//! 照 `crates/miyu/tests/support/pty.rs` 的办法开伪终端：`rustix` 的 `openpt`、`grantpt`、`unlockpt`、`ptsname` 建一对，
//! 从端设好窗口大小（[`ROWS`] × [`COLUMNS`]），子进程的标准输入、输出、错误各开一份从端。这样不用 `unsafe`（工作区不许），
//! 子进程看到的三样都是终端、窗口大小也对。子进程没有控制终端：终端界面不靠它（按键、窗口大小、问终端都走标准输入输出），
//! 量尺也不发 `Ctrl+C` 这类要控制终端才变成信号的键。不借 util-linux 的 `script`：隔着它，模拟器的进程号不是子进程的，读
//! 装载起点（`/proc/<进程号>/maps`）还要往下找一层。
//!
//! 主端由一个线程一直读：读到的攒起来给 [`Terminal::wait_for`] 找字，同一块里问终端的几样当场回答（[`answer`]）。没人读的
//! 伪终端写满以后子进程就卡住了，所以等别的东西的时候也一直在读。

use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
use rustix::termios::{Winsize, tcsetwinsize};

use super::screen::{COLUMNS, ROWS, answer, shows, tail};

/// 等的时候隔多久看一次屏幕。
const POLL: Duration = Duration::from_millis(50);

/// 报错时附上屏幕上最后多少个字。
const TAIL: usize = 400;

/// 主端读到的，和回答终端时写不进去的那一次（写不进去以后不再读，终端界面也就等不到回答了）。
#[derive(Default)]
struct Seen {
    output: Vec<u8>,
    broken: Option<String>,
}

/// 伪终端里跑着的程序。丢掉时结束它。
pub struct Terminal {
    child: Child,
    input: File,
    seen: Arc<Mutex<Seen>>,
}

impl Terminal {
    /// 在一个新的伪终端里起 `command`（标准输入、输出、错误由这里接，调用方别设）。`command` 交进来以后丢掉：它手上的那几份
    /// 从端要关上，子进程退出时读的线程才读得到头。
    ///
    /// # Errors
    ///
    /// 开不了伪终端、设不了窗口大小、拉不起程序。
    pub fn spawn(mut command: Command) -> Result<Terminal, String> {
        let pty = |e: rustix::io::Errno| format!("开不了伪终端：{e}");
        let main = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).map_err(pty)?;
        grantpt(&main).map_err(pty)?;
        unlockpt(&main).map_err(pty)?;
        let name = PathBuf::from(OsString::from_vec(
            ptsname(&main, Vec::new()).map_err(pty)?.into_bytes(),
        ));
        let open = || {
            OpenOptions::new()
                .read(true)
                .write(true)
                .open(&name)
                .map_err(|e| format!("开不了从端 {}：{e}", name.display()))
        };
        let secondary = open()?;
        let size = Winsize {
            ws_row: ROWS,
            ws_col: COLUMNS,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        tcsetwinsize(&secondary, size).map_err(|e| format!("设不了窗口大小：{e}"))?;
        let child = command
            .stdin(Stdio::from(open()?))
            .stdout(Stdio::from(open()?))
            .stderr(Stdio::from(secondary))
            .spawn()
            .map_err(|e| format!("拉不起 {:?}：{e}", command.get_program()))?;
        drop(command);
        let main = File::from(main);
        let clone = |what: &str| {
            main.try_clone()
                .map_err(|e| format!("伪终端的主端复制不出{what}：{e}"))
        };
        let reader = clone("读的一份")?;
        let replies = clone("回答的一份")?;
        let seen = Arc::new(Mutex::new(Seen::default()));
        let shared = Arc::clone(&seen);
        std::thread::spawn(move || read(reader, replies, &shared));
        Ok(Terminal {
            child,
            input: main,
            seen,
        })
    }

    /// 子进程的进程号：放在模拟器里起的是模拟器的。
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// 现在攒到了多少字节：[`Terminal::wait_for`] 从这里往后找，前面画过的不算。
    pub fn mark(&self) -> usize {
        self.lock().output.len()
    }

    /// 往终端里打字（像人按键那样写进主端）。
    ///
    /// # Errors
    ///
    /// 写不进去。
    pub fn write(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.input
            .write_all(bytes)
            .map_err(|e| format!("往伪终端里写不进去：{e}"))
    }

    /// 等屏幕上（[`Terminal::mark`] 记下的 `from` 以后、去掉转义序列）出现 `needle`，最多等 `patience`。
    ///
    /// # Errors
    ///
    /// 等不到、程序先退出了、回答终端时写不进去：报在等什么（`what`）、屏幕上最后是什么。
    pub async fn wait_for(
        &mut self,
        needle: &str,
        from: usize,
        patience: Duration,
        what: &str,
    ) -> Result<(), String> {
        self.wait_for_any(&[needle], from, patience, what).await
    }

    /// 同 [`Terminal::wait_for`]，`needles` 里出现哪一个都算（施工 LW-14：回到对话时重画哪几行说不准）。
    ///
    /// # Errors
    ///
    /// 同 [`Terminal::wait_for`]。
    pub async fn wait_for_any(
        &mut self,
        needles: &[&str],
        from: usize,
        patience: Duration,
        what: &str,
    ) -> Result<(), String> {
        let began = Instant::now();
        loop {
            let (output, broken) = {
                let seen = self.lock();
                (
                    seen.output[from.min(seen.output.len())..].to_vec(),
                    seen.broken.clone(),
                )
            };
            if needles.iter().any(|needle| shows(&output, needle)) {
                return Ok(());
            }
            let problem = if let Some(broken) = broken {
                Some(broken)
            } else if let Some(status) = self
                .child
                .try_wait()
                .map_err(|e| format!("看不了终端里的程序还在不在：{e}"))?
            {
                Some(format!("终端里的程序先退出了（{status}）"))
            } else if began.elapsed() > patience {
                Some(format!("等了 {} 秒", patience.as_secs()))
            } else {
                None
            };
            if let Some(problem) = problem {
                return Err(format!(
                    "{what}：屏幕上没等到「{}」，{problem}。屏幕上最后是：{}",
                    needles.join("」或「"),
                    tail(&output, TAIL)
                ));
            }
            tokio::time::sleep(POLL).await;
        }
    }

    /// 等屏幕安静下来：`gap` 这么久没有新的输出就回，最多等 `cap`（一直在画的，例如转圈的记号，到点照样往下走）。
    pub async fn quiet(&self, gap: Duration, cap: Duration) {
        let began = Instant::now();
        let mut last = (self.mark(), Instant::now());
        while began.elapsed() < cap {
            tokio::time::sleep(POLL).await;
            let now = self.mark();
            if now != last.0 {
                last = (now, Instant::now());
            } else if last.1.elapsed() >= gap {
                return;
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Terminal {
    fn drop(&mut self) {
        #[expect(clippy::let_underscore_must_use, reason = "已经走了的结束不了，照样收")]
        let _ = self.child.kill();
        #[expect(
            clippy::let_underscore_must_use,
            reason = "收不到也只是留一个僵尸到量尺退出"
        )]
        let _ = self.child.wait();
    }
}

/// 读主端直到从端都关了（Linux 上是 `EIO`，不是干净的读到头）：攒进 `seen`，问终端的当场答。
fn read(mut reader: File, mut replies: File, seen: &Mutex<Seen>) {
    let mut chunk = vec![0u8; 64 * 1024];
    while let Ok(n) = reader.read(&mut chunk) {
        if n == 0 {
            break;
        }
        seen.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .output
            .extend_from_slice(&chunk[..n]);
        // 写回答时不拿着锁：子进程一时不读，写会卡住，等的那一头还要看屏幕。
        let reply = answer(&chunk[..n]);
        if !reply.is_empty()
            && let Err(e) = replies.write_all(&reply)
        {
            seen.lock().unwrap_or_else(PoisonError::into_inner).broken =
                Some(format!("回答终端界面问的几样时写不进伪终端：{e}"));
            break;
        }
    }
}
