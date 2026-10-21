//! 她带的附件（施工 C-9，`docs/blueprint/tools/send_message.md`「附件」）：`send_message`、`subagent` 的 `attachments`。
//! 执行前的链照 [`targets`] 查过（工作区以外的问过人、只读的数据根碰不到），这里照这次调用的工作目录读、照内容认
//! （[`miyu_tool::attachment::kind`]，和人附的同一份），交给端口存成 blob、造成块。有一个读不成、太大的，整句不发：
//! 她改了路径再发一次，那边不会收到缺了一个附件的半句。

use std::io::Read as _;
use std::path::Path;

use miyu_fs::{Kind, OpenError, open_file};
use miyu_kernel::id::{FileName, SourcePath};
use miyu_kernel::template::Template;
use miyu_tool::attachment::{Attached, LIMIT, kind};
use miyu_tool::{Call, Done, Target};

use crate::blocking::blocking;
use crate::common::{Common, Shown, plain, said};
use crate::load::{self, LoadError, say};

/// 一句话最多带几个附件。
pub(crate) const MAX: usize = 8;

/// 输出里给她看的几句：`software/basesystem/attachments/*.txt`，和几件工具共用的。
#[derive(Clone)]
pub(crate) struct Texts {
    common: Common,
    too_many: Template,
    too_big: Template,
}

impl Texts {
    /// 照资源目录 `resources` 里的字造，共用的几句是 `common`。
    pub(crate) fn load(resources: &Path, common: Common) -> Result<Texts, LoadError> {
        let text = |name: &str, fields: &[&str]| load::text(resources, "attachments", name, fields);
        Ok(Texts {
            common,
            too_many: text("too-many", &["limit"])?,
            too_big: text("too-big", &["path"])?,
        })
    }
}

/// 要读的几个路径：给执行前的链查。
pub(crate) fn targets(paths: &[String]) -> Vec<Target> {
    paths
        .iter()
        .map(|path| Target {
            path: path.clone(),
            write: false,
            itself: false,
        })
        .collect()
}

/// 照先后读好、认好。超过 [`MAX`] 个的、有一个读不成的、太大的，交回给她的那一句。
pub(crate) async fn read(
    texts: &Texts,
    call: &Call,
    paths: Vec<String>,
) -> Result<Vec<Attached>, Box<Done>> {
    if paths.len() > MAX {
        let limit = MAX.to_string();
        return Err(Box::new(
            Done::error(say(&texts.too_many, &[("limit", &limit)]))
                .said(said("attachments/too-many").with("limit", limit)),
        ));
    }
    let texts = texts.clone();
    let call = call.clone();
    blocking(call.stop.clone(), move |_| {
        paths.iter().map(|path| one(&texts, &call, path)).collect()
    })
    .await
}

/// 读一个：换成真实的位置，一层链接都不跟地打开，最多读到上限多一个字节。
fn one(texts: &Texts, call: &Call, path: &str) -> Result<Attached, Box<Done>> {
    let common = &texts.common;
    let failed = |error: &dyn std::fmt::Display| Box::new(common.failed(path, error));
    let real = call.real(path).map_err(|error| failed(&error))?;
    let file = match open_file(&real) {
        Ok(file) => file,
        Err(OpenError::NotFound) => {
            return Err(Box::new(common.missing(path, &real, &Shown::here(call))));
        }
        Err(OpenError::NotAFile(Kind::Directory)) => return Err(Box::new(common.directory(path))),
        Err(OpenError::NotAFile(_)) => return Err(Box::new(common.not_regular(path))),
        Err(OpenError::Io(error)) => return Err(failed(&error)),
    };
    let mut bytes = Vec::new();
    file.take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| failed(&error))?;
    let too_big = || {
        Box::new(
            Done::error(say(&texts.too_big, &[("path", path)]))
                .said(said("attachments/too-big").with("path", path)),
        )
    };
    if bytes.len() as u64 > LIMIT {
        return Err(too_big());
    }
    let kind = kind(&bytes, None).map_err(|_| too_big())?;
    let segment = real.file_name().unwrap_or(real.as_os_str());
    let name = FileName::parse(&segment.to_string_lossy()).map_err(|error| failed(&error))?;
    Ok(Attached {
        name,
        // Windows 上真实的位置带着 `\\?\`：去掉，和头交来的路径一个写法。
        path: SourcePath::parse(&plain(&real)).ok(),
        bytes,
        kind,
    })
}
