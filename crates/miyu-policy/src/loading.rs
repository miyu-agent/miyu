//! 要先加载的工具（施工 L-1，`docs/designs/25-工具加载.md` W9）：工具面上带了 `need_load` 的那几件，tools 数组里只给名字、
//! 说明和空的参数外壳 [`STUB`]；多一件 [`LOAD_TOOLS`]，内核当场答（`miyu_kernel::tool::Loading`）。用的字随快照冻着
//! （[`LoadingTexts`]，`resources/core/loading/`）：核心升级改了字，老会话照样逐字节重现当时的请求。

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use miyu_kernel::raw::RawJson;
use miyu_kernel::request::ToolSpec;
use miyu_kernel::template::TemplateError;
use miyu_kernel::tool::{Access, Contract, Loading, LoadingSources, ToolRule};

use crate::tools::ToolEntry;

/// 加载的那件工具的名字。
pub const LOAD_TOOLS: &str = "load_tools";

/// 要先加载的工具在 tools 数组里的参数：空的外壳。
pub const STUB: &str = r#"{"type":"object"}"#;

/// 要先加载的工具用的字（`resources/core/loading/`）。以前造的快照里没有：那时还没有要先加载的工具。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadingTexts {
    /// `load_tools` 的说明（`load_tools.json` 的 `description`）。
    pub description: String,
    /// `load_tools` 的参数格式（`load_tools.json` 的 `parameters`），原样。
    pub parameters: RawJson,
    /// 要的名字里有不用加载、不存在的（`unknown.txt`，字段 `names`）。
    pub unknown: String,
    /// 参数里没有名字的列表（`bad-args.txt`）。
    pub bad_args: String,
    /// 没加载直接调、出错了，附在完整参数格式前面的那一句（`contract.txt`，字段 `name`）。
    pub contract: String,
}

/// 工具面上有要先加载的：把它们的参数换成外壳，加上 `load_tools`（照名字排进去），交回给内核的 [`Loading`]。没有要先加载的、
/// 快照里没有这几句字的，什么都不动，交回没有：字节和以前一样。
///
/// # Errors
///
/// 模板的写法坏了。
pub(crate) fn stub(
    tools: &[ToolEntry],
    texts: Option<&LoadingTexts>,
    face: &mut Vec<ToolSpec>,
    rules: &mut BTreeMap<String, ToolRule>,
) -> Result<Option<Loading>, TemplateError> {
    let Some(texts) = texts.filter(|_| tools.iter().any(|tool| tool.need_load)) else {
        return Ok(None);
    };
    let shell: RawJson = serde_json::from_str(STUB).expect("写死的外壳合 JSON");
    let mut contracts = BTreeMap::new();
    for tool in tools.iter().filter(|tool| tool.need_load) {
        if let Some(spec) = face.iter_mut().find(|spec| spec.name == tool.name) {
            spec.parameters = shell.clone();
        }
        contracts.insert(
            tool.name.clone(),
            Contract {
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            },
        );
    }
    rules.insert(
        LOAD_TOOLS.to_string(),
        ToolRule {
            access: Access::Read,
            parameters: texts.parameters.clone(),
            clicall: false,
        },
    );
    face.push(ToolSpec {
        name: LOAD_TOOLS.to_string(),
        description: texts.description.clone(),
        parameters: texts.parameters.clone(),
    });
    face.sort_by(|one, other| one.name.cmp(&other.name));
    Loading::new(
        LOAD_TOOLS.to_string(),
        contracts,
        LoadingSources {
            unknown: &texts.unknown,
            bad_args: &texts.bad_args,
            contract: &texts.contract,
        },
    )
    .map(Some)
}

/// 发给模型的一件工具（施工 S-T，`session.tools`）：名字，和是不是只露名字、要先加载（`loaded` 是假的）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// 工具名。
    pub name: String,
    /// 是假的：只露名字，要先加载。
    pub loaded: bool,
}

/// 工具面 `tools` 照 `stub` 发出去的样子：照发的先后（名字的字节序），要先加载的那几件 `loaded` 是假的，有它们、又有
/// `texts` 的多一件 [`LOAD_TOOLS`]。和 `stub` 走同一条路，不另算。
pub fn offered(tools: &[ToolEntry], texts: Option<&LoadingTexts>) -> Vec<Offered> {
    let Ok((mut face, mut rules)) = crate::tools::split(tools) else {
        return Vec::new();
    };
    let stubbed = matches!(stub(tools, texts, &mut face, &mut rules), Ok(Some(_)));
    face.sort_by(|one, other| one.name.cmp(&other.name));
    face.into_iter()
        .map(|spec| {
            let loaded = !stubbed
                || tools
                    .iter()
                    .find(|tool| tool.name == spec.name)
                    .is_none_or(|tool| !tool.need_load);
            Offered {
                name: spec.name,
                loaded,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests;
