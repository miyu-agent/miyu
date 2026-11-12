//! 清单里一项能写在哪几层、什么时候生效怎么读（施工 9-1 下；施工 CPU-1 加 `restart` 时从 `settings.rs` 分出来，行数）。

use toml_edit::Item as Node;

use super::super::reader::Reader;
use super::super::{Code, Problem};
use crate::item::{Applies, Layer};

/// `layers`：只认 `system`。
pub(super) fn layers(reader: &Reader<'_>, at: &str, item: &Node) -> Result<Vec<Layer>, Problem> {
    let bad = || {
        reader.problem(
            Some(item),
            Code::BadLayers,
            &format!("{at}.layers"),
            format!("{at}.layers must list system"),
        )
    };
    let array = item.as_array().ok_or_else(bad)?;
    let mut layers = Vec::new();
    for value in array {
        let layer = match value.as_str() {
            Some("system") => Layer::System,
            _ => return Err(bad()),
        };
        if layers.contains(&layer) {
            return Err(bad());
        }
        layers.push(layer);
    }
    if layers.is_empty() {
        return Err(bad());
    }
    layers.sort();
    Ok(layers)
}

/// `applies`：`now`、`new_session`、`next_turn`、`restart`（重启核心，施工 CPU-1）、`program_start`（这个程序下次启动时）。
pub(super) fn applies(reader: &Reader<'_>, at: &str, item: &Node) -> Result<Applies, Problem> {
    Ok(match item.as_str() {
        Some("now") => Applies::Now,
        Some("new_session") => Applies::NewSession,
        Some("next_turn") => Applies::NextTurn,
        Some("restart") => Applies::Restart,
        Some("program_start") => Applies::HeadStart,
        _ => {
            return Err(reader.problem(
                Some(item),
                Code::BadApplies,
                &format!("{at}.applies"),
                format!(
                    "{at}.applies must be now, new_session, next_turn, restart or program_start"
                ),
            ));
        }
    })
}
