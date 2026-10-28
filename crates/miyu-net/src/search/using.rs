//! 这一次搜用得上的（施工 N-2 下、N-3 上）：设置里挑的那一家、配了的 key、SearXNG 的地址。工具照这一轮的配置拼好交给
//! [`super::Search::search`]（`crate::settings::using`）；测试照手写的。

use std::collections::BTreeMap;

/// 这一次用得上的。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Using {
    /// 设置里挑的那一家：`auto`、没有的照出厂的顺序。
    pub preferred: Option<String>,
    /// 配了 key 的：来源的编号 → key。
    pub keys: BTreeMap<String, String>,
    /// SearXNG 实例的地址。
    pub address: Option<String>,
}

impl Using {
    /// 只挑了一家，什么都没配。
    pub fn picked(preferred: &str) -> Using {
        Using {
            preferred: Some(preferred.to_string()),
            ..Using::default()
        }
    }

    /// 再配一家的 key。
    #[must_use]
    pub fn with_key(mut self, source: &str, key: &str) -> Using {
        self.keys.insert(source.to_string(), key.to_string());
        self
    }

    /// 再配 SearXNG 的地址。
    #[must_use]
    pub fn with_address(mut self, address: &str) -> Using {
        self.address = Some(address.to_string());
        self
    }

    /// 来源 `source` 配的 key。
    pub(crate) fn key(&self, source: &str) -> Option<&str> {
        self.keys.get(source).map(String::as_str)
    }
}
