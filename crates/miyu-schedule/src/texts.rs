//! 给她看的几句（`software/<包>/<工具>/*.txt`，参数不对、没有端口的在 `software/<包>/common/`），和给人看的说法的编号
//! （`software/<包>/<key>`，字在 `software/<包>/human/<语言>.json`）。

use std::path::Path;

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_kernel::time::{Timestamp, UtcOffset};
use miyu_tool::Done;
use miyu_tool::load::{self, LoadError, say};

/// 一个包的字从哪读、给人看的编号带哪个前缀。
#[derive(Clone, Copy)]
pub(crate) struct Package {
    pub(crate) id: &'static str,
}

impl Package {
    /// 读 `tool` 下的一句 `name`，拿它要的字段试换一次。
    pub(crate) fn text(
        self,
        resources: &Path,
        (tool, name): (&str, &str),
        fields: &[&str],
    ) -> Result<Template, LoadError> {
        load::text(resources, self.id, tool, name, fields)
    }

    /// 一句出错：照 `template` 换进 `fields`，给人看的是 `key`、带同样的字段。
    pub(crate) fn error(self, template: &Template, key: &str, fields: &[(&str, &str)]) -> Done {
        Done::error(line(template, fields)).said(self.said(key, fields))
    }

    /// 一句成了：同上。
    pub(crate) fn ok(self, template: &Template, key: &str, fields: &[(&str, &str)]) -> Done {
        Done::ok(line(template, fields)).said(self.said(key, fields))
    }

    /// 给人看的说法：编号是 `software/<包>/<key>`，带着换进去的字段。
    pub(crate) fn said(self, key: &str, fields: &[(&str, &str)]) -> Said {
        fields.iter().fold(
            Said::new(format!("software/{}/{key}", self.id)),
            |said, (name, value)| said.with(name, (*value).to_string()),
        )
    }
}

/// 两个包都有的两句：参数不对、这个会话用不了。
pub(crate) struct Common {
    pub(crate) package: Package,
    bad_args: Template,
    off: Template,
}

impl Common {
    /// 照资源目录 `resources` 读包 `package` 的。
    pub(crate) fn load(resources: &Path, package: Package) -> Result<Common, LoadError> {
        Ok(Common {
            package,
            bad_args: package.text(resources, ("common", "bad-args"), &["error"])?,
            off: package.text(resources, ("common", "off"), &[])?,
        })
    }

    /// 参数不对。
    pub(crate) fn bad_args(&self, error: &dyn std::fmt::Display) -> Done {
        let error = error.to_string();
        self.package
            .error(&self.bad_args, "common/bad-args", &[("error", &error)])
    }

    /// 这个会话用不了：没有端口、唤醒表用不了。
    pub(crate) fn off(&self) -> Done {
        self.package.error(&self.off, "common/off", &[])
    }
}

/// 照模板换进字段，去掉末尾的换行：几句接成几行时由调的一方加换行。
pub(crate) fn line(template: &Template, fields: &[(&str, &str)]) -> String {
    say(template, fields).trim_end().to_string()
}

/// 下一次的时刻照会话的时区写到分钟：`2026-10-27 09:00`。
pub(crate) fn clock(at: Timestamp, offset: UtcOffset) -> String {
    at.local_minute(offset)
}
