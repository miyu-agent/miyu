//! 边缘情况（2026-10-30 发布前测试，缺陷 E-3）：`read` 一页最多交 64 KiB（`read/lines.rs` 的 `page`），可读的时候整行整行
//! 地读进内存：`split(b'\n')` 把每一行（不管是不是在这一页里）整行放进一个 `Vec`，再 `from_utf8_lossy(..).into_owned()`
//! 拷一份；带 BOM 的 UTF-16 更是整份 `read_to_end`、再解成 `Vec<u16>`、再拼成 `String`（`utf16`）。一份几个 GB、没有换行的
//! 文本（压缩过的 JS、一行的 JSON、日志），或者一份几个 GB 的 UTF-16 文件，她 `read` 一下，核心就要分出文件大小一到几倍的
//! 内存，能把核心撑爆。
//!
//! 期望：读一页用的内存和文件多大无关（这里拿 64 MiB 的文件、放宽到 16 MiB 量）。修之前这条是红的。
//!
//! 量的是整个进程的峰值常驻内存（Linux 的 `/proc/self/status` 里的 `VmHWM`，量之前写 `/proc/self/clear_refs` 清零），所以
//! 自己一个测试程序（AGENTS.md「测试」），只有这一个测试，只在 Linux 上跑。
#![cfg(target_os = "linux")]

mod support;

use std::io::Write;

use support::Site;

/// 这时的峰值常驻内存，字节。
fn high_water() -> usize {
    let status = std::fs::read_to_string("/proc/self/status").expect("读得了");
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmHWM:"))
        .and_then(|rest| {
            rest.trim()
                .trim_end_matches("kB")
                .trim()
                .parse::<usize>()
                .ok()
        })
        .expect("有 VmHWM")
        * 1024
}

/// 这时的常驻内存，字节。
fn resident() -> usize {
    let status = std::fs::read_to_string("/proc/self/status").expect("读得了");
    status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .and_then(|rest| {
            rest.trim()
                .trim_end_matches("kB")
                .trim()
                .parse::<usize>()
                .ok()
        })
        .expect("有 VmRSS")
        * 1024
}

/// 文件多大。
const SIZE: usize = 64 * 1024 * 1024;
/// 读一页最多该用多少内存：远大于一页的 64 KiB，远小于文件。
const BUDGET: usize = 16 * 1024 * 1024;

/// 在场地的 `work/` 里写一份 `head` 打头、`piece` 填到 [`SIZE`] 的文件：一片片写，不在内存里攒整份。
fn written(site: &Site, name: &str, head: &[u8], piece: &[u8]) {
    let mut file = std::fs::File::create(site.0.join("work").join(name)).expect("建得了");
    file.write_all(head).expect("写得进");
    let mut left = SIZE;
    while left > 0 {
        let n = left.min(piece.len());
        file.write_all(&piece[..n]).expect("写得进");
        left -= n;
    }
}

/// 读 `name` 的第一页，交回出没出错、量到的最多多分出去多少。
async fn peak_of(site: &Site, name: &str) -> (bool, usize) {
    // 峰值清零（写 5 只清 `VmHWM`）。
    std::fs::write("/proc/self/clear_refs", "5").expect("清得了峰值");
    let before = resident();
    let (error, _) = site
        .call("read", serde_json::json!({"file_path": name}))
        .await;
    (error, high_water().saturating_sub(before))
}

#[tokio::test]
async fn reading_one_page_does_not_hold_the_whole_file() {
    let site = Site::new();
    let line = vec![b'a'; 1024 * 1024];
    // 一份没有换行的 UTF-8 文本。
    written(&site, "one-line.txt", b"", &line);
    // 一份带 BOM 的 UTF-16（小端）：「a」一个字两个字节，中间有换行。
    let mut utf16 = Vec::new();
    for k in 0..512 * 1024 {
        let unit: u16 = if k % 80 == 79 {
            u16::from(b'\n')
        } else {
            u16::from(b'a')
        };
        utf16.extend_from_slice(&unit.to_le_bytes());
    }
    written(&site, "wide.txt", &[0xFF, 0xFE], &utf16);
    drop((line, utf16));

    let mut problems = Vec::new();
    for name in ["one-line.txt", "wide.txt"] {
        let (error, peak) = peak_of(&site, name).await;
        if error || peak > BUDGET {
            problems.push(format!(
                "{name}（{} MiB）：出错 {error}，读一页最多分出去 {} MiB（该在 {} MiB 以内）",
                SIZE >> 20,
                peak >> 20,
                BUDGET >> 20
            ));
        }
    }
    drop(std::fs::remove_dir_all(&site.0));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
