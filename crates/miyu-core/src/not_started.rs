//! 起不来的那一行（施工 EN-2，从 `lib.rs` 分出来）：每一步没过的都带代码，原因只写会变的那一段（系统的原话、路径），头照
//! 内核 `human/` 的 `start/<代码>` 照自己的语言说整句，`{reason}` 换成原因（`miyu_ipc::StartError::said`）。数据根认不出的
//! 那几种（施工 LC-1）原因是数据根的位置，字里写成 `{path}`。运行日志（装上了的话）只记没过的是哪一步。

use std::io::{self, Write};
use std::process::ExitCode;

use miyu_ipc::{OpenError, Ready};
use miyu_store::resources::ResourceError;
use miyu_store::root::{DataRoot, PrepareError, RootError};

use super::{TARGET, pending};

/// 核心起不来时会写的代码，每一个在内核 `human/` 三种语言里都有 `start/<代码>`（测试照它查）。
pub(crate) const CODES: &[&str] = &[
    "miyu_home_relative",
    "no_home",
    "no_local_app_data",
    "data_root_not_ours",
    "data_root_old_layout",
    "data_root_other_layout",
    "data_root_failed",
    "lock_failed",
    "log_failed",
    "resources_relative",
    "resources_missing",
    "resources_not_found",
    "resources_unknown",
    "runtime_failed",
    "socket_too_long",
    "socket_not_private",
    "socket_occupied",
    "pipe_occupied",
    "socket_failed",
    "home_failed",
    "models_failed",
    "tools_failed",
];

/// 没过的一步：代码和原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
    code: &'static str,
    reason: String,
}

impl Failure {
    /// 代码 `code`，原因 `reason`（系统或下层的原话）。
    pub(crate) fn new(code: &'static str, reason: impl ToString) -> Failure {
        debug_assert!(CODES.contains(&code), "{code} 没登记");
        Failure {
            code,
            reason: reason.to_string(),
        }
    }

    /// 找不到数据根：`MIYU_HOME` 相对的原因是写的路径。
    pub(crate) fn root(error: &RootError) -> Failure {
        match error {
            RootError::RelativeMiyuHome(path) => Failure::new("miyu_home_relative", path.display()),
            RootError::NoHome => Failure::new("no_home", ""),
            RootError::NoLocalAppData => Failure::new("no_local_app_data", ""),
        }
    }

    /// 建不成骨架：认不出的三种照 [`PrepareError::code`]，原因是数据根；读写出错的是系统的原话。
    pub(crate) fn prepare(error: &PrepareError, root: &DataRoot) -> Failure {
        match error.code() {
            Some(code) => Failure::new(code, root.path().display()),
            None => Failure::new("data_root_failed", error),
        }
    }

    /// 找不到资源目录：原因是写的、找过的位置。
    pub(crate) fn resources(error: &ResourceError) -> Failure {
        match error {
            ResourceError::Relative(path) => Failure::new("resources_relative", path.display()),
            ResourceError::Missing(path) => Failure::new("resources_missing", path.display()),
            ResourceError::NotFound(tried) if tried.is_empty() => {
                Failure::new("resources_unknown", "")
            }
            ResourceError::NotFound(tried) => {
                let tried: Vec<String> = tried.iter().map(|p| p.display().to_string()).collect();
                Failure::new("resources_not_found", tried.join(", "))
            }
        }
    }

    /// 开不了套接字：原因是位置，读写出错的是系统的原话。
    pub(crate) fn socket(error: &OpenError) -> Failure {
        match error {
            OpenError::TooLong(path) => Failure::new("socket_too_long", path.display()),
            OpenError::NotPrivate(dir) => Failure::new("socket_not_private", dir.display()),
            OpenError::Occupied(path) if cfg!(windows) => {
                Failure::new("pipe_occupied", path.display())
            }
            OpenError::Occupied(path) => Failure::new("socket_occupied", path.display()),
            OpenError::Io(error) => Failure::new("socket_failed", error),
            OpenError::Running => Failure::lock(error),
        }
    }

    /// 拿不到锁（不是别人拿着的那种）：读写出错的是系统的原话。
    pub(crate) fn lock(error: &OpenError) -> Failure {
        match error {
            OpenError::Io(error) => Failure::new("lock_failed", error),
            other => Failure::new("lock_failed", other),
        }
    }

    /// 装不上运行日志：系统的原话。
    pub(crate) fn log(error: &miyu_log::InstallError) -> Failure {
        match error {
            miyu_log::InstallError::Io(error) => Failure::new("log_failed", error),
            miyu_log::InstallError::Twice(error) => Failure::new("log_failed", error),
        }
    }

    /// 写成那一行。
    fn ready(self) -> Ready {
        Ready::Failed {
            code: Some(self.code.to_string()),
            reason: self.reason,
        }
    }
}

/// 起不来：照 `failure` 写那一行交给头，运行日志只记没过的是哪一步 `stage`（原因可能带路径，不进日志，施工 4-9 再补四上）。
/// 调它的地方接着就走、放锁，所以先关门、等已经起了的写盘线程写完（施工 TMP-1 补2；还在读目录的不等，施工 CS-8）。
pub(crate) fn failed(stage: &'static str, failure: Failure) -> ExitCode {
    pending::close();
    tracing::warn!(target: TARGET, stage, "not started");
    say(&failure.ready());
    ExitCode::FAILURE
}

/// 往标准输出写那一行：拉起核心的头在管道的另一头等着。
pub(crate) fn say(ready: &Ready) {
    let mut out = io::stdout();
    if let Err(error) = out
        .write_all(ready.line().as_bytes())
        .and_then(|()| out.flush())
    {
        tracing::warn!(target: TARGET, error = %error, "ready line not written");
    }
}

#[cfg(test)]
mod tests;
