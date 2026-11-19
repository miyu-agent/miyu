//! 核心起不来、头连不上的几种情形。

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use miyu_kernel::event::Said;
use miyu_store::root::DataRoot;

/// 核心起不来。
#[derive(Debug)]
pub enum OpenError {
    /// 这个数据根上已经有一个核心在运行：`run/core.lock` 在它手里。
    Running,
    /// 套接字的路径太长：本该放的位置、临时目录下都放不下。带着本该放的位置。
    TooLong(PathBuf),
    /// 套接字要放的目录不是自己的，或者别人也能进：不用它。
    NotPrivate(PathBuf),
    /// 套接字的位置上有别的东西：不是套接字的文件，或者别的程序正在听；Windows 上是管道名被别的程序
    /// 占了。不动它。
    Occupied(PathBuf),
    /// 读写出错。
    Io(io::Error),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OpenError::Running => write!(f, "这个数据根上已经有一个核心在运行"),
            OpenError::TooLong(path) => write!(
                f,
                "套接字的路径太长，放不下：{}；临时目录下也放不下",
                path.display()
            ),
            OpenError::NotPrivate(dir) => write!(
                f,
                "{} 不属于当前用户，或者其他用户也能访问：套接字不放在这里",
                dir.display()
            ),
            OpenError::Occupied(path) => f.write_str(&occupied(path, cfg!(windows))),
            OpenError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for OpenError {}

/// [`OpenError::Occupied`] 的说法：Windows 上是命名管道（`pipe`），名字被别的程序占了；别处是套接字的位置上有别的东西
/// （施工 WN-1：原来 Windows 上也说「不是套接字的文件」）。
fn occupied(path: &Path, pipe: bool) -> String {
    match pipe {
        true => format!("命名管道 {} 已经被其他程序占用。没有修改它", path.display()),
        false => format!(
            "{} 上有其他内容：不是套接字的文件，或者其他程序正在监听。没有修改它",
            path.display()
        ),
    }
}

/// [`ConnectError::NotPrivate`] 的说法：Windows 上是命名管道（`pipe`）另一头不是自己的进程；别处是套接字所在的目录不是自己的
/// （施工 WN-1：原来 Windows 上也说「别人也能进」）。
fn not_private(place: &Path, pipe: bool) -> String {
    match pipe {
        true => format!(
            "命名管道 {} 另一头不是当前用户的进程：不连接",
            place.display()
        ),
        false => format!(
            "{} 不属于当前用户，或者其他用户也能访问：不连接",
            place.display()
        ),
    }
}

impl From<io::Error> for OpenError {
    fn from(error: io::Error) -> OpenError {
        OpenError::Io(error)
    }
}

/// 头连不上核心。
#[derive(Debug)]
pub enum ConnectError {
    /// 核心没在跑：`run/socket` 没有，或者那里连不上。
    NotRunning,
    /// 套接字所在的目录不是自己的，或者别人也能进；Windows 上是管道另一头不是自己的进程。不连，免得把
    /// 本机令牌交给冒充核心的人。
    NotPrivate(PathBuf),
    /// 读写出错。
    Io(io::Error),
}

impl fmt::Display for ConnectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectError::NotRunning => write!(f, "核心没有在运行"),
            ConnectError::NotPrivate(place) => f.write_str(&not_private(place, cfg!(windows))),
            ConnectError::Io(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for ConnectError {}

impl ConnectError {
    /// 给人看的说法（施工 EN-1）：头照内核的 `start/<代码>` 照自己的语言说；[`fmt::Display`] 的中文原话留给日志和取不到字的
    /// 时候。读写出错的没有，照系统给的原话。
    #[must_use]
    pub fn said(&self) -> Option<Said> {
        match self {
            ConnectError::NotRunning => Some(Said::new("core/start/not_running")),
            ConnectError::NotPrivate(place) => Some(not_private_said(place, cfg!(windows))),
            ConnectError::Io(_) => None,
        }
    }
}

/// [`not_private`] 的说法：命名管道、套接字各一句。
fn not_private_said(place: &Path, pipe: bool) -> Said {
    let key = match pipe {
        true => "core/start/not_private_pipe",
        false => "core/start/not_private_socket",
    };
    Said::new(key).with("path", place.display().to_string())
}

impl From<io::Error> for ConnectError {
    fn from(error: io::Error) -> ConnectError {
        ConnectError::Io(error)
    }
}

/// 头拉起核心没成。
#[derive(Debug)]
pub enum StartError {
    /// 核心起不来，它说了原因（照原样给人看）。认得出是哪一种的带代码（施工 LC-1，[`crate::Ready::Failed`]）：头照
    /// `start/<代码>` 取给人看的字，照自己的语言说。
    Refused {
        /// 代码，没有的是 `None`。
        code: Option<String>,
        /// 原因。
        reason: String,
    },
    /// 核心没说好了就退了：原因在它的运行日志里（数据根的 `var/log/core.log`，[`DataRoot::core_log`]）。
    Silent,
    /// 等核心说好了，等太久了。
    Timeout,
    /// 别的头正在拉起核心，等太久了。
    Busy,
    /// 核心是人停的（施工 CS-5，`run/stopped`）：头自己重连（[`crate::reconnect_or_start`]）不拉，等人点「启动」。
    StoppedByUser,
    /// 连不上。
    Connect(ConnectError),
    /// 读写出错，例如拉不起来：找不到程序。
    Io(io::Error),
}

impl fmt::Display for StartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            // 带代码的原因只是会变的那一段（施工 EN-2），整句由头照 `start/<代码>` 说；取不到字的照代码和原因说。
            StartError::Refused {
                code: Some(code),
                reason,
            } => write!(f, "核心无法启动（{code}）：{reason}"),
            StartError::Refused { reason, .. } => write!(f, "核心无法启动：{reason}"),
            // 照数据根取的相对位置（施工 FX-4：原来写死 `state/logs/core.log`，FH-3 挪到 `var/log/` 以后指错了地方）。
            StartError::Silent => write!(
                f,
                "核心没有启动，也没有给出原因：查看数据根的 {}",
                DataRoot::at(PathBuf::new()).core_log().display()
            ),
            StartError::Timeout => write!(f, "等待核心启动超时"),
            StartError::Busy => write!(f, "其他程序正在启动核心，等待超时"),
            StartError::StoppedByUser => write!(f, "核心已停止"),
            StartError::Connect(error) => error.fmt(f),
            StartError::Io(error) => write!(f, "无法启动核心：{error}"),
        }
    }
}

impl std::error::Error for StartError {}

impl StartError {
    /// 给人看的说法（施工 EN-1）：同 [`ConnectError::said`]。核心说了原因的：带代码的照 `start/<代码>`（施工 EN-2：每一步都带），
    /// `{reason}` 是原因（数据根认不出的那几种原因是数据根的位置，字里写成 `{path}`，两个字段都给）；没代码的照
    /// `start/refused` 带上原话。日志的位置照数据根取相对的。
    #[must_use]
    pub fn said(&self) -> Option<Said> {
        Some(match self {
            StartError::Refused {
                code: Some(code),
                reason,
            } => Said::new(format!("core/start/{code}"))
                .with("reason", reason.as_str())
                .with("path", reason.as_str()),
            StartError::Refused { reason, .. } => {
                Said::new("core/start/refused").with("reason", reason.as_str())
            }
            StartError::Silent => Said::new("core/start/silent").with(
                "log",
                DataRoot::at(PathBuf::new())
                    .core_log()
                    .display()
                    .to_string(),
            ),
            StartError::Timeout => Said::new("core/start/timeout"),
            StartError::Busy => Said::new("core/start/busy"),
            StartError::StoppedByUser => Said::new("core/start/stopped_by_user"),
            StartError::Connect(error) => return error.said(),
            StartError::Io(error) => {
                Said::new("core/start/spawn_failed").with("error", error.to_string())
            }
        })
    }
}

impl From<io::Error> for StartError {
    fn from(error: io::Error) -> StartError {
        StartError::Io(error)
    }
}

#[cfg(test)]
mod tests;
