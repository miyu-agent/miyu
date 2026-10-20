//! 知识库（施工 K-1 上，`docs/blueprint/knowledge.md`）：一个知识库是 `home/<账号>/kb/<编号>/` 这个目录，里面是文档，外加一份
//! 派生的索引 `home/<账号>/index/kb/<编号>.db`（检索库，`recall.md`）。真相是目录里的文件，索引随时能删掉照文件重建。
//!
//! - 列、搜之前先照文件补这个库（`knowledge/catch_up.rs`）：新的、变了的整份重切，没了的拿掉，没变的（指纹一样）不重读。
//!   一个库一把锁：两个请求同时来，后来的等前面的补完。碰磁盘，调的一方放在阻塞线程里。
//! - 读照路径直接读目录里的文件（`knowledge/read.rs`），不读索引。
//! - 挂进来的库（施工 K-4，`knowledge/mount.rs`）：文档在 `kb.toml` 的 `mount` 那个目录里，只读；挂的目录用不了的列出来带
//!   `error`、搜的时候跳过，索引不动。
//! - 软件包带来的库（施工 K-8，`knowledge/place.rs`）：装了的包目录里的 `kb/<编号>/`，只读，自动想起的开关人改了记在账号
//!   自己的 `kb.toml`；同编号的自己的赢；卸包时索引、开关一起删。
//! - 这一层不记运行日志：跳过的文档交回给调的一方记（[`Skipped`]），卸包清不掉的交回给调的一方记。

mod catch_up;
mod extract;
mod listing;
mod manage;
mod mount;
mod place;
mod read;
mod watch;
mod write;

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;
use miyu_recall::associate::Thresholds;

use crate::recall::{DbError, RecallIndex};
use crate::root::DataRoot;

pub use extract::{EXTENSIONS, Kind as DocumentKind, beside as extractor_beside};
pub use listing::{Document, Listing};
pub use manage::BaseFields;
pub use read::Page;
pub use watch::IndexWatch;
pub use write::Written;

/// 包文件夹 `dir` 带的库的编号（施工 F-8 六补：装之前查冲突，`docs/blueprint/packages.md`「冲突」），照编号排。
pub fn carried(dir: &std::path::Path) -> Vec<String> {
    place::carried_in(dir)
        .into_iter()
        .map(|(id, _)| id)
        .collect()
}

/// 核心一份的知识库登记：开过的索引留着（一个库一个连接、一把补的锁）。
#[derive(Debug)]
pub struct Knowledge {
    root: DataRoot,
    /// 这台机器的家目录：挂的路径 `~/` 开头的照它接（施工 K-4）。
    home: Option<PathBuf>,
    /// 抽字的小程序 `miyu-extract`（施工 K-5）：没有的不认 PDF、网页（补的时候跳过，读的时候读不了）。
    extractor: Option<PathBuf>,
    /// 装了的软件包（编号、包目录，照编号排，施工 K-8）：它们的 `kb/` 里的库也算。
    packages: place::Packages,
    /// 只照意思找到的那一路的下限，照模型（施工 K-2 修）：表里没有的照 [`miyu_recall::FLOOR`]。
    floors: Thresholds,
    open: Mutex<Open>,
    /// 建索引的进度报给谁（施工 W-15）：没登记的不报。
    watcher: Option<watch::Watcher>,
}

/// 开着的索引：账号、库的编号 → 那一份。
type Open = BTreeMap<(AccountId, String), Arc<Shelf>>;

/// 一个库开着的索引和补的锁。索引交得出去（施工 K-2）：会话在后台补它缺的向量。
#[derive(Debug)]
struct Shelf {
    index: Arc<RecallIndex>,
    catching_up: Mutex<()>,
    /// 索引文件在哪（施工 K-8：卸包时照它认出是那个包的）。
    path: PathBuf,
}

/// 照意思找要的问句的向量（施工 K-2，`recall.md` 第三条）：算它的模型、归一化过的向量。
#[derive(Debug, Clone, Copy)]
pub struct Meaning<'a> {
    /// 算向量的模型（`local:<模型>`、`<供应商>/<模型>`）：向量表里照它找。
    pub model: &'a str,
    /// 问句的向量。
    pub vector: &'a [f32],
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
    /// `kb.toml` 读不出来的、挂的目录用不了的（施工 K-4）为什么。
    pub error: Option<String>,
    /// 自动想起开着（`kb.toml` 的 `auto`，施工 K-6 下；包里的人改过的照改的，施工 K-8）。
    pub auto: bool,
    /// 软件包带来的是哪个包的（施工 K-8）；自己的、挂进来的没有。
    pub package: Option<String>,
    /// 挂进来的库挂的是哪个目录（施工 K-10）：`kb.toml` 里写的原样，`~/` 不展开；自己的、软件包带来的没有。
    pub mount: Option<String>,
    /// 上次建好（有文档重建过、补完）的时刻（施工 W-15）：没建过的、索引删掉重建过还没建好的没有。
    pub indexed_at: Option<String>,
    /// 这时有没有在建（施工 W-15）：别的线程正在补它。
    pub indexing: bool,
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
    /// PDF 的第几页，从 1 起（施工 K-5）；别的文档没有。
    pub page: Option<usize>,
    /// 这一块的字（不带标题那一行）。
    pub text: String,
    /// 文档改的时刻。
    pub at: Timestamp,
}

/// 补的时候跳过的一个文档（或者读不了的子目录）：哪个库、哪个文档、哪一种、为什么（英文的一句）。调的一方照它记一行运行日志。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// 哪个库。
    pub kb: String,
    /// 哪个文档；`folder` 的是哪个子目录。
    pub path: String,
    /// 哪一种（施工 K-10）。
    pub reason: Reason,
    /// 为什么。
    pub why: String,
    /// 是读不了的子目录（施工 K-10）。
    pub folder: bool,
}

/// 一个文档为什么没进索引（施工 K-10，`kb.documents` 的 `reason`）：头照它写给人看的字。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// 文件或者抽出来的字超过上限。
    TooLarge,
    /// 文字的文档不是 UTF-8。
    NotUtf8,
    /// PDF 里没有字：扫描的、只有图。
    NoText,
    /// PDF、网页，没有抽字的小程序。
    NoExtractor,
    /// 抽字的小程序出错、超时。
    ExtractFailed,
    /// 文件、子目录读不了。
    Unreadable,
}

impl Reason {
    /// 协议里的写法。
    #[must_use]
    pub fn code(self) -> &'static str {
        match self {
            Reason::TooLarge => "too_large",
            Reason::NotUtf8 => "not_utf8",
            Reason::NoText => "no_text",
            Reason::NoExtractor => "no_extractor",
            Reason::ExtractFailed => "extract_failed",
            Reason::Unreadable => "unreadable",
        }
    }
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
    /// 挂进来的库只读（施工 K-4，19 N7）。
    ReadOnly,
    /// 软件包带来的库只读（施工 K-8）：名字、说明、文档照包；自动想起的开关人能改。
    Packaged,
    /// 不是文字的文档（PDF、网页）写不了（施工 K-5）。
    NotText,
    /// 要建、要挂的库已经有了（施工 K-7 上）。
    BaseExists,
    /// 要挂的目录用不了：为什么（施工 K-7 上）。
    FolderUnavailable(String),
    /// 挂的目录用不了：为什么（施工 K-4）。列出来的照它带 `error`，搜的时候跳过，读的当没有这个文档。
    Unavailable(String),
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
            KnowledgeError::ReadOnly => write!(f, "mounted folder is read-only"),
            KnowledgeError::Packaged => write!(f, "knowledge base from a package is read-only"),
            KnowledgeError::NotText => write!(f, "only text documents can be written"),
            KnowledgeError::BaseExists => write!(f, "knowledge base already exists"),
            KnowledgeError::FolderUnavailable(why) => write!(f, "{why}"),
            KnowledgeError::Unavailable(why) => write!(f, "{why}"),
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
            home: None,
            extractor: None,
            packages: Mutex::default(),
            floors: Thresholds::default(),
            open: Mutex::new(BTreeMap::new()),
            watcher: None,
        }
    }

    /// 挂的路径 `~/` 开头的照这台机器的家目录 `home` 接（施工 K-4）；没交的那种挂不上（列出来带 `error`）。
    #[must_use]
    pub fn with_home(self, home: Option<PathBuf>) -> Knowledge {
        Knowledge { home, ..self }
    }

    /// PDF、网页照抽字的小程序 `extractor` 抽（施工 K-5，[`extractor_beside`] 找主程序旁边的那个）；没交的那种不认。
    #[must_use]
    pub fn with_extractor(self, extractor: Option<PathBuf>) -> Knowledge {
        Knowledge { extractor, ..self }
    }

    /// 建索引的进度报给 `watch`（施工 W-15，`watch.rs`）。
    #[must_use]
    pub fn with_watch(self, watch: Arc<dyn IndexWatch>) -> Knowledge {
        Knowledge {
            watcher: Some(watch::Watcher(watch)),
            ..self
        }
    }

    /// 只照意思找到的那一路的下限照这张表（施工 K-2 修，`core/knowledge/search.toml`）；没交的、表里没有的模型照
    /// [`miyu_recall::FLOOR`]。
    #[must_use]
    pub fn with_floors(self, floors: Thresholds) -> Knowledge {
        Knowledge { floors, ..self }
    }

    /// 问句照模型 `model` 算的时候，只照意思找到的那一路的下限（施工 K-2 修）：表里有的照表，没有的照 [`miyu_recall::FLOOR`]。
    #[must_use]
    pub fn floor(&self, model: &str) -> f32 {
        self.floors.of(model).unwrap_or(miyu_recall::FLOOR)
    }

    /// 账号 `account` 下的知识库，照编号排；每个先照文件补，别处正在补的不等它，照索引现有的样子列、带 `indexing`（施工 W-15：
    /// 不然列表一直等到建完，「在建」永远看不到）。目录不在的当一个都没有。
    ///
    /// # Errors
    ///
    /// 知识库的目录、索引读不了。
    pub fn list(&self, account: &AccountId) -> Result<(Vec<Base>, Vec<Skipped>), KnowledgeError> {
        let mut bases = Vec::new();
        let mut skipped = Vec::new();
        for id in self.ids(account)? {
            if self.shelf(account, &id).catching_up.try_lock().is_err() {
                bases.push(self.counted(account, &id, None)?);
                continue;
            }
            match self.catch_up(account, &id) {
                Ok((base, mut dropped)) => {
                    skipped.append(&mut dropped);
                    bases.push(base);
                }
                Err(KnowledgeError::Unavailable(why)) => {
                    bases.push(self.counted(account, &id, Some(why))?);
                }
                Err(error) => return Err(error),
            }
        }
        Ok((bases, skipped))
    }

    /// 照关键词搜：`only` 是只搜这几个库（人经协议写了 `kb` 的一个，她的工具照人格收窄的几个，施工 K-1 下），没有的搜账号下
    /// 全部；几个库的照名次轮流合（每个库的第一名在前，同名次照 `only` 的先后，全部的照编号），最多 `limit` 块。搜之前照文件补。
    /// 有问句的向量 `near` 的（施工 K-2）：每个库照关键词、照意思各找 `limit` 的四倍，两路照名次合（`fuse_above`，只有照意思
    /// 那一路找到的过下限，下限照模型，[`Knowledge::with_floors`]），再轮流合；补向量是调的一方在后台做的（[`Knowledge::index_of`]），没补上的块只有关键词那一路找得到。
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
        near: Option<Meaning<'_>>,
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
        let floor = near.map_or(miyu_recall::FLOOR, |near| self.floor(near.model));
        let mut skipped = Vec::new();
        let mut lists = Vec::new();
        for id in ids {
            match self.catch_up(account, &id) {
                Ok((_, mut dropped)) => skipped.append(&mut dropped),
                Err(KnowledgeError::Unavailable(_)) => continue,
                Err(error) => return Err(error),
            }
            let shelf = self.shelf(account, &id);
            lists.push(fused(&shelf.index, &id, query, limit, near, floor)?.into_iter());
        }
        let mut hits = Vec::new();
        for _ in 0..limit {
            for found in &mut lists {
                hits.extend(found.next());
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
        let dir = match self.documents(account, kb)?.usable() {
            Err(KnowledgeError::Unavailable(_)) => return Err(KnowledgeError::UnknownDocument),
            dir => dir?,
        };
        read::page(&dir, path, page, self.extractor.as_deref())
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

    /// 库 `id` 开着自动想起没有（`kb.toml` 的 `auto`，施工 K-6 下）：`kb.toml` 写错的、没写的当关着。不补索引。
    ///
    /// # Errors
    ///
    /// 没有这个库。
    pub fn auto(&self, account: &AccountId, id: &str) -> Result<bool, KnowledgeError> {
        let place = self.place(account, id)?;
        let switched = self.switched(account, id, &place);
        Ok(switched
            .or(catch_up::about(place.dir()).0.auto)
            .unwrap_or(false))
    }

    /// 库 `id` 的每一段（施工 K-6 下，自动想起照标题名挑、照它拼）：先照文件补，交回索引里的每一块写成搜到的那样，和跳过的
    /// 文档。挂的目录用不了的是空的（不补、不报错，同搜的时候跳过）。
    ///
    /// # Errors
    ///
    /// 没有这个库；目录、索引读不了。
    pub fn sections(
        &self,
        account: &AccountId,
        id: &str,
    ) -> Result<(Vec<Hit>, Vec<Skipped>), KnowledgeError> {
        let skipped = match self.catch_up(account, id) {
            Ok((_, skipped)) => skipped,
            Err(KnowledgeError::Unavailable(_)) => return Ok((Vec::new(), Vec::new())),
            Err(error) => return Err(error),
        };
        let shelf = self.shelf(account, id);
        let hits = shelf
            .index
            .entries()?
            .into_iter()
            .filter_map(|(key, text, at)| catch_up::hit(id, &key, &text, at))
            .collect();
        Ok((hits, skipped))
    }

    /// 库 `id` 开着的索引（施工 K-2）：会话照它在后台补缺的向量（同记忆的回合库）。库在不在不查：没有的照样开一份空的。
    #[must_use]
    pub fn index_of(&self, account: &AccountId, id: &str) -> Arc<RecallIndex> {
        Arc::clone(&self.shelf(account, id).index)
    }
}

/// 库 `kb` 里照关键词、照意思（有问句的向量的）各找 `limit` 的四倍，两路照名次合，取 `limit` 块（施工 K-2，同记忆的回合库
/// 那一套，`memory/past.rs`）；只有照意思那一路找到的过 `floor`（施工 K-2 修）。
fn fused(
    index: &RecallIndex,
    kb: &str,
    query: &str,
    limit: usize,
    near: Option<Meaning<'_>>,
    floor: f32,
) -> Result<Vec<Hit>, KnowledgeError> {
    let wide = limit.saturating_mul(4);
    let words = index.search(query, wide)?;
    let close = match near {
        Some(near) => index.nearest(near.model, near.vector, wide)?,
        None => Vec::new(),
    };
    let keys: Vec<String> = words.iter().map(|hit| hit.key.clone()).collect();
    let similar: Vec<(String, f32)> = close
        .iter()
        .map(|hit| (hit.key.clone(), hit.similar))
        .collect();
    // 合完照键找回字和时刻：两路交回的是同一块的同一份。
    let mut found: BTreeMap<String, (String, Timestamp)> = close
        .into_iter()
        .map(|hit| (hit.key, (hit.text, hit.at)))
        .collect();
    found.extend(words.into_iter().map(|hit| (hit.key, (hit.text, hit.at))));
    Ok(miyu_recall::fuse_above(&keys, &similar, floor)
        .into_iter()
        .filter_map(|key| {
            let (text, at) = found.remove(&key)?;
            catch_up::hit(kb, &key, &text, at)
        })
        .take(limit)
        .collect())
}
