//! 包自带的模型供应商（施工 PV-4，设计 `66-扩展提供模型驱动.md` 第四节第 2 条，项目主人 2026-11-17 定）：`[providers.<编号>]`
//! 一张一家，格子同配置里的 `[providers.<编号>]`。装上、开着的包带的供应商进配置的「包这一层」（核心的 `config/provided.rs`），
//! 不写配置文件；卸掉、关掉就没有。人改这一家的设置写在配置文件里、盖住包给的。
//!
//! 这里只查写法：编号同配置里的供应商编号；`name` 是三种语言的名字；`driver` 必写，内置的三种，或者 `<包编号>/<驱动名>`：驱动名
//! 要是这份清单 `[[driver]]` 里的（包编号是不是这个包由核心查，清单不知道自己的编号）；`base_url` 只能写死（不写 `{ env = … }`）；
//! 认的格只有 [`super::fields::PROVIDER`] 那几个，`key` 不收：key 是人的，人在自己那一层配。别的格（值的类型、模型手写的资料）
//! 由核心照配置清单查，查不过的那一格不收、`miyu check` 报出来。

use toml_edit::{DocumentMut, Item, Table, TableLike};

use super::reader::{Reader, line_of};
use super::{Code, DriverDecl, Problem};
use crate::phrases::Phrases;

/// 内置的驱动（配置里 `providers.<id>.driver` 的选项）。
const BUILT_IN: [&str; 3] = ["openai-chat", "anthropic", "openai-responses"];

/// 包带的一家供应商。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderDecl {
    /// 编号：配置里 `providers.<编号>` 那一段，模型写成 `<编号>/<模型>`。
    pub id: String,
    /// 名字，语言到一句；没写的是空的（照目录、编号）。
    pub name: Phrases,
    /// 驱动：内置的一种，或者 `<包编号>/<驱动名>`。
    pub driver: String,
    /// 去掉 `name` 以后的这一家，写成配置文件的样子（`[providers.<编号>]` 开头）：核心照配置清单读它。
    pub table: String,
    /// 这一张写在第几行。
    pub line: Option<usize>,
}

impl ProviderDecl {
    /// `driver` 指的扩展驱动的包编号；内置驱动没有。
    pub fn driver_package(&self) -> Option<&str> {
        self.driver.split_once('/').map(|(package, _)| package)
    }
}

/// 供应商编号的写法：同配置里「路径里的名字」（小写字母开头，小写字母、数字、`-`、`_`，最多 32 个）。
fn provider_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && id.len() <= 32
}

/// `[providers]`：底下一张一家，照编号的先后。`drivers` 是这份清单已经读好的 `[[driver]]`。
pub(super) fn read(
    reader: &Reader<'_>,
    node: &Item,
    drivers: &[DriverDecl],
) -> Result<Vec<ProviderDecl>, Problem> {
    let Some(all) = node.as_table_like() else {
        return Err(reader.problem(
            Some(node),
            Code::NotATable,
            "providers",
            "providers must be a table".to_string(),
        ));
    };
    let mut providers = Vec::new();
    for (id, item) in all.iter() {
        let key = format!("providers.{id}");
        let bad = |item: &Item, detail: &str, why: &str| {
            reader.problem(
                Some(item),
                Code::BadProvider,
                detail,
                format!("{key}: {why}"),
            )
        };
        if !provider_id(id) {
            return Err(bad(item, &key, "the id is not a valid provider id"));
        }
        let Some(table) = item.as_table_like() else {
            return Err(reader.problem(
                Some(item),
                Code::NotATable,
                &key,
                format!("{key} must be a table"),
            ));
        };
        reader.only(table, &key, super::fields::PROVIDER)?;
        let name = match table.get("name") {
            Some(name) => reader.phrases(name, &format!("{key}.name"))?,
            None => Phrases::new(),
        };
        let written = reader.required(table, item, &key, "driver")?;
        let driver = written
            .as_str()
            .ok_or_else(|| bad(written, &format!("{key}.driver"), "driver must be text"))?;
        if !fits(driver, drivers) {
            return Err(bad(
                written,
                driver,
                "driver must be a built-in driver or <this package>/<a [[driver]] of this package>",
            ));
        }
        if let Some(url) = table.get("base_url")
            && url.as_str().is_none()
        {
            return Err(bad(
                url,
                &format!("{key}.base_url"),
                "base_url must be text",
            ));
        }
        providers.push(ProviderDecl {
            id: id.to_string(),
            name,
            driver: driver.to_string(),
            table: written_out(id, table),
            line: item.span().map(|span| line_of(reader.text, span.start)),
        });
    }
    Ok(providers)
}

/// 驱动写得对不对：内置的一种，或者 `<包编号>/<驱动名>`、驱动名是这份清单里声明了的。
fn fits(driver: &str, drivers: &[DriverDecl]) -> bool {
    match driver.split_once('/') {
        None => BUILT_IN.contains(&driver),
        Some((package, name)) => {
            !package.is_empty() && drivers.iter().any(|declared| declared.name == name)
        }
    }
}

/// 去掉 `name` 的这一家写成配置文件的样子。
fn written_out(id: &str, table: &dyn TableLike) -> String {
    let mut copied = Table::new();
    for (key, item) in table.iter().filter(|(key, _)| *key != "name") {
        copied.insert(key, item.clone());
    }
    let mut providers = Table::new();
    providers.set_implicit(true);
    providers.insert(id, Item::Table(copied));
    let mut document = DocumentMut::new();
    document.insert("providers", Item::Table(providers));
    document.to_string()
}

#[cfg(test)]
mod tests;
