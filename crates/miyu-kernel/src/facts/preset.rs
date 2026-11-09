//! 换了预设那一块（施工 DM-8，2026-11-09 项目主人要的：切了预设，她那边要像切权限级别一样知道）：人换了预设以后，下一个回合
//! 开始时注入一次，写换成的和换之前的预设编号。换预设会先压缩，模型那一块里的 `preset` 在压缩以后是头一回出现，看不出换过；
//! 这一块只说「人换了」。注入过的不再注入（[`crate::session`] 的 `Reference` 照日志算还欠不欠这一块）。以前造的快照里没有这份
//! 模板，没有这一块。

use std::collections::BTreeMap;

use super::{FactTemplates, kind};
use crate::event::ContextInjected;
use crate::template::{Template, TemplateError};

/// 这一类的名字：内核记的这一类就是「告诉过她了」。
pub const PRESET: &str = "preset";

/// 不用预设（无工具）的写法，同模型那一块的 `preset`。
const NO_PRESET: &str = "none";

impl FactTemplates {
    /// 带上换了预设那一块的模板（`core/facts/preset-changed.txt`）：`None` 的不加。读好以后拿全部字段试换一次。
    ///
    /// # Errors
    ///
    /// 写法坏了、要了没有的字段。
    pub fn with_preset_changed(
        mut self,
        text: Option<&str>,
    ) -> Result<FactTemplates, TemplateError> {
        let template = text.map(Template::parse).transpose()?;
        if let Some(template) = &template {
            template.render(&fields("", ""))?;
        }
        self.preset_changed = template;
        Ok(self)
    }

    /// 换了预设那一块：换成的 `preset`、换之前的 `previous`（空的是不用预设）。没有模板的（以前造的快照）、安静的没有。
    ///
    /// # Panics
    ///
    /// 实际不会 panic：造的时候已经拿全部字段试换过；`preset` 这个类别名也合写法。
    pub fn preset_changed(&self, preset: &str, previous: &str) -> Option<ContextInjected> {
        let template = self.preset_changed.as_ref().filter(|_| !self.quiet)?;
        Some(ContextInjected {
            refs: Vec::new(),
            after: false,
            kind: kind(PRESET),
            text: template
                .render(&fields(named(preset), named(previous)))
                .expect("造的时候试换过全部字段"),
        })
    }
}

/// 预设编号的写法：空的是不用预设。
fn named(preset: &str) -> &str {
    match preset {
        "" => NO_PRESET,
        preset => preset,
    }
}

fn fields<'a>(id: &'a str, previous: &'a str) -> BTreeMap<&'static str, &'a str> {
    BTreeMap::from([("id", id), ("previous", previous)])
}
