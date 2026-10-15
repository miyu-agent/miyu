//! 要先加载的工具（施工 L-1，`docs/designs/25-工具加载.md` W9）：登记时带了 `need_load` 的，tools 数组里只有名字、说明和
//! 空的参数外壳。她调 `load_tools` 时内核当场答，交回这几件的完整说明和参数格式；没加载直接调、出错了的，结果后面附上
//! 这一件的完整参数格式。都是纯的：照快照里冻着的格式算，不碰目录、不碰磁盘，回放一字不差。
//!
//! 完整格式照原样的 JSON 交，不过模板：模板会把字段里的引号转义掉（[`crate::template`]）。说明哪几个名字不用加载的那一句、
//! 附在出错后面的那一句开头才是模板。

use std::collections::BTreeMap;

use serde::Deserialize;

use super::Worded;
use crate::event::Said;
use crate::raw::RawJson;
use crate::template::{Template, TemplateError};

/// 一件要先加载的工具的完整格式。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contract {
    /// 给模型看的说明，整段。
    pub description: String,
    /// 参数的 JSON Schema，原样。
    pub parameters: RawJson,
}

/// 几句字的原文（`resources/core/loading/`）。
#[derive(Debug, Clone, Copy)]
pub struct LoadingSources<'a> {
    /// 要的名字里有不用加载、不存在的：字段 `names`。
    pub unknown: &'a str,
    /// 参数里没有名字的列表。
    pub bad_args: &'a str,
    /// 没加载直接调、出错了，附在后面的那一句的开头：字段 `name`。
    pub contract: &'a str,
}

/// 要先加载的那几件，和答 `load_tools` 的几句。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loading {
    /// 加载的那件工具叫什么（`load_tools`）：内核照这个名字当场答。
    pub tool: String,
    /// 要先加载的每一件：名字到完整格式。
    pub contracts: BTreeMap<String, Contract>,
    unknown: Template,
    bad_args: Template,
    contract: Template,
}

/// `load_tools` 的参数。
#[derive(Deserialize)]
struct Args {
    tools: Vec<String>,
}

impl Loading {
    /// 造一份：模板照原文读。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了。
    pub fn new(
        tool: String,
        contracts: BTreeMap<String, Contract>,
        sources: LoadingSources<'_>,
    ) -> Result<Loading, TemplateError> {
        Ok(Loading {
            tool,
            contracts,
            unknown: Template::parse(sources.unknown)?,
            bad_args: Template::parse(sources.bad_args)?,
            contract: Template::parse(sources.contract)?,
        })
    }

    /// 答一次 `load_tools`，参数是 `args`（一个 JSON 对象的原文）。交回成没成、写成的一句：成了的是要的那几件的完整格式，
    /// 照要的先后、重复的只交一次，一行一件 `{"name","description","parameters"}`；要的名字里有不用加载、不存在的，最后
    /// 一句说是哪几个。参数里没有名字的列表的，不成。
    pub fn answer(&self, args: &str) -> (bool, Worded) {
        let Ok(Args { tools }) = serde_json::from_str::<Args>(args) else {
            let text = self.bad_args.render(&BTreeMap::new()).unwrap_or_default();
            let said = Said::new("core/loading/bad-args");
            return (false, worded(text, said));
        };
        let mut loaded: Vec<&str> = Vec::new();
        let mut unknown: Vec<&str> = Vec::new();
        for name in &tools {
            let list = match self.contracts.contains_key(name) {
                true => &mut loaded,
                false => &mut unknown,
            };
            if !list.contains(&name.as_str()) {
                list.push(name);
            }
        }
        let mut text = String::new();
        for name in &loaded {
            let contract = &self.contracts[*name];
            text.push_str(&line(name, contract));
            text.push('\n');
        }
        if !unknown.is_empty() {
            let names = unknown.join(", ");
            let fields = BTreeMap::from([("names", names.as_str())]);
            text.push_str(&self.unknown.render(&fields).unwrap_or_default());
        }
        let said = Said::new("core/loading/loaded").with("tools", loaded.join(", "));
        (true, worded(text, said))
    }

    /// 叫 `name` 的工具没加载直接调、出错了：附在结果后面的那一句和它的完整参数格式。不是要先加载的没有。
    pub fn contract_of(&self, name: &str) -> Option<String> {
        let contract = self.contracts.get(name)?;
        let fields = BTreeMap::from([("name", name)]);
        let lead = self.contract.render(&fields).unwrap_or_default();
        Some(format!("{lead}{}\n", contract.parameters.get()))
    }
}

/// 一件的完整格式写成一行 JSON：名字、说明照 JSON 的字符串写，参数照原样。
fn line(name: &str, contract: &Contract) -> String {
    let quoted = |text: &str| serde_json::to_string(text).unwrap_or_default();
    format!(
        r#"{{"name":{},"description":{},"parameters":{}}}"#,
        quoted(name),
        quoted(&contract.description),
        contract.parameters.get()
    )
}

fn worded(text: String, said: Said) -> Worded {
    Worded {
        text,
        said: Some(said),
    }
}

#[cfg(test)]
mod tests;
