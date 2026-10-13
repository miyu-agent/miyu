//! 人管理知识库（施工 K-7 上，`docs/blueprint/knowledge.md`「怎么走」第八条）：建库、改名字说明、挂目录、加文档、删库、重建
//! 索引。人经协议用；她的写照旧在 `write.rs`（她删不了整个库，19 N14）。
//!
//! - `kb.toml` 照配置的那一套只改那一格（`miyu_config::edit`）：注释、别的格（`mount`）照原样。
//! - 删库、重建先拿这个库的补的锁，再关上索引的连接、删掉索引的文件（Windows 上开着的 SQLite 删不掉）；会话在后台补向量拿着的
//!   是同一份索引，关上以后它照「库用不了的不写」。
//! - 删库整个库目录挪进系统回收站（2026-10-13 项目主人定）；挂进来的那个库目录里只有登记，挂的目录在外面，一个字都不动。

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::PoisonError;

use miyu_config::Value;
use miyu_config::edit::{Change, apply};
use miyu_kernel::id::AccountId;

use super::mount::Documents;
use super::read::target;
use super::write::{existing, trash_refused};
use super::{Base, Knowledge, KnowledgeError, Skipped};
use crate::layers::valid;

impl Knowledge {
    /// 建库 `kb`，`name`、`description` 写进 `kb.toml`（不写的不写）。
    ///
    /// # Errors
    ///
    /// 编号不合写法（[`KnowledgeError::UnknownBase`]）；已经有了（[`KnowledgeError::BaseExists`]）；建不了、写不进。
    pub fn found(
        &self,
        account: &AccountId,
        kb: &str,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<Base, KnowledgeError> {
        let dir = self.vacant(account, kb)?;
        std::fs::create_dir_all(&dir).map_err(io)?;
        about(&dir, &[("name", name), ("description", description)])?;
        self.counted(account, kb, None)
    }

    /// 改库 `kb` 的名字、说明：不写的不动，空字符串是拿掉；`kb.toml` 里别的照原样。
    ///
    /// # Errors
    ///
    /// 没有这个库；`kb.toml` 读不懂、写不进。
    pub fn rename(
        &self,
        account: &AccountId,
        kb: &str,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<Base, KnowledgeError> {
        let dir = self.dir(account, kb)?;
        about(&dir, &[("name", name), ("description", description)])?;
        self.counted(account, kb, None)
    }

    /// 把现成的目录 `folder`（绝对路径或 `~/` 开头）挂成库 `kb`，照它补一遍，交回补完的样子和跳过的文档。挂不上的什么都不建。
    ///
    /// # Errors
    ///
    /// 编号不合写法、已经有了；目录用不了（[`KnowledgeError::FolderUnavailable`]）；建不了、写不进、补不了。
    pub fn mount(
        &self,
        account: &AccountId,
        kb: &str,
        folder: &str,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<(Base, Vec<Skipped>), KnowledgeError> {
        let dir = self.vacant(account, kb)?;
        self.mounted(folder)
            .map_err(KnowledgeError::FolderUnavailable)?;
        std::fs::create_dir_all(&dir).map_err(io)?;
        about(
            &dir,
            &[
                ("name", name),
                ("description", description),
                ("mount", Some(folder)),
            ],
        )?;
        self.catch_up(account, kb)
    }

    /// 往库 `kb` 里加一篇 `path`，内容是 `bytes`（人加的什么格式都行，PDF、网页也行）；已经有了的只在 `replace` 时换掉。
    ///
    /// # Errors
    ///
    /// 没有这个库；挂进来的（[`KnowledgeError::ReadOnly`]）；路径不合写法、不是文档（[`KnowledgeError::UnknownDocument`]）；
    /// 已经有了（[`KnowledgeError::Exists`]）；写不进。
    pub fn add(
        &self,
        account: &AccountId,
        kb: &str,
        path: &str,
        bytes: &[u8],
        replace: bool,
    ) -> Result<(), KnowledgeError> {
        let Documents::Own(dir) = self.documents(account, kb)? else {
            return Err(KnowledgeError::ReadOnly);
        };
        let dir = std::fs::canonicalize(dir).map_err(io)?;
        let file = target(&dir, path).ok_or(KnowledgeError::UnknownDocument)?;
        if existing(&file)?.is_some() && !replace {
            return Err(KnowledgeError::Exists);
        }
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent).map_err(io)?;
        }
        miyu_fs::replace(&file, bytes).map_err(io)
    }

    /// 删库 `kb`：先删索引，再把整个库目录挪进系统回收站（`home` 是这台机器的家目录，Linux 照它找回收站），交回回收站里的
    /// 位置。挪不进的，库目录照旧在，索引下一次补的时候照文件重建。
    ///
    /// # Errors
    ///
    /// 没有这个库；索引删不掉；挪不进回收站（[`KnowledgeError::Trash`]）。
    pub fn delete(
        &self,
        account: &AccountId,
        kb: &str,
        home: Option<&Path>,
    ) -> Result<String, KnowledgeError> {
        let dir = std::fs::canonicalize(self.dir(account, kb)?).map_err(io)?;
        let shelf = self.shelf(account, kb);
        let _only_one = shelf
            .catching_up
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        shelf.index.discard()?;
        self.lock().remove(&(account.clone(), kb.to_string()));
        miyu_fs::trash::put(&dir, home).map_err(trash_refused)
    }

    /// 重建库 `kb` 的索引：删掉照文件补一遍（向量下一次搜完再补），交回补完的样子和跳过的文档；挂的目录用不了的照列出来的样子
    /// 带 `error`。
    ///
    /// # Errors
    ///
    /// 没有这个库；索引建不了；补不了。
    pub fn reindex(
        &self,
        account: &AccountId,
        kb: &str,
    ) -> Result<(Base, Vec<Skipped>), KnowledgeError> {
        self.dir(account, kb)?;
        {
            let shelf = self.shelf(account, kb);
            let _only_one = shelf
                .catching_up
                .lock()
                .unwrap_or_else(PoisonError::into_inner);
            shelf.index.reset()?;
        }
        match self.catch_up(account, kb) {
            Err(KnowledgeError::Unavailable(why)) => {
                Ok((self.counted(account, kb, Some(why))?, Vec::new()))
            }
            caught => caught,
        }
    }

    /// 要建的库 `kb` 的目录：编号合写法、还没有。
    fn vacant(&self, account: &AccountId, kb: &str) -> Result<PathBuf, KnowledgeError> {
        if !valid(kb) {
            return Err(KnowledgeError::UnknownBase);
        }
        let dir = self.top(account).join(kb);
        match dir.exists() {
            true => Err(KnowledgeError::BaseExists),
            false => Ok(dir),
        }
    }
}

/// 库目录 `dir` 的 `kb.toml` 照 `fields` 改：没写的（`None`）不动，空字符串拿掉，别的写成字。没有 `kb.toml` 的新建。
fn about(dir: &Path, fields: &[(&str, Option<&str>)]) -> Result<(), KnowledgeError> {
    let file = std::fs::canonicalize(dir).map_err(io)?.join("kb.toml");
    let mut text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => return Err(io(error)),
    };
    for (key, value) in fields {
        let value = value.map(|value| Value::Text(Cow::Owned(value.to_string())));
        let change = match &value {
            None => continue,
            Some(Value::Text(text)) if text.is_empty() => Change::Unset(key),
            Some(value) => Change::Set(key, value),
        };
        text = apply(&text, change).map_err(|blocked| {
            KnowledgeError::Io(format!("kb.toml: cannot write {}", blocked.0))
        })?;
    }
    miyu_fs::replace(&file, text.as_bytes()).map_err(io)
}

fn io(error: std::io::Error) -> KnowledgeError {
    KnowledgeError::Io(error.to_string())
}
