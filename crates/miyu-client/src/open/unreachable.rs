//! 连不上、拉不起核心（施工 EN-2）：[`Refused`] 的原话照这台机器的语言（[`locale`]：`zh`、`ja` 开头的照它，别的英文）从资源
//! 目录的字里取（`miyu_ipc` 的错交的说法，核心起不来的照它那一行的代码），取不到的照底层的原话。接界面的程序（迁移、网页、
//! 接入通讯平台）经 [`Core::connect`](super::Core::connect) 这几个口子连核心，不用各自翻译。

use std::fmt::Display;

use miyu_kernel::event::Said;
use miyu_store::env::Env;
use miyu_store::human::Human;
use miyu_store::resources::ResourceRoot;

use super::{Refused, locale};

/// 连不上：原话照语言说。
pub(super) fn unreachable(said: Option<Said>, error: impl Display) -> Refused {
    let words = ResourceRoot::locate(&Env::current())
        .ok()
        .and_then(|resources| Human::load(&resources, language(&locale())).ok());
    Refused::local("unreachable", spoken(said, &error, words.as_ref()))
}

/// 照 `words` 说 `said`，取不到的照 `error` 的原话。
fn spoken(said: Option<Said>, error: &dyn Display, words: Option<&Human>) -> String {
    said.and_then(|said| words?.say(&said))
        .unwrap_or_else(|| error.to_string())
}

/// 系统的语言 `locale` 读哪一份字。
fn language(locale: &str) -> &'static str {
    match locale {
        locale if locale.starts_with("zh") => "zh",
        locale if locale.starts_with("ja") => "ja",
        _ => "en",
    }
}

#[cfg(test)]
mod tests;
