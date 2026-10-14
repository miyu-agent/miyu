//! 探到的沙盒记下来（施工 V-5 上，`docs/blueprint/sandbox.md`「怎么走」第 2 条）：探一次要起助手、等它说完，十几到
//! 二十毫秒，以前在接连接之前做，冷启动整段等着它。助手和系统没换的，下一次起来照记下的用，后台再探一遍、照新的记，
//! 下一次起来用新的。
//!
//! 认不认得照四样：助手的路径、大小、改动时间，加上系统的版本（Linux 照内核的版本号：Landlock 跟着内核走；别的平台照
//! 平台名和架构）。说法的版本 `miyu_sandbox::VERSION` 在记下的那一份里，对不上的当没记过。

use std::path::Path;
use std::time::UNIX_EPOCH;

use serde::{Deserialize, Serialize};

use miyu_sandbox::Probe;

/// 记在状态目录里的文件名。
pub(crate) const FILE: &str = "sandbox-probe.json";

/// 认这一份记下的照什么。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Key {
    /// 助手的路径。
    helper: String,
    /// 助手的大小。
    len: u64,
    /// 助手的改动时间，纳秒。
    modified_ns: u128,
    /// 系统的版本。
    system: String,
}

/// 文件里的一份。
#[derive(Debug, Serialize, Deserialize)]
struct Remembered {
    key: Key,
    probe: Probe,
}

/// 助手 `helper` 现在的样子；读不到它的大小、改动时间的交回空的（那就不记、不认）。
pub(crate) fn key(helper: &Path) -> Option<Key> {
    let meta = std::fs::metadata(helper).ok()?;
    let modified_ns = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_nanos();
    Some(Key {
        helper: helper.to_string_lossy().into_owned(),
        len: meta.len(),
        modified_ns,
        system: system(),
    })
}

/// 系统的版本：Linux 照 `/proc/sys/kernel/osrelease`，读不到的和别的平台照平台名和架构。
fn system() -> String {
    let base = format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH);
    if cfg!(target_os = "linux")
        && let Ok(release) = std::fs::read_to_string("/proc/sys/kernel/osrelease")
    {
        return format!("{base}-{}", release.trim());
    }
    base
}

/// 状态目录 `state` 里记着的、`key` 对得上、说法的版本也对得上的那一份；没有的、读不懂的交回空的。
pub(crate) fn recall(state: &Path, key: &Key) -> Option<Probe> {
    let bytes = std::fs::read(state.join(FILE)).ok()?;
    let remembered: Remembered = serde_json::from_slice(&bytes).ok()?;
    (remembered.key == *key && remembered.probe.version == miyu_sandbox::VERSION)
        .then_some(remembered.probe)
}

/// 记下这一次探到的：先写到旁边、再换过去，写到一半停了也不会留下半份。写不成的记一行 `DEBUG`，下次照常探。
pub(crate) fn remember(state: &Path, key: &Key, probe: &Probe) {
    let remembered = Remembered {
        key: key.clone(),
        probe: probe.clone(),
    };
    let written = serde_json::to_vec(&remembered)
        .map_err(std::io::Error::other)
        .and_then(|bytes| {
            std::fs::create_dir_all(state)?;
            let temp = state.join(format!("{FILE}.{}.tmp", std::process::id()));
            std::fs::write(&temp, bytes)?;
            std::fs::rename(&temp, state.join(FILE))
        });
    if let Err(error) = written {
        tracing::debug!(target: crate::TARGET, error = %error, "sandbox probe not remembered");
    }
}
