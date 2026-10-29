//! `kb_save`（施工 K-3，19 N14）：新建一篇，或者接在一篇后面；写到还没有的库就是建这个库，`about` 写进它的 `kb.toml`。写成了
//! 报「改了文件」的效果（改前改后的内容）：撤销那一轮时改回去。

use std::path::Path;

use serde::Deserialize;

use miyu_kernel::tool::Access;
use miyu_tool::load::{self, LoadError};
use miyu_tool::{Call, Done, Effect, KB_SAVE, KbSave, Progress, Running, Spec, Tool};

use crate::SOFTWARE;
use crate::texts::{Texts, line, said};

/// `kb_save`。
pub(crate) struct Save {
    spec: Spec,
    texts: Texts,
}

/// 她给的参数。
#[derive(Deserialize)]
struct Args {
    kb: String,
    path: String,
    text: String,
    #[serde(default)]
    append: bool,
    #[serde(default)]
    about: Option<String>,
}

impl Save {
    /// 照资源目录 `resources` 里的说明造，几句是 `texts`。
    pub(crate) fn load(resources: &Path, texts: Texts) -> Result<Save, LoadError> {
        Ok(Save {
            spec: load::spec(resources, SOFTWARE, KB_SAVE, Access::Write)?,
            texts,
        })
    }
}

impl Tool for Save {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        Box::pin(async move {
            let texts = &self.texts;
            let all = texts.all();
            let args = match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => args,
                Err(error) => return texts.bad_args(&error),
            };
            let Some(port) = call.knowledge.as_ref() else {
                return texts.off();
            };
            let (kb, path, append) = (args.kb.clone(), args.path.clone(), args.append);
            let save = KbSave {
                kb: args.kb,
                path: args.path,
                text: args.text,
                append,
                about: args.about,
            };
            let written = match port.save(save).await {
                Ok(written) => written,
                Err(refused) => return texts.refused(&refused),
            };
            let (template, key) = match (written.created, append) {
                (true, _) => (&all.created, "kb_save/created"),
                (false, true) => (&all.appended, "kb_save/appended"),
                (false, false) => (&all.saved, "kb_save/saved"),
            };
            let fields = [("kb", kb.as_str()), ("path", path.as_str())];
            Done::ok(line(template, &fields))
                .said(said(key, &fields))
                .effect(Effect::Changed {
                    path: written.path,
                    before: written.before,
                    after: written.after,
                })
        })
    }
}
