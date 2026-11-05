//! 别的系统（不是 Unix、不是 Windows）没有通道：开不成，命令照旧跑。类型照另两半的写，用的地方不分平台。

use std::ffi::OsString;
use std::io;
use std::sync::Arc;

use miyu_sandbox::Sandboxed;
use miyu_tool::{NestedPort, Process};

use crate::Texts;
use crate::kin::{Binder, Kin};

/// 开不成的端点：没有值。
pub enum Channel {}

/// 交给扩展的那一格：没有值。
#[derive(serde::Serialize)]
pub enum Handed {}

impl Channel {
    /// 开不成。
    ///
    /// # Errors
    ///
    /// 总是：这个系统上还没有通道。
    pub fn open(_port: Arc<dyn NestedPort>, _texts: Arc<Texts>, _kin: Kin) -> io::Result<Channel> {
        Err(io::Error::from(io::ErrorKind::Unsupported))
    }

    /// 没有值。
    pub fn binder(&self) -> Option<Binder> {
        match *self {}
    }

    /// 没有值。
    pub fn env(&self, _env: &mut Vec<(OsString, OsString)>) {
        match *self {}
    }

    /// 没有值。
    pub fn handed(&self) -> Option<Handed> {
        match *self {}
    }

    /// 没有值。
    pub fn sandboxed(&self, _sandboxed: &Sandboxed) -> Sandboxed {
        match *self {}
    }
}

/// 没有端点可带。
pub fn keep(_process: Box<dyn Process>, channel: Channel) -> Box<dyn Process> {
    match channel {}
}
