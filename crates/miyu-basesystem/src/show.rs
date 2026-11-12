//! `show`（施工 D-5，`docs/blueprint/tools/show.md`，设计 `35-展示.md`）：把一个文件给人看。照 `read` 的规矩打开（一层链接都
//! 不跟），照打开的句柄取大小和修改时间，读开头一截认种类（[`kind`]），报 `file.shown`。施工 SW-1 起不拷内容、不限大小：头照
//! 路径读，远程的头经 `session.shown` 读。她只收到一句「已展示给用户」，内容不进上下文。

mod kind;

use std::io::Read as _;
use std::path::Path;

use serde::Deserialize;

use miyu_fs::{Kind as FileKind, OpenError, open_file};
use miyu_kernel::template::Template;
use miyu_kernel::tool::Access;
use miyu_tool::{
    Call, Done, Effect, Progress, Running, ShownFile, Spec, Stop, Target, Tool, modified_at,
};

use crate::blocking::blocking;
use crate::common::{Common, Shown, said};
use crate::load::{self, LoadError, say};

/// 认种类读开头多少（施工 SW-1）：图片、PDF、音视频只看文件头，「是不是 UTF-8」看这一截。以前整份读、最多 20 MiB；1 MiB 以内的
/// 文件整份都看，结果和以前一样，再大的文本、视频也只读这么多。
const KIND_HEAD: u64 = 1024 * 1024;

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

/// 打开、取大小和修改时间、读开头认、报。读之前看一眼旗：叫停了就不读，交回 `stopped`。
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
    // 大小、修改时间照打开的这个句柄取，不另 stat 路径：中间被换掉的也是同一份。
    let metadata = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) => return common.failed(path, &error),
    };
    let Some(modified) = modified_at(&metadata) else {
        let error = std::io::Error::other("no usable modification time");
        return common.failed(path, &error);
    };
    let mut head = Vec::new();
    if let Err(error) = file.take(KIND_HEAD).read_to_end(&mut head) {
        return common.failed(path, &error);
    }
    let found = kind::kind(&real, &head, metadata.len() > head.len() as u64);
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
            name,
            media_type: found.media_type.to_string(),
            size: metadata.len(),
            modified,
            title,
            language: found.language.map(str::to_string),
            pixels: found.size,
        }))
}
