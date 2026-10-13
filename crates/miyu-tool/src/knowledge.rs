//! 交给知识库的几件工具的端口（施工 K-1 下，`docs/blueprint/knowledge.md`「她的工具」）：搜、读一页；写一篇、改一段、挪进
//! 回收站（施工 K-3）。
//!
//! 工具只拿这个端口，不认识知识库的目录、索引、人格：执行器照这个会话造一个（实现在 `miyu-session`，查哪几个库照记忆的账号
//! 和人格的收窄），碰磁盘的在阻塞线程里做。工具照它交回的写给她看的那几句。

use std::path::PathBuf;

use crate::Pending;

/// 搜知识库的那件工具的名字：会话查得到至少一个库，工具面上才有它（施工 K-1 下，2026-10-12 项目主人定没有库不给）。
pub const KB_SEARCH: &str = "kb_search";
/// 读知识库里一个文档的那件工具的名字。
pub const KB_READ: &str = "kb_read";

/// 新建、追加一篇的那件工具的名字（施工 K-3）：知识库这个包装着、预设开着就给，一个库都没有也给（好建第一个，2026-10-12
/// 项目主人定）。
pub const KB_SAVE: &str = "kb_save";
/// 照原文改一段的那件工具的名字（施工 K-3）：同 `kb_search`，查得到至少一个库才给。
pub const KB_EDIT: &str = "kb_edit";
/// 把一篇挪进回收站的那件工具的名字（施工 K-3）：同上。
pub const KB_DELETE: &str = "kb_delete";

/// `kb_search` 最多给几块：一块最多 1200 字节（`miyu_recall::chunk::LIMIT`），五块六千字节上下，一次看得完（出厂值只在这一处，
/// 以后要改再进配置）。
pub const KB_HITS: usize = 5;

/// 知识库的端口。
pub trait KnowledgePort: Send + Sync {
    /// 照关键词搜她查得到的库（写了 `kb` 的只搜这一个），最多 [`KB_HITS`] 块，几个库照名次轮流合。
    fn search<'a>(
        &'a self,
        query: String,
        kb: Option<String>,
    ) -> Pending<'a, Result<Vec<KbHit>, KbRefused>>;

    /// 读库 `kb` 里的文档 `path` 的第 `page` 页（从 1 起）。
    fn read<'a>(
        &'a self,
        kb: String,
        path: String,
        page: usize,
    ) -> Pending<'a, Result<KbPage, KbRefused>>;

    /// 写一篇（施工 K-3）：新建，或者 `append` 接在后面；库还没有的建这个库。
    fn save<'a>(&'a self, save: KbSave) -> Pending<'a, Result<KbWritten, KbRefused>>;

    /// 把库 `kb` 里 `path` 那一篇的 `old` 换成 `new`（施工 K-3）：`old` 得正好出现一次。
    fn edit<'a>(
        &'a self,
        kb: String,
        path: String,
        old: String,
        new: String,
    ) -> Pending<'a, Result<KbWritten, KbRefused>>;

    /// 把库 `kb` 里 `path` 那一篇挪进系统回收站（施工 K-3）。
    fn delete<'a>(&'a self, kb: String, path: String) -> Pending<'a, Result<KbTrashed, KbRefused>>;
}

/// 写一篇要的（施工 K-3）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KbSave {
    /// 哪个库：还没有的建它。
    pub kb: String,
    /// 库里的相对路径。
    pub path: String,
    /// 写进去的字。
    pub text: String,
    /// 接在这一篇后面（没有就新建）；不是的话新建，已经有了不写。
    pub append: bool,
    /// 建库时这个库是什么，一句：写进它的 `kb.toml`。库已经有了的不管。
    pub about: Option<String>,
}

/// 写进去的一篇（施工 K-3）：工具照它报「改了文件」的效果，撤销时改回去。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KbWritten {
    /// 文件在哪（绝对路径）。
    pub path: PathBuf,
    /// 改前的内容；新建的没有。
    pub before: Option<Vec<u8>>,
    /// 改后的内容。
    pub after: Vec<u8>,
    /// 这一次建了这个库。
    pub created: bool,
}

/// 挪进回收站的一篇（施工 K-3）：工具照它报「挪进回收站」的效果，撤销时移回来。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KbTrashed {
    /// 原来在哪（绝对路径）。
    pub path: PathBuf,
    /// 回收站里在哪，各平台自己的写法（同 `trash`）。
    pub trash: String,
}

/// 搜到的一块。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KbHit {
    /// 哪个库。
    pub kb: String,
    /// 哪个文档（库里的相对路径）。
    pub path: String,
    /// 标题路径（`语法 › て形`），没有标题的是空的。
    pub heading: String,
    /// PDF 的第几页，从 1 起（施工 K-5）；别的文档没有。
    pub page: Option<usize>,
    /// 这一块的字。
    pub text: String,
}

/// 读到的一页。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KbPage {
    /// 这一页的字。
    pub text: String,
    /// 第几页，从 1 起。
    pub page: usize,
    /// 一共几页。
    pub pages: usize,
}

/// 搜不了、读不了。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KbRefused {
    /// 没有这个库：不在、她查不到（人格没列的）都算，不让她知道有。
    NoBase(String),
    /// 库里没有这个文档：哪个库、哪个文档（不是文档的、路径跑出去的都算）。
    NoDocument(String, String),
    /// 没有这一页。
    NoPage(usize),
    /// 新建的那一篇已经有了：哪个库、哪一篇（施工 K-3）。
    Exists(String, String),
    /// 写完太大（施工 K-3）：哪一篇。
    TooLarge(String),
    /// 要换的原文找不到（施工 K-3）：哪一篇。
    NoMatch(String),
    /// 要换的原文不止一处（施工 K-3）：哪一篇、几处。
    Ambiguous(String, usize),
    /// 挪不进回收站（施工 K-3）：哪一篇、为什么。
    Trash(String, String),
    /// 要建的库编号不合写法（施工 K-3 补）：她写的编号。不说「没有这个库」：她是要建，得说清编号怎么写。
    BadId(String),
    /// 写到挂进来的库（施工 K-4，19 N7：只读）：哪个库。
    ReadOnly(String),
    /// 写到软件包带来的库（施工 K-8：只读）：哪个库。
    Packaged(String),
    /// 写 PDF、网页（施工 K-5：只有文字的文档写得了）：哪一篇。
    NotText(String),
    /// 读不了：为什么。
    Failed(String),
}

impl std::fmt::Debug for dyn KnowledgePort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("KnowledgePort")
    }
}

/// 两个端口比的是不是同一个：[`crate::Call`] 照格子比较时用。
impl PartialEq for dyn KnowledgePort {
    fn eq(&self, other: &dyn KnowledgePort) -> bool {
        std::ptr::addr_eq(self, other)
    }
}

impl Eq for dyn KnowledgePort {}
