//! 交给知识库的两件工具的端口（施工 K-1 下，`docs/blueprint/knowledge.md`「她的工具」）：搜、读一页。
//!
//! 工具只拿这个端口，不认识知识库的目录、索引、人格：执行器照这个会话造一个（实现在 `miyu-session`，查哪几个库照记忆的账号
//! 和人格的收窄），碰磁盘的在阻塞线程里做。工具照它交回的写给她看的那几句。

use crate::Pending;

/// 搜知识库的那件工具的名字：会话查得到至少一个库，工具面上才有它（施工 K-1 下，2026-10-12 项目主人定没有库不给）。
pub const KB_SEARCH: &str = "kb_search";
/// 读知识库里一个文档的那件工具的名字。
pub const KB_READ: &str = "kb_read";

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
