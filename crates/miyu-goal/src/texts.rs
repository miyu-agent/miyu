//! `goal` 输出里给她看的几句（`software/goal/goal/*.txt`，参数不对、没有端口的在 `software/goal/common/`），和给人看的说法的
//! 编号（`software/goal/<key>`，字在 `software/goal/human/<语言>.json`）。

use std::path::Path;

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_tool::Done;
use miyu_tool::load::{self, LoadError, say};

use crate::SOFTWARE;

/// 读好的几句。
pub(crate) struct Texts {
    pub(crate) bad_args: Template,
    pub(crate) off: Template,
    pub(crate) none: Template,
    pub(crate) now: Template,
    pub(crate) reason: Template,
    pub(crate) set: Template,
    pub(crate) exists: Template,
    pub(crate) completed: Template,
    pub(crate) blocked: Template,
    pub(crate) too_early: Template,
    pub(crate) failed: Template,
    pub(crate) wrapup_completed: Template,
    pub(crate) wrapup_blocked: Template,
}

impl Texts {
    /// 照资源目录 `resources` 读，每一份拿它要的字段试换一次。
    pub(crate) fn load(resources: &Path) -> Result<Texts, LoadError> {
        let text = |tool: &str, name: &str, fields: &[&str]| {
            load::text(resources, SOFTWARE, tool, name, fields)
        };
        let goal = |name: &str, fields: &[&str]| text("goal", name, fields);
        Ok(Texts {
            bad_args: text("common", "bad-args", &["error"])?,
            off: text("common", "off", &[])?,
            none: goal("none", &[])?,
            now: goal("now", &["objective", "state", "round"])?,
            reason: goal("reason", &["reason"])?,
            set: goal("set", &[])?,
            exists: goal("exists", &[])?,
            completed: goal("completed", &[])?,
            blocked: goal("blocked", &[])?,
            too_early: goal("too-early", &["round", "least"])?,
            failed: goal("failed", &["reason"])?,
            wrapup_completed: goal("wrapup-completed", &["objective"])?,
            wrapup_blocked: goal("wrapup-blocked", &["objective", "reason"])?,
        })
    }

    /// 一句出错：照 `template` 换进 `fields`，给人看的是 `key`、带同样的字段。
    pub(crate) fn error(template: &Template, key: &str, fields: &[(&str, &str)]) -> Done {
        Done::error(line(template, fields)).said(said(key, fields))
    }

    /// 参数不对。
    pub(crate) fn bad_args(&self, error: &dyn std::fmt::Display) -> Done {
        Texts::error(
            &self.bad_args,
            "common/bad-args",
            &[("error", &error.to_string())],
        )
    }

    /// 这个会话用不了：没有端口。
    pub(crate) fn off(&self) -> Done {
        Texts::error(&self.off, "common/off", &[])
    }
}

/// 照模板换进字段，去掉末尾的换行：几句接成几行时由调的一方加换行。
pub(crate) fn line(template: &Template, fields: &[(&str, &str)]) -> String {
    say(template, fields).trim_end().to_string()
}

/// 给人看的说法：编号是 `software/goal/<key>`，带着换进去的字段。
pub(crate) fn said(key: &str, fields: &[(&str, &str)]) -> Said {
    fields.iter().fold(
        Said::new(format!("software/{SOFTWARE}/{key}")),
        |said, (name, value)| said.with(name, (*value).to_string()),
    )
}
