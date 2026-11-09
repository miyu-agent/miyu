//! `miyu bus` 给人看的字（施工 BUS-3）。

use super::Language;

impl Language {
    /// 总线上一个名字都没有。
    pub(crate) fn bus_empty(&self) -> &'static str {
        match self {
            Language::Chinese => "总线上还没有名字",
            Language::English => "no names on the bus",
        }
    }

    /// 一个名字是哪一种：`provides`、`plugs`；认不出的照原样。
    pub(crate) fn bus_kind<'a>(&self, kind: &'a str) -> &'a str {
        match (self, kind) {
            (Language::Chinese, "provides") => "提供",
            (Language::Chinese, "plugs") => "接进",
            _ => kind,
        }
    }

    /// 可不可用。
    pub(crate) fn bus_ready(&self, ready: bool) -> &'static str {
        match (self, ready) {
            (Language::Chinese, true) => "可用",
            (Language::Chinese, false) => "不可用",
            (Language::English, true) => "ready",
            (Language::English, false) => "not ready",
        }
    }
}
