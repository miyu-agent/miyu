//! 测试的临时目录（施工 TMP-1）：门禁跑测试时把系统的临时目录（`TMPDIR`，Windows 上是 `TMP`、`TEMP`）指到一个新的空目录，
//! 跑完数里面剩下了什么：测试的夹具都该自己清干净，剩下的照名字的前缀报出来（同一个夹具建的名字只差进程号、时刻、序号），
//! 然后整个删掉。这样门禁、CI 跑多少趟都不往机器的临时目录里攒（2026-10-28 攒到十二万个目录，把 `/tmp` 的配额顶满过）。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// 剩下的算不算错：只在 Linux 上算。攒满配额的是开发机（Linux）上的 `/tmp`；Windows 上别处还开着的文件删不掉（后台任务、
/// 核心进程拿着库的句柄），要做到零得把每个夹具改成先关掉所有句柄再删，CI 的机器又是一次一扔的，不值；macOS 照数照报。
pub const ENFORCED: bool = cfg!(target_os = "linux");

/// 本来就活得比进程长、不算夹具没清的（名字的开头；这些照样跟着整个目录删掉）：
/// - `miyu-call-`：核心进程的回调目录（`miyu-call-<进程号>`），一个核心进程一个，下一个核心进程起来时按进程号还活不活清掉
///   别人留下的（`miyu-basesystem` 的 `shell/channel`）；
/// - `miyu-scripts`：没有数据根时脚本工具的缓存，名字固定、不随进程攒，下次接着用。
const KEPT: [&str; 2] = ["miyu-call-", "miyu-scripts"];

/// 门禁这一趟的临时目录：系统临时目录下 `m<进程号>`。名字短：macOS 上 Unix 套接字的路径有 104 字节的上限，系统的临时目录
/// 本来就长。
pub fn fresh() -> std::io::Result<PathBuf> {
    let dir = std::env::temp_dir().join(format!("m{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// 跑完以后：`dir` 里剩下的照前缀数，一种一行；然后整个删掉（删不掉的另报一行）。
pub fn left(dir: &Path) -> Vec<String> {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if KEPT.iter().any(|kept| name.starts_with(kept)) {
                continue;
            }
            *counts.entry(prefix(&name)).or_default() += 1;
        }
    }
    let mut problems: Vec<String> = counts
        .into_iter()
        .map(|(prefix, count)| format!("测试留下了 {count} 个 {prefix}*：夹具没清干净"))
        .collect();
    if let Err(error) = std::fs::remove_dir_all(dir) {
        problems.push(format!("{} 删不掉：{error}", dir.display()));
    }
    problems
}

/// 名字的前缀：去掉第一段数字（进程号、时刻、序号这些）开头的那一段以后的。没有数字段的照原样。
fn prefix(name: &str) -> String {
    let mut kept = Vec::new();
    for part in name.split(['-', '_', '.']) {
        if part.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            break;
        }
        kept.push(part);
    }
    match kept.len() {
        0 => name.to_string(),
        _ => format!("{}-", kept.join("-")),
    }
}

#[cfg(test)]
mod tests {
    use super::{left, prefix};

    #[test]
    fn names_are_grouped_by_what_comes_before_the_numbers() {
        assert_eq!(
            prefix("miyu-session-1609955-1793171295661506566-2"),
            "miyu-session-"
        );
        assert_eq!(
            prefix("miyu-endpoint-dream-1418131-35"),
            "miyu-endpoint-dream-"
        );
        assert_eq!(
            prefix("miyu-endpoint-1421515-1793171126279432256-4-sandbox-tmp"),
            "miyu-endpoint-"
        );
        assert_eq!(prefix("tmp.XkWUV0i20l"), "tmp-XkWUV0i20l-");
        assert_eq!(prefix("miyu-scripts"), "miyu-scripts-");
    }

    #[test]
    fn what_outlives_a_process_is_not_counted_but_everything_is_removed() {
        let dir = std::env::temp_dir().join(format!("xtask-temp-{}", std::process::id()));
        for name in [
            "miyu-call-12",
            "miyu-scripts",
            "miyu-session-3-4",
            "miyu-session-5-6",
        ] {
            std::fs::create_dir_all(dir.join(name)).expect("建得了目录");
        }
        assert_eq!(left(&dir), ["测试留下了 2 个 miyu-session-*：夹具没清干净"]);
        assert!(!dir.exists());
    }
}
