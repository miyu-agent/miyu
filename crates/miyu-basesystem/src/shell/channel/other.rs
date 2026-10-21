//! 别的系统（不是 Unix、不是 Windows）没有通道：开不成，命令照旧跑。类型照另两半的写，用的地方不分平台。

use std::ffi::OsString;
use std::io;
use std::sync::Arc;

use miyu_sandbox::Sandboxed;
use miyu_tool::{NestedPort, Process};

use super::Texts;

/// 开不成的端点：没有值。
pub(in crate::shell) enum Channel {}

/// 记进程组的那一头：没有值。
pub(in crate::shell) enum Binder {}

impl Channel {
    /// 开不成。
    ///
    /// # Errors
    ///
    /// 总是：这个系统上还没有通道。
    pub(in crate::shell) fn open(
        _port: Arc<dyn NestedPort>,
        _texts: Arc<Texts>,
    ) -> io::Result<Channel> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    pub(in crate::shell) fn binder(&self) -> Binder {
        match *self {}
    }

    pub(in crate::shell) fn env(&self, _env: &mut Vec<(OsString, OsString)>) {
        match *self {}
    }

    pub(in crate::shell) fn sandboxed(&self, _sandboxed: &Sandboxed) -> Sandboxed {
        match *self {}
    }
}

impl Binder {
    pub(in crate::shell) fn bind(self, _member: crate::shell::process::Member) {
        match self {}
    }
}

/// 没有端点可带。
pub(in crate::shell) fn keep(_process: Box<dyn Process>, channel: Channel) -> Box<dyn Process> {
    match channel {}
}
