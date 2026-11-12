//! 还剩多少内存：几种文件的读法、cgroup 一层层取最紧的、比例和门槛的算法；真机上读得到。读文件的都喂假的，不靠本机。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use super::*;

const GIB: u64 = 1024 * 1024 * 1024;

/// 一份真机的 `/proc/meminfo` 开头（Linux 6.x）。
const MEMINFO: &str = "\
MemTotal:       32734568 kB
MemFree:         1203304 kB
MemAvailable:   20123456 kB
Buffers:          402324 kB
Cached:         17404832 kB
";

/// 假的文件：照绝对路径找，没有的读不到。
fn files(entries: &[(&str, &str)]) -> BTreeMap<PathBuf, String> {
    entries
        .iter()
        .map(|(path, text)| (PathBuf::from(path), (*text).to_string()))
        .collect()
}

fn linux(entries: &[(&str, &str)]) -> Option<Room> {
    let files = files(entries);
    linux_with(&|path: &Path| files.get(path).cloned())
}

fn room(available: u64, total: u64) -> Room {
    Room { available, total }
}

#[test]
fn meminfo_is_read_in_bytes() {
    assert_eq!(
        parse_meminfo(MEMINFO),
        Some(room(20_123_456 * 1024, 32_734_568 * 1024))
    );
}

#[test]
fn meminfo_without_available_or_with_a_zero_total_is_unknown() {
    assert_eq!(parse_meminfo("MemTotal: 1024 kB\nMemFree: 512 kB\n"), None);
    assert_eq!(parse_meminfo("MemTotal: 0 kB\nMemAvailable: 0 kB\n"), None);
    assert_eq!(parse_meminfo("MemTotal: lots\nMemAvailable: 1 kB\n"), None);
    assert_eq!(parse_meminfo(""), None);
}

#[test]
fn only_the_v2_line_names_the_cgroup() {
    let text = "12:memory:/user.slice\n0::/user.slice/bm.scope\n";
    assert_eq!(cgroup_path(text), Some("/user.slice/bm.scope"));
    assert_eq!(cgroup_path("12:memory:/user.slice\n"), None);
}

#[test]
fn an_unlimited_or_unreadable_level_has_no_room() {
    assert_eq!(cgroup_level("max\n", "123\n", None), None);
    assert_eq!(cgroup_level("lots", "123", None), None);
    assert_eq!(cgroup_level("100", "", None), None);
    assert_eq!(cgroup_level("0", "0", None), None);
}

#[test]
fn a_level_is_its_limit_less_what_it_uses() {
    assert_eq!(cgroup_level("1000\n", "300\n", None), Some(room(700, 1000)));
    // 回收得了的文件缓存不算在用的。
    let stat = "anon 200\nfile 500\ninactive_file 250\nactive_file 50\n";
    assert_eq!(
        cgroup_level("1000", "800", Some(stat)),
        Some(room(450, 1000))
    );
    // 超过了上限的是 0；扣得比在用的还多的照上限。
    assert_eq!(cgroup_level("1000", "1200", None), Some(room(0, 1000)));
    let more = "inactive_file 5000\n";
    assert_eq!(
        cgroup_level("1000", "800", Some(more)),
        Some(room(1000, 1000))
    );
}

#[test]
fn the_tightest_level_up_the_tree_wins() {
    let tree = [
        ("/proc/meminfo", MEMINFO),
        ("/proc/self/cgroup", "0::/user.slice/bm.scope\n"),
        // 自己那一层：一半。
        ("/sys/fs/cgroup/user.slice/bm.scope/memory.max", "2000\n"),
        (
            "/sys/fs/cgroup/user.slice/bm.scope/memory.current",
            "1000\n",
        ),
        // 上一层：只剩 4%。
        ("/sys/fs/cgroup/user.slice/memory.max", "10000\n"),
        ("/sys/fs/cgroup/user.slice/memory.current", "9600\n"),
    ];
    assert_eq!(linux(&tree), Some(room(400, 10_000)));
}

#[test]
fn an_unlimited_cgroup_leaves_the_system_figure() {
    let tree = [
        ("/proc/meminfo", MEMINFO),
        ("/proc/self/cgroup", "0::/user.slice\n"),
        ("/sys/fs/cgroup/user.slice/memory.max", "max\n"),
        ("/sys/fs/cgroup/user.slice/memory.current", "9600\n"),
    ];
    assert_eq!(linux(&tree), parse_meminfo(MEMINFO));
}

#[test]
fn a_cgroup_tighter_than_the_system_wins() {
    // bm 的样子：cgroup 限 18 GiB、用掉 17.5 GiB，系统还剩六成。
    let max = (18 * GIB).to_string();
    let current = (18 * GIB - GIB / 2).to_string();
    let tree = [
        ("/proc/meminfo", MEMINFO),
        ("/proc/self/cgroup", "0::/bm.scope\n"),
        ("/sys/fs/cgroup/bm.scope/memory.max", max.as_str()),
        ("/sys/fs/cgroup/bm.scope/memory.current", current.as_str()),
    ];
    let found = linux(&tree).expect("读得到");
    assert_eq!(found, room(GIB / 2, 18 * GIB));
    assert_eq!(found.percent(), 2);
    assert!(found.below(5));
}

#[test]
fn without_meminfo_the_cgroup_still_counts() {
    let tree = [
        ("/proc/self/cgroup", "0::/a\n"),
        ("/sys/fs/cgroup/a/memory.max", "100\n"),
        ("/sys/fs/cgroup/a/memory.current", "10\n"),
    ];
    assert_eq!(linux(&tree), Some(room(90, 100)));
}

#[test]
fn nothing_readable_is_unknown() {
    assert_eq!(linux(&[]), None);
    assert_eq!(linux(&[("/proc/meminfo", "garbage")]), None);
    // 只有 v1 的 cgroup、根那一层：只看系统的。
    let v1 = [("/proc/meminfo", MEMINFO), ("/proc/self/cgroup", "0::/\n")];
    assert_eq!(linux(&v1), parse_meminfo(MEMINFO));
}

#[test]
fn the_percent_rounds_down_and_never_divides_by_zero() {
    assert_eq!(room(49, 1000).percent(), 4);
    assert_eq!(room(50, 1000).percent(), 5);
    assert_eq!(room(1000, 1000).percent(), 100);
    assert_eq!(room(0, 0).percent(), 0);
    assert_eq!(room(u64::MAX, u64::MAX).percent(), 100);
    assert_eq!(room(3 * 1024 * 1024 + 5, 10 * GIB).mib(), 3);
}

#[test]
fn exactly_the_threshold_is_not_below_it() {
    assert!(room(49, 1000).below(5));
    assert!(room(4_999_999, 100_000_000).below(5));
    assert!(!room(50, 1000).below(5));
    assert!(!room(5_000_000, 100_000_000).below(5));
    assert!(room(0, 1000).below(5));
    assert!(!room(0, 1000).below(0));
    assert!(!room(u64::MAX, u64::MAX).below(5));
}

#[test]
fn the_tightest_goes_by_the_ratio() {
    assert_eq!(tightest([]), None);
    // 剩得多、比例低的那一份更紧。
    let rooms = [room(500, 1000), room(1000, 100_000), room(900, 1000)];
    assert_eq!(tightest(rooms), Some(room(1000, 100_000)));
    // 一样紧的取先来的。
    assert_eq!(tightest([room(1, 2), room(2, 4)]), Some(room(1, 2)));
}

#[test]
fn the_macos_level_is_a_share_of_the_total() {
    assert_eq!(
        macos_of(3, 16 * GIB),
        Some(room(16 * GIB * 3 / 100, 16 * GIB))
    );
    assert_eq!(macos_of(100, 8), Some(room(8, 8)));
    assert_eq!(macos_of(101, 8), None);
    assert_eq!(macos_of(50, 0), None);
}

/// 真机上：三个平台都读得到、可用的不比总量多。
#[test]
fn this_machine_has_a_figure() {
    let found = super::room();
    if cfg!(any(target_os = "linux", target_os = "macos", windows)) {
        let found = found.expect("读得到");
        assert!(found.total > 0, "{found:?}");
        assert!(found.available <= found.total, "{found:?}");
    } else {
        assert_eq!(found, None);
    }
}
