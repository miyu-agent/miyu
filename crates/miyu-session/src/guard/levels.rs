//! 权限级别怎么判（从 `guard.rs` 挪来，施工 5-15 下：那边到了 500 行）：实际生效的那一级，一条路径照级别、所在的那一片判。

use miyu_fs::Zone;
use miyu_kernel::event::{Level, Permission};

/// 实际生效的那一级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effective {
    /// 完全放开。
    Full,
    /// 工作区。
    Workspace,
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
        Level::Workspace => Effective::Workspace,
        Level::Other(_) => Effective::ReadOnly,
    }
}

/// 一条路径照级别判（11 第二节的判法表）：读哪儿都放行，数据根除外（施工 5-4 上，原来边界以外的读要问人）；别的程序的私密
/// 目录读写都问人，完全放开不问，只读时写照拒（施工 5-13）。
pub(super) fn mark(level: Effective, zone: Zone, write: bool) -> Mark {
    match (zone, level, write) {
        (Zone::Forbidden, _, _) | (Zone::Shelf, _, true) => Mark::Forbidden,
        (_, Effective::Full, _) => Mark::Allow,
        (Zone::Private, Effective::ReadOnly, true) => Mark::ReadOnly,
        (Zone::Private, _, _) => Mark::Ask,
        (_, _, false) => Mark::Allow,
        (_, Effective::ReadOnly, true) => Mark::ReadOnly,
        (Zone::Writable, Effective::Workspace, true) => Mark::Allow,
        (Zone::Readable | Zone::Outside, Effective::Workspace, true) => Mark::Ask,
    }
}
