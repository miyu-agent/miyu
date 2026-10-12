//! 知识库（施工 K-1 上，`docs/blueprint/knowledge.md`）：一个知识库是 `home/<账号>/kb/<编号>/` 这个目录，里面是文档，外加一份
//! 派生的索引 `home/<账号>/index/kb/<编号>.db`（检索库，`recall.md`）。真相是目录里的文件，索引随时能删掉照文件重建。
//!
//! - 列、搜之前先照文件补这个库（`knowledge/catch_up.rs`）：新的、变了的整份重切，没了的拿掉，没变的（指纹一样）不重读。
//!   一个库一把锁：两个请求同时来，后来的等前面的补完。碰磁盘，调的一方放在阻塞线程里。
//! - 读照路径直接读目录里的文件（`knowledge/read.rs`），不读索引。
//! - 这一层不记运行日志：跳过的文档交回给调的一方记（[`Skipped`]）。

mod catch_up;
mod read;
mod write;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;

use crate::layers::valid;
use crate::recall::{DbError, RecallIndex};
use crate::root::DataRoot;

pub use read::Page;
pub use write::Written;

/// 核心一份的知识库登记：开过的索引留着（一个库一个连接、一把补的锁）。
#[derive(Debug)]
pub struct Knowledge {
    root: DataRoot,
    open: Mutex<BTreeMap<(AccountId, String), Arc<Shelf>>>,
}

/// 一个库开着的索引和补的锁。
#[derive(Debug)]
struct Shelf {
    index: RecallIndex,
    catching_up: Mutex<()>,
}

/// 一个库：列出来的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Base {
    /// 编号（目录名）。
    pub id: String,
    /// 给人看的名字：`kb.toml` 的 `name`，没写的照编号。
    pub name: String,
    /// 一句话说这个库是什么：`kb.toml` 的 `description`，没写的空着。
    pub description: String,
    /// 照文件补完以后有几个文档在索引里。
    pub documents: usize,
    /// 几块。
    pub chunks: usize,
    /// `kb.toml` 读不出来的为什么（照没有 `kb.toml` 的样子用）。
    pub error: Option<String>,
}

/// 搜到的一块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hit {
    /// 哪个库。
    pub kb: String,
    /// 哪个文档（库里的相对路径，照 `/` 分）。
    pub path: String,
    /// 第几块，从 0 起。
    pub chunk: usize,
    /// 标题路径，没有标题的是空的。
    pub heading: String,
    /// 这一块的字（不带标题那一行）。
    pub text: String,
    /// 文档改的时刻。
    pub at: Timestamp,
}

/// 补的时候跳过的一个文档：哪个库、哪个文档、为什么（英文的一句）。调的一方照它记一行运行日志。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// 哪个库。
    pub kb: String,
    /// 哪个文档。
    pub path: String,
    /// 为什么。
    pub why: String,
}

/// 知识库出的错。
#[derive(Debug)]
pub enum KnowledgeError {
    /// 没有这个库（编号不合写法的也算）。
    UnknownBase,
    /// 库里没有这个文档：不是文档的、不在库里的、路径跑出去的都算。
    UnknownDocument,
    /// 没有这一页。
    BadPage,
    /// 新建的那一篇已经有了（施工 K-3）。
    Exists,
    /// 写完超过一篇的上限（施工 K-3，同照文件补的 4 MiB）。
    TooLarge,
    /// 照原文换一段：原文在这一篇里找不到（施工 K-3）。
    NoMatch,
    /// 照原文换一段：原文不止一处，几处（施工 K-3）。
    Ambiguous(usize),
    /// 挪不进回收站：为什么（施工 K-3）。
    Trash(String),
    /// 索引读写不了。
    Db(DbError),
    /// 目录、文件读不了：为什么。
    Io(String),
}

impl From<DbError> for KnowledgeError {
    fn from(error: DbError) -> KnowledgeError {
        KnowledgeError::Db(error)
    }
}

impl std::fmt::Display for KnowledgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            KnowledgeError::UnknownBase => write!(f, "unknown knowledge base"),
            KnowledgeError::UnknownDocument => write!(f, "unknown document"),
            KnowledgeError::BadPage => write!(f, "no such page"),
            KnowledgeError::Exists => write!(f, "document already exists"),
            KnowledgeError::TooLarge => write!(f, "document too large"),
            KnowledgeError::NoMatch => write!(f, "text not found"),
            KnowledgeError::Ambiguous(count) => write!(f, "text found {count} times"),
            KnowledgeError::Trash(why) => write!(f, "trash: {why}"),
            KnowledgeError::Db(error) => write!(f, "index: {error}"),
            KnowledgeError::Io(why) => write!(f, "{why}"),
        }
    }
}

impl Knowledge {
    /// 一份空的登记，用到哪个库才开哪个的索引。
    #[must_use]
    pub fn new(root: &DataRoot) -> Knowledge {
        Knowledge {
            root: root.clone(),
            open: Mutex::new(BTreeMap::new()),
        }
    }

    /// 账号 `account` 下的知识库，照编号排；每个先照文件补。目录不在的当一个都没有。
    ///
    /// # Errors
    ///
    /// 知识库的目录、索引读不了。
    pub fn list(&self, account: &AccountId) -> Result<(Vec<Base>, Vec<Skipped>), KnowledgeError> {
        let mut bases = Vec::new();
        let mut skipped = Vec::new();
        for id in self.ids(account)? {
            let (base, mut dropped) = self.catch_up(account, &id)?;
            skipped.append(&mut dropped);
            bases.push(base);
        }
        Ok((bases, skipped))
    }

    /// 照关键词搜：`only` 是只搜这几个库（人经协议写了 `kb` 的一个，她的工具照人格收窄的几个，施工 K-1 下），没有的搜账号下
    /// 全部；几个库的照名次轮流合（每个库的第一名在前，同名次照 `only` 的先后，全部的照编号），最多 `limit` 块。搜之前照文件补。
    ///
    /// # Errors
    ///
    /// `only` 里有不在的库；目录、索引读不了。
    pub fn search(
        &self,
        account: &AccountId,
        query: &str,
        only: Option<&[String]>,
        limit: usize,
    ) -> Result<(Vec<Hit>, Vec<Skipped>), KnowledgeError> {
        let ids = match only {
            Some(ids) => {
                for id in ids {
                    self.dir(account, id)?;
                }
                ids.to_vec()
            }
            None => self.ids(account)?,
        };
        let mut skipped = Vec::new();
        let mut lists = Vec::new();
        for id in ids {
            let (_, mut dropped) = self.catch_up(account, &id)?;
            skipped.append(&mut dropped);
            let shelf = self.shelf(account, &id);
            let found = shelf.index.search(query, limit)?;
            lists.push((id, found));
        }
        let mut hits = Vec::new();
        for rank in 0..limit {
            for (id, found) in &lists {
                if let Some(hit) = found.get(rank) {
                    hits.extend(catch_up::hit(id, hit));
                }
            }
        }
        hits.truncate(limit);
        Ok((hits, skipped))
    }

    /// 读库 `kb` 里的文档 `path` 的第 `page` 页（从 1 起）：照路径直接读文件。
    ///
    /// # Errors
    ///
    /// 没有这个库、库里没有这个文档、没有这一页；文件读不了。
    pub fn read(
        &self,
        account: &AccountId,
        kb: &str,
        path: &str,
        page: usize,
    ) -> Result<Page, KnowledgeError> {
        let dir = self.dir(account, kb)?;
        read::page(&dir, path, page)
    }

    /// 库 `id` 给人看的名字和一句说明（`kb.toml`，施工 K-1 补：她的 system 里列出来）：没写名字的照编号，没写说明的空着；
    /// `kb.toml` 写错的照没写用（`kb.list` 带的 `error` 照旧在那边报）。不补索引。
    ///
    /// # Errors
    ///
    /// 没有这个库。
    pub fn title(&self, account: &AccountId, id: &str) -> Result<(String, String), KnowledgeError> {
        let dir = self.dir(account, id)?;
        let (about, _) = catch_up::about(&dir);
        Ok((
            about.name.unwrap_or_else(|| id.to_string()),
            about.description.unwrap_or_default(),
        ))
    }

    /// 账号下合写法的库的编号，照编号排（施工 K-1 下起会话照它判给不给她两件工具、她查得到哪几个）。目录不在的是空的。
    ///
    /// # Errors
    ///
    /// 知识库的目录读不了。
    pub fn ids(&self, account: &AccountId) -> Result<Vec<String>, KnowledgeError> {
        let top = self.top(account);
        let entries = match std::fs::read_dir(&top) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(KnowledgeError::Io(error.to_string())),
        };
        let mut ids: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| valid(name))
            .collect();
        ids.sort();
        Ok(ids)
    }

    /// 账号下的知识库都在哪个目录下。
    fn top(&self, account: &AccountId) -> PathBuf {
        self.root.account_dir(account).join("kb")
    }

    /// 库 `id` 的目录：编号合写法、目录在。
    fn dir(&self, account: &AccountId, id: &str) -> Result<PathBuf, KnowledgeError> {
        let dir = self.top(account).join(id);
        if valid(id) && dir.is_dir() {
            Ok(dir)
        } else {
            Err(KnowledgeError::UnknownBase)
        }
    }

    /// 库 `id` 开着的索引：开过的交回同一份。
    fn shelf(&self, account: &AccountId, id: &str) -> Arc<Shelf> {
        let mut open = self.lock();
        let key = (account.clone(), id.to_string());
        if let Some(shelf) = open.get(&key) {
            return Arc::clone(shelf);
        }
        let path = self.root.index(account).join("kb").join(format!("{id}.db"));
        let (index, _) = RecallIndex::open(&path);
        let shelf = Arc::new(Shelf {
            index,
            catching_up: Mutex::new(()),
        });
        open.insert(key, Arc::clone(&shelf));
        shelf
    }

    fn lock(&self) -> MutexGuard<'_, BTreeMap<(AccountId, String), Arc<Shelf>>> {
        self.open.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
