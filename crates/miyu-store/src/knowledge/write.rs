//! 她写知识库（施工 K-3，`docs/blueprint/knowledge.md`「怎么走」第七条，19 N14）：建库、新建一篇、追加、照原文换一段、整篇
//! 挪进系统回收站。路径照读的那一套（[`super::read`]）；写照 `miyu_fs::replace()`（先写临时文件再改名，中途崩了原来的还在），
//! 删照 `miyu_fs::trash`。每次交回改前改后的内容：会话照它记成效果，撤销那一轮时改回去（同 `write`、`trash`）。索引不在这里
//! 动：下一次搜的时候照文件补。
//!
//! 写之前把库的目录换成真实的位置（`canonicalize`）：`miyu_fs::replace()` 一层层不跟链接地打开上级目录，数据根的路上有链接的
//! （macOS 的临时目录 `/var` 是 `/private/var` 的链接，家目录挪过位置的）照原样写不进，同 `write` 先换真实位置再写。

use std::path::{Path, PathBuf};

use miyu_kernel::id::AccountId;

use super::catch_up::BIGGEST;
use super::extract::Kind;
use super::mount::Documents;
use super::read::target;
use super::{Knowledge, KnowledgeError};
use crate::layers::valid;

/// 写进去的一篇：在哪（绝对路径）、改前的内容（新建的没有）、改后的内容。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Written {
    /// 文件在哪。
    pub path: PathBuf,
    /// 改前的内容；新建的没有。
    pub before: Option<Vec<u8>>,
    /// 改后的内容。
    pub after: Vec<u8>,
}

impl Knowledge {
    /// 建库 `kb`：建它的目录，写了 `about` 的写进 `kb.toml` 的 `description`。已经有的什么都不动。
    ///
    /// # Errors
    ///
    /// 编号不合写法（[`KnowledgeError::UnknownBase`]）；建不了、写不进。
    pub fn create(
        &self,
        account: &AccountId,
        kb: &str,
        about: Option<&str>,
    ) -> Result<(), KnowledgeError> {
        if !valid(kb) {
            return Err(KnowledgeError::UnknownBase);
        }
        let dir = self.top(account).join(kb);
        if dir.is_dir() {
            return Ok(());
        }
        std::fs::create_dir_all(&dir).map_err(io)?;
        let dir = std::fs::canonicalize(&dir).map_err(io)?;
        if let Some(about) = about.map(str::trim).filter(|about| !about.is_empty()) {
            // JSON 的字符串写法也是合法的 TOML 基本字符串：引号、反斜杠、换行都照它转义。
            let quoted =
                serde_json::to_string(about).map_err(|error| io(std::io::Error::other(error)))?;
            miyu_fs::replace(
                &dir.join("kb.toml"),
                format!("description = {quoted}\n").as_bytes(),
            )
            .map_err(io)?;
        }
        Ok(())
    }

    /// 在库 `kb` 里写一篇 `path`：不 `append` 的新建（已经有的不写），`append` 的接在后面（没有就新建；原来末尾没换行的先补
    /// 一个）。写进去的末尾没换行的补一个。
    ///
    /// # Errors
    ///
    /// 没有这个库；路径不合写法、不是文档、有了却不是普通文件（[`KnowledgeError::UnknownDocument`]）；PDF、网页
    /// （[`KnowledgeError::NotText`]，施工 K-5）；新建的已经有了；
    /// 写完超过 4 MiB；读不了、写不进。
    pub fn save(
        &self,
        account: &AccountId,
        kb: &str,
        path: &str,
        text: &str,
        append: bool,
    ) -> Result<Written, KnowledgeError> {
        let file = self.target(account, kb, path)?;
        Knowledge::writable(path)?;
        let before = existing(&file)?;
        if before.is_some() && !append {
            return Err(KnowledgeError::Exists);
        }
        let mut after = before.clone().unwrap_or_default();
        if !after.is_empty() && !after.ends_with(b"\n") {
            after.push(b'\n');
        }
        after.extend_from_slice(text.as_bytes());
        if !after.ends_with(b"\n") {
            after.push(b'\n');
        }
        put(file, before, after)
    }

    /// 把库 `kb` 里 `path` 那一篇的 `old` 换成 `new`：`old` 得正好出现一次。
    ///
    /// # Errors
    ///
    /// 没有这个库、这一篇；PDF、网页（[`KnowledgeError::NotText`]，施工 K-5）；`old` 找不到、不止一处；不是 UTF-8 的当读不了；
    /// 写完超过 4 MiB；写不进。
    pub fn edit(
        &self,
        account: &AccountId,
        kb: &str,
        path: &str,
        old: &str,
        new: &str,
    ) -> Result<Written, KnowledgeError> {
        let file = self.target(account, kb, path)?;
        Knowledge::writable(path)?;
        let before = existing(&file)?.ok_or(KnowledgeError::UnknownDocument)?;
        let text = std::str::from_utf8(&before)
            .map_err(|_| KnowledgeError::Io("not UTF-8".to_string()))?;
        match text.matches(old).count() {
            0 => return Err(KnowledgeError::NoMatch),
            1 => {}
            count => return Err(KnowledgeError::Ambiguous(count)),
        }
        let after = text.replacen(old, new, 1).into_bytes();
        put(file, Some(before), after)
    }

    /// 把库 `kb` 里 `path` 那一篇挪进系统回收站（`home` 是这台机器的家目录，Linux 照它找回收站）：交回它原来在哪、回收站
    /// 里在哪。
    ///
    /// # Errors
    ///
    /// 没有这个库、这一篇；挪不进回收站（没有能放的、挪了找不到、挪不动）。
    pub fn trash(
        &self,
        account: &AccountId,
        kb: &str,
        path: &str,
        home: Option<&Path>,
    ) -> Result<(PathBuf, String), KnowledgeError> {
        let file = self.target(account, kb, path)?;
        existing(&file)?.ok_or(KnowledgeError::UnknownDocument)?;
        match miyu_fs::trash::put(&file, home) {
            Ok(location) => Ok((file, location)),
            Err(miyu_fs::trash::Refused::Unavailable) => {
                Err(KnowledgeError::Trash("no trash on this disk".to_string()))
            }
            Err(miyu_fs::trash::Refused::Lost) => Err(KnowledgeError::Trash(
                "moved but not found in the trash".to_string(),
            )),
            Err(miyu_fs::trash::Refused::Failed(error)) => {
                Err(KnowledgeError::Trash(error.to_string()))
            }
        }
    }

    /// `path` 是不是她写得了的一篇（库里的相对路径、扩展名，同读的那一套；只有文字的两种，施工 K-5），不看在不在（施工 K-3：
    /// 建库以前先看）。
    ///
    /// # Errors
    ///
    /// 不合写法、不是文档（[`KnowledgeError::UnknownDocument`]）；PDF、网页（[`KnowledgeError::NotText`]）。
    pub fn writable(path: &str) -> Result<(), KnowledgeError> {
        target(Path::new(""), path).ok_or(KnowledgeError::UnknownDocument)?;
        match Kind::of(path) {
            Some(kind) if kind.writable() => Ok(()),
            _ => Err(KnowledgeError::NotText),
        }
    }

    /// 库 `kb` 里 `path` 照写法该在哪：库得在、不是挂进来的（施工 K-4，只读），路径照读的那一套，库的目录照真实的位置。
    fn target(&self, account: &AccountId, kb: &str, path: &str) -> Result<PathBuf, KnowledgeError> {
        let Documents::Own(dir) = self.documents(account, kb)? else {
            return Err(KnowledgeError::ReadOnly);
        };
        let dir = std::fs::canonicalize(dir).map_err(io)?;
        target(&dir, path).ok_or(KnowledgeError::UnknownDocument)
    }
}

/// 一篇现在的内容：没有的是没有；有了却不是普通文件（目录、链接）的当不是文档。
fn existing(file: &Path) -> Result<Option<Vec<u8>>, KnowledgeError> {
    match std::fs::symlink_metadata(file) {
        Ok(metadata) if metadata.is_file() => std::fs::read(file).map(Some).map_err(io),
        Ok(_) => Err(KnowledgeError::UnknownDocument),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(io(error)),
    }
}

/// 写进去：超过上限的不写；上级目录没有的建。
fn put(file: PathBuf, before: Option<Vec<u8>>, after: Vec<u8>) -> Result<Written, KnowledgeError> {
    if u64::try_from(after.len()).map_or(true, |size| size > BIGGEST) {
        return Err(KnowledgeError::TooLarge);
    }
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).map_err(io)?;
    }
    miyu_fs::replace(&file, &after).map_err(io)?;
    Ok(Written {
        path: file,
        before,
        after,
    })
}

fn io(error: std::io::Error) -> KnowledgeError {
    KnowledgeError::Io(error.to_string())
}
