//! 还剩多少可用内存（施工 MEM-1 起，`docs/blueprint/heap.md`「还剩多少内存」）：核心隔一阵看一眼，内存紧、核心自己又是大头时
//! 把闲着的会话卸出内存（施工 MEM-2），不撑到整个核心被系统杀掉。这里只管读，判紧不紧、卸哪些是核心的事（`miyu-endpoint` 的
//! 会话表）。整台机器的和核心所在 cgroup 的分开交：两种紧判法不一样。
//!
//! - Linux：系统的 `/proc/meminfo`（`MemAvailable`、`MemTotal`）；核心所在的 cgroup v2：从 `/proc/self/cgroup` 写的那一层
//!   一路往上走到根，每层有 `memory.max`（不是 `max`）的算一份，可用的是上限减去在用的（在用的扣掉回收得了的
//!   `inactive_file`），几层里照比例最低的那一份。
//! - macOS：内核给的可用百分比 `kern.memorystatus_level`，总量 `hw.memsize`。没有 cgroup。
//! - Windows：`GlobalMemoryStatusEx` 的 `ullAvailPhys`、`ullTotalPhys`。没有 cgroup。
//!
//! 哪一样读不到的是没有，调的一方当作「不知道」，不当紧。

use std::path::Path;

/// 还剩多少可用内存，字节：`available` 可用的，`total` 总量。总量不是 0（读到 0 的当读不到）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Room {
    /// 可用的：不挤掉别人就拿得到的。
    pub available: u64,
    /// 总量：系统的物理内存，或者 cgroup 的上限。
    pub total: u64,
}

impl Room {
    /// 可用的占总量的百分之几，往下取整。
    pub fn percent(&self) -> u64 {
        match self.total {
            0 => 0,
            total => {
                let percent = u128::from(self.available) * 100 / u128::from(total);
                u64::try_from(percent).unwrap_or(u64::MAX)
            }
        }
    }

    /// 可用的比总量的 `percent`% 少没有：正好等于的不算少。照整数乘开比，不经浮点、不先取整。
    pub fn below(&self, percent: u64) -> bool {
        u128::from(self.available) * 100 < u128::from(self.total) * u128::from(percent)
    }

    /// 可用的，MiB，往下取整。
    pub fn mib(&self) -> u64 {
        self.available / (1024 * 1024)
    }

    /// 照比例看，自己比 `other` 紧没有（交叉乘开比，一样紧的不算）。
    fn tighter_than(&self, other: &Room) -> bool {
        u128::from(self.available) * u128::from(other.total)
            < u128::from(other.available) * u128::from(self.total)
    }
}

/// 这一刻的两份读数（施工 MEM-2）：整台机器的，和核心所在 cgroup 一路往上有上限的几层里最紧的那一层。读不到的、没有上限的
/// 是没有。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rooms {
    /// 整台机器的：总量是物理内存。
    pub system: Option<Room>,
    /// 核心所在 cgroup（只认 v2）从自己那一层往上到根，有 `memory.max` 的几层里照比例最紧的那一层：总量是那一层的上限。
    /// 一层都没有上限的、只有 v1 的、不是 Linux 的没有。
    pub cgroup: Option<Room>,
}

/// 几份里照比例最紧的那一份；一份都没有的没有。
pub fn tightest(rooms: impl IntoIterator<Item = Room>) -> Option<Room> {
    rooms
        .into_iter()
        .fold(None, |tightest, room| match tightest {
            Some(seen) if !room.tighter_than(&seen) => Some(seen),
            _ => Some(room),
        })
}

/// 这一刻还剩多少：见模块的说明。Linux 上读几个小文件（`/proc`、`/sys` 下，不碰磁盘），macOS、Windows 上问系统一次。
pub fn room() -> Rooms {
    imp::room()
}

/// 读 `/proc/meminfo` 的内容：`MemAvailable`、`MemTotal` 两行（`<名字>: <数> kB`），别的不看。少了哪一行、总量是 0 的没有
/// （3.14 以前的内核没有 `MemAvailable`）。
pub fn parse_meminfo(text: &str) -> Option<Room> {
    let mut available = None;
    let mut total = None;
    for line in text.lines() {
        let Some((name, rest)) = line.split_once(':') else {
            continue;
        };
        let slot = match name.trim() {
            "MemAvailable" => &mut available,
            "MemTotal" => &mut total,
            _ => continue,
        };
        let kib: u64 = rest.trim().strip_suffix("kB")?.trim().parse().ok()?;
        *slot = Some(kib.saturating_mul(1024));
    }
    room_of(available?, total?)
}

/// `/proc/self/cgroup` 里 cgroup v2 的那一层：`0::<路径>` 那一行的路径。只有 v1 的（没有这一行）没有。
pub fn cgroup_path(text: &str) -> Option<&str> {
    text.lines()
        .find_map(|line| line.strip_prefix("0::"))
        .map(str::trim)
}

/// cgroup 的一层：`memory.max`、`memory.current` 的内容，和 `memory.stat`（读不到的不写）。上限是 `max`（不限）、读不懂的
/// 没有。可用的是上限减去在用的；在用的里面扣掉 `inactive_file`（回收得了的文件缓存：读过的日志都算在 cgroup 头上，不扣的话
/// 读过一阵文件，上限还远就当作满了），减出来比上限大的照上限，在用的超过了上限的是 0。
pub fn cgroup_level(max: &str, current: &str, stat: Option<&str>) -> Option<Room> {
    let max = max.trim();
    if max == "max" {
        return None;
    }
    let max: u64 = max.parse().ok()?;
    let current: u64 = current.trim().parse().ok()?;
    let reclaimable = stat.and_then(inactive_file).unwrap_or(0);
    let used = current.saturating_sub(reclaimable);
    room_of(max.saturating_sub(used), max)
}

/// `memory.stat` 里 `inactive_file <字节>` 那一行。
fn inactive_file(stat: &str) -> Option<u64> {
    stat.lines().find_map(|line| {
        let value = line.strip_prefix("inactive_file ")?;
        value.trim().parse().ok()
    })
}

/// Linux 上的读法，读文件照 `read`（给绝对路径，读不到的交回没有）：系统的是 `/proc/meminfo`；cgroup 的是
/// `/proc/self/cgroup` 写的那一层起往上每层一份（`/sys/fs/cgroup/<路径>/memory.max` 这几样，见 [`cgroup_level`]）里照比例
/// 最紧的那一份。测试照它喂假文件，不靠本机。
pub fn linux_with(read: &dyn Fn(&Path) -> Option<String>) -> Rooms {
    let system = read(Path::new("/proc/meminfo")).and_then(|text| parse_meminfo(&text));
    let cgroup = read(Path::new("/proc/self/cgroup"))
        .and_then(|text| cgroup_path(&text).map(str::to_owned))
        .and_then(|path| tightest(cgroups(read, &path)));
    Rooms { system, cgroup }
}

/// 从 cgroup 的 `path` 那一层往上到根，每层读得出上限的一份。根那一层没有 `memory.max`，自然跳过。
fn cgroups(read: &dyn Fn(&Path) -> Option<String>, path: &str) -> Vec<Room> {
    let base = Path::new("/sys/fs/cgroup");
    let mut found = Vec::new();
    let mut at = Some(Path::new(path.trim_start_matches('/')));
    while let Some(level) = at {
        let dir = base.join(level);
        let file = |name: &str| read(&dir.join(name));
        if let (Some(max), Some(current)) = (file("memory.max"), file("memory.current"))
            && let Some(room) = cgroup_level(&max, &current, file("memory.stat").as_deref())
        {
            found.push(room);
        }
        at = level.parent();
    }
    found
}

/// 总量是 0 的当读不到。
fn room_of(available: u64, total: u64) -> Option<Room> {
    (total > 0).then_some(Room { available, total })
}

/// macOS 上的读法：可用百分比 `level`（0 到 100，`kern.memorystatus_level`）乘上总量 `total`（`hw.memsize`）。百分比超过
/// 100 的、总量是 0 的没有。
pub fn macos_of(level: u64, total: u64) -> Option<Room> {
    if level > 100 {
        return None;
    }
    let available = u128::from(total) * u128::from(level) / 100;
    room_of(u64::try_from(available).ok()?, total)
}

#[cfg(target_os = "linux")]
mod imp {
    pub(super) fn room() -> super::Rooms {
        super::linux_with(&|path| std::fs::read_to_string(path).ok())
    }
}

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
mod imp {
    /// macOS 没有 cgroup：只有整台机器的。
    pub(super) fn room() -> super::Rooms {
        super::Rooms {
            system: super::macos::room(),
            cgroup: None,
        }
    }
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
mod imp {
    /// Windows 没有 cgroup：只有整台机器的。
    pub(super) fn room() -> super::Rooms {
        super::Rooms {
            system: super::windows::room(),
            cgroup: None,
        }
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod imp {
    pub(super) fn room() -> super::Rooms {
        super::Rooms::default()
    }
}

#[cfg(test)]
mod tests;
