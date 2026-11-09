//! 权限级别怎么判（从 `guard.rs` 挪来，施工 5-15 下：那边到了 500 行）：实际生效的那一级，一条路径照级别、所在的那一片判。

use miyu_fs::Zone;
use miyu_kernel::event::{Level, Permission};

/// 实际生效的那一级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effective {
    /// 完全放开。
    Full,
    /// 只读：开着只读开关，或者常用的那一级不认识（按最严的算）。
    ReadOnly,
}

/// 一条路径判下来是什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mark {
    /// 放行。
    Allow,
    /// 要问人。
    Ask,
    /// 拒绝：碰到了数据根。
    Forbidden,
    /// 拒绝：只读的时候要写。
    ReadOnly,
}

/// 实际生效的那一级：执行器照同一个算法给命令写沙盒的规格。
pub(crate) fn effective(permission: &Permission) -> Effective {
    if permission.read_only {
        return Effective::ReadOnly;
    }
    match permission.level {
        Level::Full => Effective::Full,
        Level::Other(_) => Effective::ReadOnly,
    }
}

/// 一条路径照级别判（11 第二节的判法表，A16 起只剩完全放开、只读两列）：数据根谁都碰不到；完全放开别的都放行；只读读哪儿都
/// 放行（别的程序的私密目录问人，施工 5-13），写一律拒。
pub(super) fn mark(level: Effective, zone: Zone, write: bool) -> Mark {
    match (zone, level, write) {
        (Zone::Forbidden, _, _) | (Zone::Shelf, _, true) => Mark::Forbidden,
        (_, Effective::Full, _) => Mark::Allow,
        (_, Effective::ReadOnly, true) => Mark::ReadOnly,
        (Zone::Private, Effective::ReadOnly, false) => Mark::Ask,
        (_, Effective::ReadOnly, false) => Mark::Allow,
    }
}
