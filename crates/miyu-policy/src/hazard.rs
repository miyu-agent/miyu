//! 高危命令（施工 5-17，`docs/designs/11-权限与沙盒.md` 第四节「高危命令照规则拦」，2026-10-20 项目主人定）：照一条命令的字面
//! 认出最直接的几种，防的是她一时糊涂，不是刻意绕的人。认出来的说哪一种（[`Kind`]）、拒还是问（[`Kind::refused`]）。
//!
//! 照 Codex、Claude Code、opencode、dsh、Gemini CLI 的现成做法（调研 2026-10-20）：先拆成一个个简单命令（`lex.rs`），看穿
//! `sudo`、`env`、`sh -c` 这类外壳；删什么照解析出的目标判（家目录、根目录、根下那几个系统目录、数据根），不照有没有 `-f`；
//! git 跳过 `-C`、`-c` 这类全局选项再认子命令。纯逻辑：路径只照字面换（`~`、`$HOME`、相对的接工作目录、`..`），不碰磁盘。

mod container;
mod lex;
mod prefix;
mod rules;
mod vars;

pub use prefix::prefix;
pub use rules::assess;

/// 认出来的是哪一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// 删根目录。
    DeleteRoot,
    /// 删家目录（或者它上面的）。
    DeleteHome,
    /// 删根下的系统目录（`/usr`、`/etc`、`/home` 这一层）。
    DeleteSystem,
    /// 删 Miyu 的数据根，或者装着它的目录。
    DeleteDataRoot,
    /// 删工作目录或者它的上级。
    DeleteWorkingDir,
    /// 删的目标要到运行时才知道（变量、命令替换），又是整个目录往下删的。
    DeleteUnresolved,
    /// `git push --force` 这类改写远端的。
    GitForcePush,
    /// `git reset --hard`。
    GitResetHard,
    /// `git clean -f`。
    GitClean,
    /// `git checkout -- .`、`git restore .` 这类丢掉改动的。
    GitDiscard,
    /// `git stash drop`、`git stash clear`。
    GitStashDrop,
    /// 格式化磁盘（`mkfs`、`wipefs`）。
    DiskFormat,
    /// 往裸设备写（`dd of=/dev/…`、`> /dev/sd…`）。
    DiskWrite,
    /// 关机、重启。
    Power,
    /// 下载下来直接执行（`curl … | sh`）。
    PipeToShell,
    /// 容器把整个宿主交出去（施工 5-17 三补）：`--privileged`、加 `ALL`/`SYS_ADMIN`、挂宿主的 `/`、`/dev`、`/sys`。
    ContainerHost,
    /// 容器拿到宿主的一部分（施工 5-17 三补）：`--device`、`--pid`/`--net`/`--ipc` 写 `host`、挂 `/dev`、`/sys` 下面的。
    ContainerShare,
}

impl Kind {
    /// 每一种：直接拒的那几种各有一句说它做了什么，基础系统照这张表读。
    pub const ALL: [Kind; 17] = [
        Kind::DeleteRoot,
        Kind::DeleteHome,
        Kind::DeleteSystem,
        Kind::DeleteDataRoot,
        Kind::DeleteWorkingDir,
        Kind::DeleteUnresolved,
        Kind::GitForcePush,
        Kind::GitResetHard,
        Kind::GitClean,
        Kind::GitDiscard,
        Kind::GitStashDrop,
        Kind::DiskFormat,
        Kind::DiskWrite,
        Kind::Power,
        Kind::PipeToShell,
        Kind::ContainerHost,
        Kind::ContainerShare,
    ];

    /// 协议、日志里的写法（问人时说明里的 `hazard`）。
    pub const fn code(self) -> &'static str {
        match self {
            Kind::DeleteRoot => "delete_root",
            Kind::DeleteHome => "delete_home",
            Kind::DeleteSystem => "delete_system",
            Kind::DeleteDataRoot => "delete_data_root",
            Kind::DeleteWorkingDir => "delete_working_dir",
            Kind::DeleteUnresolved => "delete_unresolved",
            Kind::GitForcePush => "git_force_push",
            Kind::GitResetHard => "git_reset_hard",
            Kind::GitClean => "git_clean",
            Kind::GitDiscard => "git_discard",
            Kind::GitStashDrop => "git_stash_drop",
            Kind::DiskFormat => "disk_format",
            Kind::DiskWrite => "disk_write",
            Kind::Power => "power",
            Kind::PipeToShell => "pipe_to_shell",
            Kind::ContainerHost => "container_host",
            Kind::ContainerShare => "container_share",
        }
    }

    /// 灭顶的直接拒，人点了也不跑（2026-10-20 项目主人定）：删根、家目录、系统目录、数据根，格式化、往裸设备写；容器把整个
    /// 宿主交出去（施工 5-17 三补，2026-10-22 项目主人定）。别的问人。
    pub const fn refused(self) -> bool {
        matches!(
            self,
            Kind::DeleteRoot
                | Kind::DeleteHome
                | Kind::DeleteSystem
                | Kind::DeleteDataRoot
                | Kind::DiskFormat
                | Kind::DiskWrite
                | Kind::ContainerHost
        )
    }
}

/// 判的时候要的，都是绝对路径的写法（真实的位置更好，照字面比）。
#[derive(Debug, Clone, Copy)]
pub struct Places<'a> {
    /// 这一轮的工作目录：相对的照它接。
    pub cwd: &'a str,
    /// 系统的家目录：`~`、`$HOME` 照它接。
    pub home: Option<&'a str>,
    /// Miyu 的数据根。账号的工作区 `home/<账号>/workspace/` 里面的不算（施工 5-14 起会话的默认工作区都在它下面）。
    pub data_root: &'a str,
}

#[cfg(test)]
mod tests;
