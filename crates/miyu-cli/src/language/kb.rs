//! `miyu kb` 给人看的字（施工 K-7 下，`docs/blueprint/cli/kb.md`「给人看的字」）。

use super::Language;
use miyu_store::knowledge::EXTENSIONS;

impl Language {
    /// `list` 一个库都没有。
    pub fn no_bases(&self) -> &'static str {
        match self {
            Language::Chinese => "还没有知识库。",
            Language::English => "No knowledge bases yet.",
        }
    }

    /// 一个库有几篇。
    pub(crate) fn kb_documents(&self, count: u64) -> String {
        match (self, count) {
            (Language::Chinese, _) => format!("{count} 篇"),
            (Language::English, 1) => "1 document".to_string(),
            (Language::English, _) => format!("{count} documents"),
        }
    }

    /// 挂的目录用不了的库，接在那一行后面。
    pub(crate) fn kb_unavailable(&self, why: &str) -> String {
        match self {
            Language::Chinese => format!("（用不了：{why}）"),
            Language::English => format!(" (unavailable: {why})"),
        }
    }

    /// 搜到的 PDF 那一块是第几页。
    pub(crate) fn kb_pdf_page(&self, page: u64) -> String {
        match self {
            Language::Chinese => format!("第 {page} 页"),
            Language::English => format!("page {page}"),
        }
    }

    /// 读的不止一页：这是第几页、一共几页。
    pub(crate) fn kb_page_of(&self, page: u64, pages: u64) -> String {
        match self {
            Language::Chinese => format!("（第 {page}/{pages} 页）"),
            Language::English => format!("(page {page} of {pages})"),
        }
    }

    /// `new` 建好了。
    pub fn kb_created(&self, kb: &str) -> String {
        match self {
            Language::Chinese => format!("建好了：{kb}"),
            Language::English => format!("Created {kb}"),
        }
    }

    /// `edit` 改好了。
    pub fn kb_updated(&self, kb: &str) -> String {
        match self {
            Language::Chinese => format!("改好了：{kb}"),
            Language::English => format!("Updated {kb}"),
        }
    }

    /// 不认的格式（施工 K-7 修）：只收哪几种、是哪个文件。
    pub(crate) fn kb_not_document(&self, name: &str) -> String {
        let listed = |gap: &str| {
            EXTENSIONS
                .iter()
                .map(|extension| format!(".{extension}"))
                .collect::<Vec<_>>()
                .join(gap)
        };
        match self {
            Language::Chinese => format!("只收 {}：{name}", listed("、")),
            Language::English => format!("Only {} can be added: {name}", listed(", ")),
        }
    }

    /// 库的编号写错了（施工 K-7 修）：说清写法（`miyu_store::layers::valid`）。
    pub(crate) fn kb_bad_id(&self, id: &str) -> String {
        match self {
            Language::Chinese => {
                format!("库的编号要小写字母开头，只能有小写字母、数字、-、_，最多 64 个：{id}")
            }
            Language::English => format!(
                "A knowledge base id starts with a lowercase letter and has only lowercase letters, digits, - and _, at most 64: {id}"
            ),
        }
    }

    /// 页码出界（施工 K-7 修）：第 `page` 页没有，一共 `pages` 页。
    pub(crate) fn kb_no_page(&self, page: u64, pages: u64) -> String {
        match (self, pages) {
            (Language::Chinese, _) => format!("没有第 {page} 页，一共 {pages} 页。"),
            (Language::English, 1) => format!("There is no page {page}; it has 1 page."),
            (Language::English, _) => format!("There is no page {page}; it has {pages} pages."),
        }
    }

    /// `edit` 什么都没写。
    pub fn kb_edit_nothing(&self) -> &'static str {
        match self {
            Language::Chinese => "改什么：写 --name、--description、--auto 或者 --no-auto。",
            Language::English => {
                "Nothing to change: give --name, --description, --auto or --no-auto."
            }
        }
    }

    /// `list` 里开了自动想起的那一格（施工 K-6 补）。
    pub(crate) fn kb_auto(&self) -> &'static str {
        match self {
            Language::Chinese => "自动想起",
            Language::English => "auto recall",
        }
    }

    /// `add` 加进去了一篇。
    pub fn kb_added(&self, kb: &str, path: &str) -> String {
        match self {
            Language::Chinese => format!("加进去了：{kb}/{path}"),
            Language::English => format!("Added {kb}/{path}"),
        }
    }

    /// `remove` 挪进回收站了，在 `trash`。
    pub fn kb_removed(&self, trash: &str) -> String {
        match self {
            Language::Chinese => format!("挪进回收站了：{trash}"),
            Language::English => format!("Moved to the trash: {trash}"),
        }
    }

    /// `mount` 挂上了，有几篇。
    pub fn kb_mounted(&self, kb: &str, documents: u64) -> String {
        match self {
            Language::Chinese => format!("挂上了：{kb}，{documents} 篇"),
            Language::English => format!("Mounted {kb}: {}", self.kb_documents(documents)),
        }
    }

    /// `delete` 删掉了，整个库在回收站的 `trash`。
    pub fn kb_deleted(&self, kb: &str, trash: &str) -> String {
        match self {
            Language::Chinese => format!("删掉了 {kb}，在回收站里：{trash}"),
            Language::English => format!("Deleted {kb}; it is in the trash: {trash}"),
        }
    }

    /// `reindex` 重建好了，有几篇。
    pub fn kb_reindexed(&self, kb: &str, documents: u64) -> String {
        match self {
            Language::Chinese => format!("重建好了：{kb}，{documents} 篇"),
            Language::English => format!("Rebuilt {kb}: {}", self.kb_documents(documents)),
        }
    }

    /// 核心拒了，原话 `said` 后面接上它给的为什么 `why`（挂不上的目录、挪不进回收站）。
    pub(crate) fn refused_because(&self, said: &str, why: &str) -> String {
        let said = said.trim_end_matches(['。', '.']);
        match self {
            Language::Chinese => format!("{said}：{why}"),
            Language::English => format!("{said}: {why}"),
        }
    }
}
