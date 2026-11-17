//! 连上核心以前出的错照界面语言说（施工 EN-1）：找数据根、建骨架、连核心、拉起核心出的错，底层（`miyu-store`、`miyu-ipc`）
//! 拿不到界面语言，只交说法（内核的 `start/<代码>`）；这里照握手以前的界面语言（系统的语言，[`language::current`]）从资源
//! 目录的字里取。取不到的（没有说法、资源目录找不到）照底层的原话。

use std::fmt::Display;

use miyu_ipc::{ConnectError, StartError};
use miyu_kernel::event::Said;
use miyu_store::env::Env;
use miyu_store::root::{PrepareError, RootError};

use crate::language;

/// 连上核心以前出的一种错：底层交的说法。
pub(crate) trait Early: Display {
    /// 给人看的说法，没有的照原话。
    fn said(&self) -> Option<Said>;
}

impl Early for RootError {
    fn said(&self) -> Option<Said> {
        Some(RootError::said(self))
    }
}

impl Early for PrepareError {
    fn said(&self) -> Option<Said> {
        PrepareError::said(self)
    }
}

impl Early for ConnectError {
    fn said(&self) -> Option<Said> {
        ConnectError::said(self)
    }
}

impl Early for StartError {
    fn said(&self) -> Option<Said> {
        StartError::said(self)
    }
}

/// 照握手以前的界面语言说 `error`。
pub(crate) fn spoken(error: &dyn Early) -> String {
    let language = language::current();
    error
        .said()
        .and_then(|said| crate::ask::human(&Env::current(), &language).say(&said))
        .unwrap_or_else(|| error.to_string())
}

#[cfg(test)]
mod tests;
