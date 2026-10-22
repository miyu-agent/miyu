//! 模型那一块（施工 E-M，2026-10-22 项目主人定）：这一轮发给哪家供应商的哪个模型、思考强度、用的预设。她被问到自己是什么
//! 模型、要填写这些、查资料时用得上。和环境那一块一样每个边界查一遍，同类最近一块逐字节相同的不注入：换了模型、档位、预设
//! 才追加一块新的，前面的缓存不动。以前造的快照里没有这份模板，没有这一块。

use std::collections::BTreeMap;

use super::{FactTemplates, kind};
use crate::event::ContextInjected;
use crate::template::{Template, TemplateError};

/// 这一轮发给谁：执行器照这一轮的配置算好、随限额交进来（`Limits::identity`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    /// 供应商的显示名，没写的是编号。
    pub provider: String,
    /// 发给它的模型名。
    pub model: String,
    /// 思考强度：配置里写了的那一档，没写的 `default`。
    pub effort: String,
    /// 预设的编号，不用预设的 `none`。
    pub preset: String,
}

impl FactTemplates {
    /// 带上模型那一块的模板（`core/facts/model.txt`）：`None` 的不加。读好以后试换一次。
    ///
    /// # Errors
    ///
    /// 写法坏了、要了没有的字段。
    pub fn with_model(mut self, text: Option<&str>) -> Result<FactTemplates, TemplateError> {
        let template = text.map(Template::parse).transpose()?;
        if let Some(template) = &template {
            template.render(&fields(&Identity {
                provider: String::new(),
                model: String::new(),
                effort: String::new(),
                preset: String::new(),
            }))?;
        }
        self.model = template;
        Ok(self)
    }

    /// 模型那一块；没有模板的没有。供应商名是人起的，模板换字段时照常转义，写不出别的标签。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经拿全部字段试换过。
    pub fn model(&self, identity: &Identity) -> Option<ContextInjected> {
        let template = self.model.as_ref()?;
        Some(ContextInjected {
            refs: Vec::new(),
            after: false,
            kind: kind("model"),
            text: template
                .render(&fields(identity))
                .expect("造的时候试换过，字段都有"),
        })
    }
}

fn fields(identity: &Identity) -> BTreeMap<&'static str, &str> {
    BTreeMap::from([
        ("provider", identity.provider.as_str()),
        ("name", identity.model.as_str()),
        ("effort", identity.effort.as_str()),
        ("preset", identity.preset.as_str()),
    ])
}
