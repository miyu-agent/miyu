//! 用途给人看的名字（施工 UP-3，`internal/blueprint/models.md`「协议」`usage.query`）：`usage.query` 照 `purpose` 分组时每一行带
//! `purpose_name`，照连接的语言。
//!
//! - 核心自己的用途照字表 `core/human` 的 `usage/purpose/<用途>` 给（有哪几种就看字表里有哪几条，不另列名单）；没写用途的那一行
//!   （主请求，连同子代理的、当场压缩的）照 `usage/purpose/main` 给。
//! - 扩展写的照包清单 `[[purpose]]` 声明的名字给：几个包声明了同一个的，照包的编号排第一的算；核心自己有的不让扩展改名。
//! - 都没有的不写，头照 `purpose` 原样印。

use std::collections::BTreeMap;

use miyu_config::Words;
use miyu_config::package::UsagePurpose;
use miyu_store::human::Human;
use serde_json::{Value, json};

use crate::Core;
use crate::refusal::Refusal;

/// 字表里核心自己的用途的名字在哪一格：`usage/purpose/<用途>`。
const PREFIX: &str = "usage/purpose/";

/// 没写用途的那一行照这一格的名字。
const MAIN: &str = "main";

/// 一次查询用的名字：字表、包声明的、连接的语言。
pub(super) struct Names {
    human: Human,
    declared: BTreeMap<String, miyu_config::phrases::Phrases>,
    language: String,
}

impl Names {
    /// 照这一刻装着的包、连接的语言 `language` 读好。
    ///
    /// # Errors
    ///
    /// 字表读不了：`internal`（同 `model.list` 的 `side`，记一行 `WARN resource unreadable`）。
    pub(super) fn load(core: &Core, language: &str) -> Result<Names, Refusal> {
        let human = crate::config::methods::words(core, language)?;
        let mut packages: Vec<(String, Vec<UsagePurpose>)> = core
            .system_packages()
            .iter()
            .filter_map(|one| Some((one.id.clone(), one.read.as_ref().ok()?.purposes.clone())))
            .collect();
        packages.sort_by(|a, b| a.0.cmp(&b.0));
        let mut declared = BTreeMap::new();
        for purpose in packages.into_iter().flat_map(|(_, purposes)| purposes) {
            declared.entry(purpose.id).or_insert(purpose.name);
        }
        Ok(Names {
            human,
            declared,
            language: language.to_string(),
        })
    }

    /// 用途 `purpose`（没写的是 `None`）给人看的名字；没有的没有。
    pub(super) fn of(&self, purpose: Option<&str>) -> Option<String> {
        let purpose = purpose.unwrap_or(MAIN);
        self.human
            .sentence(&format!("{PREFIX}{purpose}"), &[])
            .or_else(|| crate::personas::pick(self.declared.get(purpose)?, &self.language))
    }

    /// 照 `purpose` 分组的一行（`row`）带上 `purpose_name`；没有名字的不写。
    pub(super) fn name(&self, row: &mut Value) {
        let purpose = row.get("purpose").and_then(Value::as_str);
        if let Some(name) = self.of(purpose) {
            row["purpose_name"] = json!(name);
        }
    }
}
