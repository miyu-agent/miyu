//! `kb_edit`（施工 K-3，19 N14）：照原文换一段，`old` 得在这一篇里正好出现一次（同 `edit`）。写成了报「改了文件」的效果：撤销
//! 那一轮时改回去。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, Effect, KB_EDIT, Progress, Running, Spec, Tool};

use crate::PACKAGE;
use crate::texts::{Texts, line, said};

/// `kb_edit`。
pub(crate) struct Edit {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    kb: String,
    path: String,
    old: String,
    new: String,
}

impl Edit {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Edit, LoadError> {
        Ok(Edit {
            spec: load::spec(resources, PACKAGE, KB_EDIT, Access::Write)?,
            texts,
        })
    }
}

impl Tool for Edit {
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
            let (kb, path) = (args.kb.clone(), args.path.clone());
            let written = match port.edit(args.kb, args.path, args.old, args.new).await {
                Ok(written) => written,
                Err(refused) => return texts.refused(&refused),
            };
            let fields = [("kb", kb.as_str()), ("path", path.as_str())];
            Done::ok(line(&texts.all().edited, &fields))
                .said(said("kb_edit/edited", &fields))
                .effect(Effect::Changed {
                    path: written.path,
                    before: written.before,
                    after: written.after,
                })
        })
    }
}
