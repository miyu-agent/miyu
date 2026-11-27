//! `edit` 找旧串多占的内存（施工 ED-3）：原来给整份原文造一张逐字节的对照表，每个字节记一对 `usize`（16 个字节），改一个
//! 500 kB 的文件这一张表就有 8 MB，精确对上的、宽松对上的都造。期望：多占的只是原文的几份（读进来的字节、解开的字、
//! 写回去的字节、宽松那一层的看法），和对照表无关。修之前这条是红的。
//!
//! 量的是整个进程的峰值常驻内存（同 `read_memory.rs`：Linux 的 `/proc/self/status` 里的 `VmHWM`，量之前写
//! `/proc/self/clear_refs` 清零），所以自己一个测试程序，只有这一个测试，只在 Linux 上跑。
#![cfg(target_os = "linux")]

mod support;

use serde_json::json;

use support::Site;

/// `/proc/self/status` 里一格的值，字节。
fn status(field: &str) -> usize {
    let status = std::fs::read_to_string("/proc/self/status").expect("读得了");
    status
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .and_then(|rest| {
            rest.trim()
                .trim_end_matches("kB")
                .trim()
                .parse::<usize>()
                .ok()
        })
        .unwrap_or_else(|| panic!("有 {field}"))
        * 1024
}

/// 文件多大。
const SIZE: usize = 500 * 1000;
/// 改一处最多该多占多少：原文的八份（修完量到约三份）。原来光对照表就是原文的十六份。
const BUDGET: usize = 8 * SIZE;

/// 场地的 `work/` 里写一份约 [`SIZE`] 字节的文件：一行一个编号，倒数第二行是 `last`。
fn written(site: &Site, name: &str, last: &str) {
    let mut text = String::with_capacity(SIZE + 100);
    let mut n = 0;
    while text.len() < SIZE {
        text.push_str(&format!("let value_{n} = {n};\n"));
        n += 1;
    }
    text.push_str(last);
    text.push_str("\nend\n");
    site.file(&format!("work/{name}"), text.as_bytes());
}

/// 调一次 `edit`，交回出没出错、量到的最多多占多少。
async fn peak_of(site: &Site, name: &str, old: &str, new: &str) -> (bool, usize) {
    let args = json!({"file_path": name, "old_string": old, "new_string": new});
    // 峰值清零（写 5 只清 `VmHWM`）。
    std::fs::write("/proc/self/clear_refs", "5").expect("清得了峰值");
    let before = status("VmRSS:");
    let done = site
        .done_seen("work", "edit", args, Default::default())
        .await;
    (done.error, status("VmHWM:").saturating_sub(before))
}

#[tokio::test]
async fn finding_the_old_string_does_not_map_every_byte() {
    let site = Site::new();
    written(&site, "exact.rs", "let target = 1;");
    // 原文是弯引号，她写的是直的：宽松那一层才对得上。
    written(&site, "loose.rs", "let target = \u{201C}x\u{201D};");
    // 先改一个小文件：线程、资源目录里的字这些头一回才有的不算进去。
    site.file("work/small.rs", b"a\n");
    let (error, _) = peak_of(&site, "small.rs", "a", "b").await;
    assert!(!error);
    let mut problems = Vec::new();
    let mut seen = Vec::new();
    for (name, old, new) in [
        ("exact.rs", "let target = 1;", "let target = 2;"),
        ("loose.rs", "let target = \"x\";", "let target = \"y\";"),
    ] {
        let (error, peak) = peak_of(&site, name, old, new).await;
        seen.push(format!("{name} {peak}"));
        if error || peak > BUDGET {
            problems.push(format!(
                "{name}（{SIZE} 字节）：出错 {error}，多占了 {peak} 字节（该在 {BUDGET} 以内）"
            ));
        }
    }
    drop(std::fs::remove_dir_all(&site.0));
    eprintln!("多占的：{}", seen.join("，"));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}
