//! `kb_read`（施工 K-1 下）：读一个文档的一页。不止一页的，末尾另起一行说第几页、一共几页，她照它接着读；只有一页的只给字。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, KB_READ, Progress, Running, Spec, Tool};

use crate::PACKAGE;
use crate::texts::{Texts, line, said};

/// `kb_read`。
pub(crate) struct Read {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    kb: String,
    path: String,
    #[serde(default)]
    page: Option<usize>,
}

impl Read {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Read, LoadError> {
        Ok(Read {
            spec: load::spec(resources, PACKAGE, KB_READ, Access::Read)?,
            texts,
        })
    }
}

impl Tool for Read {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.bad_args(&error),
            };
            let Some(port) = call.knowledge.as_ref() else {
                return texts.off();
            };
            let path = args.path.clone();
            let page = match port.read(args.kb, args.path, args.page.unwrap_or(1)).await {
                Ok(page) => page,
                Err(refused) => return texts.refused(&refused),
            };
            let (number, pages) = (page.page.to_string(), page.pages.to_string());
            let text = match page.pages > 1 {
                true => {
                    let more = line(&texts.all().more, &[("page", &number), ("pages", &pages)]);
                    format!("{}\n{more}", page.text.trim_end())
                }
                false => page.text,
            };
            Done::ok(text).said(said(
                "kb_read/page",
                &[("path", &path), ("page", &number), ("pages", &pages)],
            ))
        })
    }
}
