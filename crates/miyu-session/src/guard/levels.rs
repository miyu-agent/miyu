//! 权限级别怎么判（从 `guard.rs` 挪来，施工 5-15 下：那边到了 500 行）：实际生效的那一级，一条路径照级别判。

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

/// 一条路径照级别判（11 第二节的判法表，A16 起只剩完全放开、只读两列）：读哪儿都放行；完全放开哪儿都能写，数据根也是
/// （施工 RO-3，2026-11-17 项目主人定：文件工具拦得住、命令拦不住，等于没保护；删整个数据根照旧由高危命令、`trash` 自己拦）；
/// 只读写一律拒（施工 PV-RM 起只读就是完全放开减去写）。在哪一片只给只碰得到自己工作区的会话判（`Confined`）。
pub(super) fn mark(level: Effective, write: bool) -> Mark {
    match (level, write) {
        (Effective::ReadOnly, true) => Mark::ReadOnly,
        _ => Mark::Allow,
    }
}
