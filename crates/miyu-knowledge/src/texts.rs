//! 两件工具输出里给她看的几句（`software/knowledge/<工具>/*.txt`，两件都要说的在 `software/knowledge/common/`），和给人看的
//! 说法的编号（`software/knowledge/<key>`，字在 `software/knowledge/human/<语言>.json`）。同记忆那个包的写法。

use std::path::Path;
use std::sync::Arc;

use miyu_kernel::event::Said;
use miyu_kernel::template::Template;
use miyu_tool::load::{self, LoadError, say};
use miyu_tool::{Done, KbRefused};

use crate::PACKAGE;

/// 读好的几句，两件工具共用一份。
#[derive(Clone)]
pub(crate) struct Texts(Arc<All>);

pub(crate) struct All {
    pub(crate) bad_args: Template,
    pub(crate) off: Template,
    pub(crate) failed: Template,
    pub(crate) no_base: Template,
    pub(crate) no_document: Template,
    pub(crate) no_page: Template,
    pub(crate) hit: Template,
    pub(crate) plain: Template,
    pub(crate) nothing: Template,
    pub(crate) more: Template,
}

impl Texts {
    /// 照资源目录 `resources` 读，每一份拿它要的字段试换一次。
    pub(crate) fn load(resources: &Path) -> Result<Texts, LoadError> {
        let text = |tool: &str, name: &str, fields: &[&str]| {
            load::text(resources, PACKAGE, tool, name, fields)
        };
        Ok(Texts(Arc::new(All {
            bad_args: text("common", "bad-args", &["error"])?,
            off: text("common", "off", &[])?,
            failed: text("common", "failed", &["error"])?,
            no_base: text("common", "no-base", &["kb"])?,
            no_document: text("common", "no-document", &["kb", "path"])?,
            no_page: text("common", "no-page", &["page"])?,
            hit: text("kb_search", "hit", &["kb", "path", "heading"])?,
            plain: text("kb_search", "plain", &["kb", "path"])?,
            nothing: text("kb_search", "nothing", &[])?,
            more: text("kb_read", "more", &["page", "pages"])?,
        })))
    }

    /// 读好的几句。
    pub(crate) fn all(&self) -> &All {
        &self.0
    }

    /// 一句出错：照 `template` 换进 `fields`，给人看的是 `key`、带同样的字段。
    fn error(&self, template: &Template, key: &str, fields: &[(&str, &str)]) -> Done {
        Done::error(line(template, fields)).said(said(key, fields))
    }

    /// 参数不对。
    pub(crate) fn bad_args(&self, error: &dyn std::fmt::Display) -> Done {
        self.error(
            &self.0.bad_args,
            "common/bad-args",
            &[("error", &error.to_string())],
        )
    }

    /// 这个会话查不了知识库：没有端口。
    pub(crate) fn off(&self) -> Done {
        self.error(&self.0.off, "common/off", &[])
    }

    /// 端口拒了：照哪一种说一句。
    pub(crate) fn refused(&self, refused: &KbRefused) -> Done {
        match refused {
            KbRefused::NoBase(kb) => self.error(&self.0.no_base, "common/no-base", &[("kb", kb)]),
            KbRefused::NoDocument(kb, path) => self.error(
                &self.0.no_document,
                "common/no-document",
                &[("kb", kb), ("path", path)],
            ),
            KbRefused::NoPage(page) => self.error(
                &self.0.no_page,
                "common/no-page",
                &[("page", &page.to_string())],
            ),
            KbRefused::Failed(error) => {
                self.error(&self.0.failed, "common/failed", &[("error", error)])
            }
        }
    }
}

/// 照模板换进字段，去掉末尾的换行：几段接起来时由调的一方加换行。
pub(crate) fn line(template: &Template, fields: &[(&str, &str)]) -> String {
    say(template, fields).trim_end().to_string()
}

/// 给人看的说法：编号是 `software/knowledge/<key>`，带着换进去的字段。
pub(crate) fn said(key: &str, fields: &[(&str, &str)]) -> Said {
    fields.iter().fold(
        Said::new(format!("software/{PACKAGE}/{key}")),
        |said, (name, value)| said.with(name, (*value).to_string()),
    )
}
