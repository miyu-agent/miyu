//! 拉起核心时带的环境变量（施工 LW-12，`internal/blueprint/heap.md`「线程缓存」）：glibc 每条线程的缓存（tcache）每种大小默认留
//! 16 块，放掉的小块先进缓存、不回 arena；核心十几条长住的线程各留一份，空闲时白占约 1 MB。这个上限在进程里改不了（`mallopt`
//! 没有这一项），glibc 只在进程起来时读 `GLIBC_TUNABLES`，所以由拉起核心的一方设在它的环境里。
//!
//! 核心起的命令不该跟着变（人在自己的终端里跑同一条命令是 glibc 默认的样子）：拉起的一方把自己原来的那一份记在
//! [`INHERITED`] 里，核心给命令挑环境变量时照它恢复（`miyu-basesystem` 的 `shell/env.rs`）。

use std::ffi::{OsStr, OsString};

/// 每条线程的缓存每种大小最多留几块。2026-11-22 量过（施工单 `LW-12-线程缓存少留.md`）：留 4 块时核心连着一个头空闲少约
/// 0.7 MB，量尺的各项时延没有退步；一块都不留再少 0.3 MB，重开大会话、追加事件、流式一轮慢一成。
pub const TCACHE: u32 = 4;

/// glibc 起来时读的那个环境变量：几项用冒号隔开，`名字=值`。
pub const TUNABLES: &str = "GLIBC_TUNABLES";

/// 拉起的一方原来的 [`TUNABLES`]（没设的是空的）：核心起的命令照它拿，不带核心自己的那一项。
pub const INHERITED: &str = "MIYU_GLIBC_TUNABLES";

/// 线程缓存每种大小留几块的那一项。
const COUNT: &str = "glibc.malloc.tcache_count";

/// 拉起核心时另外设上的环境变量；`current` 是拉起的一方自己的 [`TUNABLES`]。用的不是 glibc 的（macOS、Windows、musl）没有；
/// `current` 里已经写了线程缓存留几块的也没有：照人写的。
pub fn vars(current: Option<&OsStr>) -> Vec<(&'static str, OsString)> {
    match cfg!(all(target_os = "linux", target_env = "gnu")) {
        true => composed(current),
        false => Vec::new(),
    }
}

/// 同 [`vars`]，不看平台：`current` 后面接上线程缓存那一项，原来的那一份记进 [`INHERITED`]。
fn composed(current: Option<&OsStr>) -> Vec<(&'static str, OsString)> {
    let current = current.filter(|value| !value.is_empty());
    if current.is_some_and(|value| value.to_string_lossy().contains(COUNT)) {
        return Vec::new();
    }
    let ours = format!("{COUNT}={TCACHE}");
    let tuned = match current {
        Some(value) => {
            let mut tuned = value.to_os_string();
            tuned.push(":");
            tuned.push(ours);
            tuned
        }
        None => ours.into(),
    };
    vec![
        (TUNABLES, tuned),
        (
            INHERITED,
            current.map(OsStr::to_os_string).unwrap_or_default(),
        ),
    ]
}

/// 核心起的命令拿到的 [`TUNABLES`]：`inherited` 是核心环境里的 [`INHERITED`]。没有的（核心不是带着 [`vars`] 起的）照核心自己的
/// `own`；有的照它，空的是原来就没设，不传。
pub fn restored(inherited: Option<OsString>, own: Option<OsString>) -> Option<OsString> {
    match inherited {
        Some(value) => Some(value).filter(|value| !value.is_empty()),
        None => own,
    }
}

/// 这个进程带着的 [`TUNABLES`]（核心起来时记一笔：线程缓存那一项带上没有）；没设的、不是 UTF-8 的是空的。
pub fn tunables() -> String {
    std::env::var(TUNABLES).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(current: Option<&str>) -> Vec<(&'static str, String)> {
        composed(current.map(OsStr::new))
            .into_iter()
            .map(|(name, value)| (name, value.to_string_lossy().into_owned()))
            .collect()
    }

    #[test]
    fn the_cache_entry_is_set_and_nothing_is_recorded_when_unset() {
        let want = vec![
            (TUNABLES, "glibc.malloc.tcache_count=4".to_string()),
            (INHERITED, String::new()),
        ];
        assert_eq!(pairs(None), want);
        assert_eq!(pairs(Some("")), want, "空的当没设");
    }

    #[test]
    fn what_was_set_is_kept_in_front_and_recorded() {
        assert_eq!(
            pairs(Some("glibc.malloc.arena_max=2:glibc.pthread.rseq=0")),
            vec![
                (
                    TUNABLES,
                    "glibc.malloc.arena_max=2:glibc.pthread.rseq=0:glibc.malloc.tcache_count=4"
                        .to_string()
                ),
                (
                    INHERITED,
                    "glibc.malloc.arena_max=2:glibc.pthread.rseq=0".to_string()
                ),
            ]
        );
    }

    #[test]
    fn a_count_someone_wrote_is_left_alone() {
        assert!(pairs(Some("glibc.malloc.tcache_count=0")).is_empty());
        assert!(
            pairs(Some(
                "glibc.malloc.arena_max=2:glibc.malloc.tcache_count=16"
            ))
            .is_empty()
        );
    }

    #[test]
    fn only_glibc_gets_them() {
        let got = vars(None);
        match cfg!(all(target_os = "linux", target_env = "gnu")) {
            true => assert_eq!(got.len(), 2),
            false => assert!(got.is_empty()),
        }
    }

    #[test]
    fn commands_get_back_what_the_launcher_had() {
        let own = || Some(OsString::from("glibc.malloc.tcache_count=4"));
        assert_eq!(restored(Some(OsString::new()), own()), None, "原来没设");
        assert_eq!(
            restored(Some("glibc.malloc.arena_max=2".into()), own()),
            Some(OsString::from("glibc.malloc.arena_max=2"))
        );
        assert_eq!(restored(None, own()), own(), "不是带着起的：照核心自己的");
        assert_eq!(restored(None, None), None);
    }
}
