//! 一对真的伪终端（施工 8-5 补写在 `login_tty.rs` 里，施工 IN-3 挪出来给装包的进度条测试共用）：建一对、给子进程开从端、主端边写
//! 边收。只在 Unix 上有。

use std::fs::{File, OpenOptions};
use std::io::Read;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};

/// 一对真的伪终端：主端（测试自己留着写按键）和从端的路径（给子进程的标准输入、输出、错误各开一份；查 termios 也
/// 走从端，见 `login_tty.rs` 的 `echoing`）。
pub fn pty() -> (File, String) {
    let main = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("开得起伪终端");
    grantpt(&main).expect("放得开");
    unlockpt(&main).expect("解得开");
    let name = ptsname(&main, Vec::new()).expect("问得到名字");
    let name = name.to_str().expect("是 UTF-8").to_string();
    (File::from(main), name)
}

/// 再开一份从端：子进程的标准输入、输出、错误各自独立的一份文件描述符（和真的终端一样）。
pub fn secondary(path: &str) -> Stdio {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .expect("开得起从端");
    Stdio::from(file)
}

/// 主端边写边收：开一个专门的线程一直阻塞着读，不是等子进程退出以后再读一轮——从端的最后一份文件描述符关掉的
/// 瞬间，macOS（BSD 的 pty）会把这时候还没读走的字节直接丢掉，子进程退出前最后写的那一句因此收不全（CI 上
/// 实测到的，2026-10-02）。这个线程自己在从端全关、读到错误的时候结束，不用另外叫它停。
pub struct Capture {
    buffer: Arc<Mutex<Vec<u8>>>,
    reader: JoinHandle<()>,
}

impl Capture {
    /// 开始收：`main` 另外克隆一份文件描述符交给读的线程，原来这一份留给调用的人写按键。
    pub fn start(main: &File) -> Capture {
        let read_end = main.try_clone().expect("克隆得出读的那一份");
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let reader = {
            let buffer = Arc::clone(&buffer);
            thread::spawn(move || {
                let mut chunk = [0u8; 4096];
                loop {
                    match (&read_end).read(&mut chunk) {
                        Ok(0) => break,
                        Ok(n) => buffer
                            .lock()
                            .expect("锁没中毒")
                            .extend_from_slice(&chunk[..n]),
                        // 从端全关了：Linux 上这是 EIO，不是干净的 EOF；当读到头，没有更多字节了。
                        Err(_) => break,
                    }
                }
            })
        };
        Capture { buffer, reader }
    }

    /// 现在攒到的字，去掉伪终端自己加的 `\r`（默认开着 `ONLCR`，写出去的每个 `\n` 在从端变成 `\r\n`）。
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.buffer.lock().expect("锁没中毒")).replace('\r', "")
    }

    /// 一直等，直到攒到的字里有 `needle`：给子进程时间印出「粘贴 ... 的密钥（不显示）：」这一句，再送按键。最多
    /// 一分钟（CI 的机器可能慢），到点还没见到就说清楚现在攒到了什么。
    pub fn wait_for(&self, needle: &str) {
        let deadline = Instant::now() + Duration::from_secs(60);
        loop {
            let text = self.text();
            if text.contains(needle) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "一分钟内没等到「{needle}」，现在攒到的是：{text}"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    /// 子进程走完，交回它的退出码和收到的全部字。读的线程这时该已经自己结束了（从端的三份文件描述符都是子进程的，
    /// 它一退出、读的线程马上也会读到「从端全关了」收尾），等它一下确保真的收全了。
    pub fn finish(self, child: &mut std::process::Child) -> (i32, String) {
        // 等它退出至多一分钟：等不到的杀掉、连同屏幕上的字报出来，不让整套测试挂死在这里。
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let status = loop {
            if let Some(status) = child.try_wait().expect("看得了") {
                break status;
            }
            if std::time::Instant::now() > deadline {
                stop(child);
                let screen = String::from_utf8_lossy(&self.buffer.lock().expect("锁没中毒"))
                    .replace('\r', "");
                panic!("一分钟没退出，杀掉了。屏幕上：\n{screen}");
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        };
        let buffer = Arc::clone(&self.buffer);
        self.reader.join().expect("读的线程没 panic");
        let text = String::from_utf8_lossy(&buffer.lock().expect("锁没中毒")).replace('\r', "");
        (status.code().expect("Unix 上有退出码"), text)
    }
}

/// 杀掉、收走子进程：杀不掉、等不到的（已经自己退了）不要紧，调的一方照样报。
#[expect(
    clippy::let_underscore_must_use,
    reason = "只是收尾：杀不掉的照样往下报屏幕上的字"
)]
pub fn stop(child: &mut std::process::Child) {
    let _ = child.kill();
    let _ = child.wait();
}
