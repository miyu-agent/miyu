//! 认人（施工 XG-1 下，设计 `63-扩展工具的效果和声明.md` 第六节第 2 条）：令牌对得上以后，连上来的进程还要是「这一次的」。两种：
//!
//! - [`Kin::Command`]：内置 `shell` 跑的这条命令，照它的进程组（Unix）、作业对象（Windows）认（施工 M-3，原样）。命令起来以后
//!   才知道是谁，由 [`Binder`] 记一次；连上来时还没记下的等一会儿。
//! - [`Kin::Descendant`]：扩展起的命令，照「是这个扩展进程的子孙」认：从连上来的进程照父进程一层层往上找，找到扩展的进程号
//!   就是。Linux 读 `/proc/<pid>/stat`，macOS 问 `proc_pidinfo`（`miyu_pipe::parent_process`），Windows 拍一张进程快照
//!   （`miyu_pipe::process_parents`）。扩展进程自己不算（「子孙」）；`setsid`、两次 `fork` 甩出去的，父进程链断了，从一开始就
//!   调不成，同 `shell` 的命令出了进程组。这一层防误用（令牌漏给别的进程、命令甩出去以后还在调），不是安全边界：扩展起一个子
//!   进程就能调，对扩展的关口是能力 `tools.commands` 要人批。Unix 上父进程先走了的过继给 1 号（或收养它的进程），链上的编号
//!   总是还在的进程；Windows 不过继，链上可能有被重用的旧编号：每一层都要父进程起得不比它晚（[`elder_parent`]，施工
//!   XG-1补2），重用了编号的进程是后起的，认得出来。

use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crate::Member;

/// 命令刚起、还没记下它是谁时，连上来的最多等多久。
const BIND_WAIT: Duration = Duration::from_secs(5);

/// 父进程链最多往上找几层：一般就三四层（扩展、shell、脚本、`miyu call`），找到头（0、1 号）或者绕回来的先停。
const MOST_HOPS: usize = 256;

/// 这条命令是谁（进程组、作业对象）：起来以后才知道，记一次。
pub(crate) type Bound = Arc<OnceLock<Member>>;

/// 一个端点认谁。
#[derive(Clone)]
pub enum Kin {
    /// 内置 `shell` 跑的这条命令：起来以后照 [`crate::Channel::binder`] 记下进程组、作业对象。
    Command(Bound),
    /// 进程号是这个的进程（扩展）的子孙。
    Descendant(u32),
}

impl Kin {
    /// 一条还没起来的命令：起来以后记。
    pub fn command() -> Kin {
        Kin::Command(Arc::new(OnceLock::new()))
    }

    /// 进程 `pid`（扩展）的子孙。
    pub fn descendant_of(pid: u32) -> Kin {
        Kin::Descendant(pid)
    }

    /// 记这条命令是谁的那一头：只有 [`Kin::Command`] 有。
    pub(crate) fn binder(&self) -> Option<Binder> {
        match self {
            Kin::Command(bound) => Some(Binder(Arc::clone(bound))),
            Kin::Descendant(_) => None,
        }
    }

    /// 连上来的进程 `pid` 是不是自己人：问不出进程号的不是。
    pub(crate) async fn ours(&self, pid: Option<u32>) -> bool {
        let Some(pid) = pid else {
            return false;
        };
        match self {
            Kin::Command(bound) => match bound_member(bound).await {
                Some(member) => in_command(&member, pid),
                None => false,
            },
            Kin::Descendant(ancestor) => {
                let ancestor = *ancestor;
                tokio::task::spawn_blocking(move || descends_here(pid, ancestor))
                    .await
                    .unwrap_or(false)
            }
        }
    }
}

/// 记这条命令是谁的那一头。
pub struct Binder(Bound);

impl Binder {
    /// 记下 `member`：只记一次。
    pub fn bind(self, member: Member) {
        if self.0.set(member).is_err() {
            tracing::debug!(target: crate::TARGET, "miyu call endpoint bound twice");
        }
    }
}

/// 等到记下了这条命令是谁，最多等 [`BIND_WAIT`]；等不到的没有。
#[cfg_attr(
    unix,
    expect(
        clippy::clone_on_copy,
        reason = "Unix 上是进程组号，Windows 上是作业对象，那边要 clone"
    )
)]
async fn bound_member(member: &OnceLock<Member>) -> Option<Member> {
    tokio::time::timeout(BIND_WAIT, async {
        loop {
            if let Some(member) = member.get() {
                return member.clone();
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .ok()
}

/// 进程 `pid` 在不在这条命令的进程组 `group` 里（Unix）。
#[cfg(unix)]
fn in_command(group: &Member, pid: u32) -> bool {
    let Some(pid) = i32::try_from(pid)
        .ok()
        .and_then(rustix::process::Pid::from_raw)
    else {
        return false;
    };
    rustix::process::getpgid(Some(pid))
        .is_ok_and(|pgid| u32::try_from(pgid.as_raw_nonzero().get()) == Ok(*group))
}

/// 进程 `pid` 在不在这条命令的作业对象 `job` 里（Windows）。
#[cfg(windows)]
fn in_command(job: &Member, pid: u32) -> bool {
    job.contains(pid)
}

/// 别的系统没有通道。
#[cfg(not(any(unix, windows)))]
fn in_command(_member: &Member, _pid: u32) -> bool {
    false
}

/// `pid` 是不是 `ancestor` 的子孙（不算它自己）：照 `parent` 一层层往上找。问不出父进程的、找到头的（0 号，或者父进程是自己）、
/// 找了 [`MOST_HOPS`] 层还没到的，不是。
pub(crate) fn descends(pid: u32, ancestor: u32, parent: impl Fn(u32) -> Option<u32>) -> bool {
    let mut at = pid;
    for _ in 0..MOST_HOPS {
        let Some(up) = parent(at) else {
            return false;
        };
        if up == ancestor {
            return true;
        }
        if up == 0 || up == at {
            return false;
        }
        at = up;
    }
    false
}

/// Windows 上 `pid` 的父进程：表里记的父进程号，还要那个进程起得不比 `pid` 晚（施工 XG-1补2）。Windows 不过继，父进程走了以后
/// 编号可能被后起的进程重用，快照里的父进程号就指着它，照创建时间认得出来。问不出时间的（不在了、打不开）当问不出父进程，链到
/// 这里断。同一刻起的算数：创建时间照系统时钟走，父子可能落在同一刻；重用编号的进程起在子进程之后，晚得出来。
#[cfg_attr(
    not(any(windows, test)),
    expect(dead_code, reason = "只有 Windows 的父进程号会被重用；测试到处跑")
)]
pub(crate) fn elder_parent(
    pid: u32,
    parent: impl Fn(u32) -> Option<u32>,
    created: impl Fn(u32) -> Option<u64>,
) -> Option<u32> {
    let up = parent(pid)?;
    (created(up)? <= created(pid)?).then_some(up)
}

/// 照这台机器问父进程（Linux）：`/proc/<pid>/stat` 的第四格。
#[cfg(target_os = "linux")]
fn descends_here(pid: u32, ancestor: u32) -> bool {
    descends(pid, ancestor, |pid| {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        linux_parent(&stat)
    })
}

/// `/proc/<pid>/stat` 里的父进程号：程序名在括号里、里面可能有空格和括号，从最后一个 `)` 往后数，第二格是它。
#[cfg_attr(
    not(any(target_os = "linux", test)),
    expect(dead_code, reason = "只有 Linux 读 /proc；测试到处跑")
)]
pub(crate) fn linux_parent(stat: &str) -> Option<u32> {
    let (_, rest) = stat.rsplit_once(')')?;
    rest.split_ascii_whitespace().nth(1)?.parse().ok()
}

/// 照这台机器问父进程（macOS）：`proc_pidinfo`。
#[cfg(target_os = "macos")]
fn descends_here(pid: u32, ancestor: u32) -> bool {
    descends(pid, ancestor, miyu_pipe::parent_process)
}

/// 照这台机器问父进程（Windows）：拍一张快照，照它找，每一层比创建时间（[`elder_parent`]）。
#[cfg(windows)]
fn descends_here(pid: u32, ancestor: u32) -> bool {
    match miyu_pipe::process_parents() {
        Ok(parents) => descends(pid, ancestor, |pid| {
            elder_parent(
                pid,
                |pid| parents.get(&pid).copied(),
                miyu_pipe::process_created,
            )
        }),
        Err(error) => {
            tracing::warn!(target: crate::TARGET, error = %error, "process snapshot not taken");
            false
        }
    }
}

/// 别的系统问不出父进程：都不是。
#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn descends_here(_pid: u32, _ancestor: u32) -> bool {
    false
}

#[cfg(test)]
mod tests;
