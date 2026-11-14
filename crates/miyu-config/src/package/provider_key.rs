//! 要哪一家模型供应商的 key（施工 PK-1）：`[provider_key]` 一张表，读成 [`ProviderKey`]。只有一格 `catalog`：目录里那一家的
//! 编号，写法同配置里的 `providers.<id>.catalog`（[`crate::secret::valid_name`]）。只有扩展包能写（`kinds::tables`）；写了它
//! 必须在 `[process] capabilities` 里声明 `provider.key`，声明了 `provider.key` 也必须写它（同 `[[driver]]` 和 `model.driver`）。
//! 核心照它认主人配的哪一家供应商是这一家，把那一家的 key 交给批过的扩展（端点的 `extensions/provider_key.rs`）。

use toml_edit::Item;

use super::capability::Capability;
use super::reader::{Reader, line_of};
use super::{Code, Problem};

/// 扩展要的那一家供应商。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderKey {
    /// 目录里那一家的编号，例如 `deepseek`。
    pub catalog: String,
    /// 这张表写在第几行。
    pub line: Option<usize>,
}

/// `[provider_key]`：没写的没有；和 `provider.key` 能力对不上的报在这张表上，声明了能力却没写表的报在整份上。
pub(super) fn read(
    reader: &Reader<'_>,
    node: Option<&Item>,
    capabilities: &[Capability],
) -> Result<Option<ProviderKey>, Problem> {
    let declared = capabilities.contains(&Capability::ProviderKey);
    let Some(node) = node else {
        return match declared {
            true => Err(bad(
                None,
                "provider.key",
                "the provider.key capability needs a [provider_key] table".to_string(),
            )),
            false => Ok(None),
        };
    };
    let line = node.span().map(|span| line_of(reader.text, span.start));
    let Some(table) = node.as_table_like() else {
        return Err(bad(
            line,
            "provider_key",
            "provider_key must be a table".to_string(),
        ));
    };
    reader.only(table, "provider_key", super::fields::PROVIDER_KEY)?;
    let catalog = match table.get("catalog").and_then(Item::as_str) {
        Some(catalog) if crate::secret::valid_name(catalog) => catalog.to_string(),
        Some(catalog) => {
            return Err(bad(
                line,
                "provider_key.catalog",
                format!(
                    "provider_key.catalog {catalog:?} must start with a lowercase letter, use only lowercase letters, digits, - and _, at most 64"
                ),
            ));
        }
        None => {
            return Err(bad(
                line,
                "provider_key.catalog",
                "provider_key.catalog is missing or not text".to_string(),
            ));
        }
    };
    if !declared {
        return Err(bad(
            line,
            "provider_key",
            "[provider_key] needs the provider.key capability in [process] capabilities"
                .to_string(),
        ));
    }
    Ok(Some(ProviderKey { catalog, line }))
}

/// 写错了的一处。
fn bad(line: Option<usize>, key: &str, message: String) -> Problem {
    Problem {
        line,
        code: Code::BadProviderKey,
        detail: key.to_string(),
        message,
    }
}

#[cfg(test)]
mod tests;
