//! `show`（施工 D-5，`docs/blueprint/tools/show.md`，设计 `35-展示.md`）：把一个文件给人看。照 `read` 的规矩读（一层链接都
//! 不跟、最多 20 MiB），认种类（[`kind`]），报 `file.shown`：内容由执行器存成 blob，头照它画。她只收到一句「已展示给用户」，
//! 内容不进上下文。

mod kind;

use std::io::Read as _;
use std::path::Path;

use serde::Deserialize;

use miyu_fs::{Kind as FileKind, OpenError, open_file};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::attachment::LIMIT;
use miyu_tool::{Call, Done, Effect, Progress, Running, ShownFile, Spec, Stop, Target, Tool};

use crate::blocking::blocking;
use crate::common::{Common, Shown, said};
use crate::load::{self, LoadError, say};

/// `show`。
pub(crate) struct Show {
    spec: Spec,
    texts: Texts,
}

/// 输出里给她看的几句：`software/basesystem/show/*.txt`，和几件工具共用的。
#[derive(Clone)]
struct Texts {
    common: Common,
    shown: Template,
    too_big: Template,
}

/// 她给的参数：`path` 写成 `file_path`、`filePath` 的也认。
#[derive(Deserialize)]
struct Args {
    #[serde(alias = "file_path", alias = "filePath")]
    path: String,
    #[serde(default)]
    title: Option<String>,
}

impl Show {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Show, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "show", name, fields);
        Ok(Show {
            spec: load::spec(resources, "show", Access::Read)?,
            texts: Texts {
                common,
                shown: text("shown", &[])?,
                too_big: text("too-big", &["path"])?,
            },
        })
    }
}

impl Tool for Show {
    fn spec(&self) -> &Spec {
        &self.spec
    }

    fn targets(&self, call: &Call) -> Vec<Target> {
        serde_json::from_str::<Args>(&call.args)
            .map(|args| {
                vec![Target {
                    path: args.path,
                    write: false,
                    itself: false,
                }]
            })
            .unwrap_or_default()
    }

    fn run(&self, call: Call, _progress: Progress) -> Running<'_> {
        let texts = self.texts.clone();
        Box::pin(async move {
            match serde_json::from_str::<Args>(&call.args) {
                Ok(args) => {
                    blocking(call.stop.clone(), move |stop| {
                        show(&texts, &call, &args, stop)
                    })
                    .await
                }
                Err(error) => texts.common.bad_args(&error),
            }
        })
    }
}

/// 读、认、报。读之前看一眼旗：叫停了就不读，交回 `stopped`。
fn show(texts: &Texts, call: &Call, args: &Args, stop: &Stop) -> Done {
    let path = args.path.as_str();
    let common = &texts.common;
    let real = match call.real(path) {
        Ok(real) => real,
        Err(error) => return common.failed(path, &error),
    };
    if stop.stopped() {
        return Done::stopped();
    }
    let file = match open_file(&real) {
        Ok(file) => file,
        Err(OpenError::NotFound) => return common.missing(path, &real, &Shown::here(call)),
        Err(OpenError::NotAFile(FileKind::Directory)) => return common.directory(path),
        Err(OpenError::NotAFile(_)) => return common.not_regular(path),
        Err(OpenError::Io(error)) => return common.failed(path, &error),
    };
    let mut content = Vec::new();
    if let Err(error) = file.take(LIMIT + 1).read_to_end(&mut content) {
        return common.failed(path, &error);
    }
    if content.len() as u64 > LIMIT {
        return Done::error(say(&texts.too_big, &[("path", path)]))
            .said(said("show/too-big").with("path", path));
    }
    let found = kind::kind(&real, &content);
    let name = real
        .file_name()
        .unwrap_or(real.as_os_str())
        .to_string_lossy()
        .into_owned();
    let title = args
        .title
        .as_deref()
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_string);
    Done::ok(say(&texts.shown, &[]))
        .said(said("show/shown"))
        .effect(Effect::Shown(ShownFile {
            path: real,
            content,
            name,
            media_type: found.media_type.to_string(),
            title,
            language: found.language.map(str::to_string),
            size: found.size,
        }))
}
