//! `kb_delete`（施工 K-3，19 N14）：把一篇挪进系统回收站（同 `trash`）。挪成了报「挪进回收站」的效果：撤销那一轮时移回来。
//! 整个库不删（项目主人 2026-10-12 定不给她）。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, Effect, KB_DELETE, Progress, Running, Spec, Tool};

use crate::PACKAGE;
use crate::texts::{Texts, line, said};

/// `kb_delete`。
pub(crate) struct Delete {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    kb: String,
    path: String,
}

impl Delete {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Delete, LoadError> {
        Ok(Delete {
            spec: load::spec(resources, PACKAGE, KB_DELETE, Access::Write)?,
            texts,
        })
    }
}

impl Tool for Delete {
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
            let trashed = match port.delete(args.kb, args.path).await {
                Ok(trashed) => trashed,
                Err(refused) => return texts.refused(&refused),
            };
            let fields = [("kb", kb.as_str()), ("path", path.as_str())];
            Done::ok(line(&texts.all().trashed, &fields))
                .said(said("kb_delete/trashed", &fields))
                .effect(Effect::Trashed {
                    path: trashed.path,
                    trash: trashed.trash,
                })
        })
    }
}
