//! `miyu kb` 给人看的字（施工 K-7 下，`docs/blueprint/cli/kb.md`「给人看的字」；CL-2 补照设计 47 第三节改：结果行动词打头、
//! 不带句号，报错交 [`Problem`]，建议另起一行）。

use super::Language;
use crate::diagnostic::Problem;
use miyu_store::knowledge::EXTENSIONS;

impl Language {
    /// `list` 一个库都没有。
    pub fn no_bases(&self) -> &'static str {
        match self {
            Language::Chinese => "没有知识库",
            Language::English => "no knowledge bases",
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

    /// `list <库>` 一篇都没有（施工 K-10）。
    pub fn kb_no_documents(&self) -> &'static str {
        match self {
            Language::Chinese => "没有文档",
            Language::English => "no documents",
        }
    }

    /// `list <库>` 这一回读不了的那一篇、那个子目录，接在那一行后面（施工 K-10）：照核心的 `reason`，不认得的照原样。
    pub(crate) fn kb_unread(&self, reason: &str) -> String {
        let chinese = match reason {
            "too_large" => "文件过大",
            "not_utf8" => "不是 UTF-8 编码",
            "no_text" => "PDF 里没有文字（扫描件或只有图片）",
            "no_extractor" => "缺少 miyu-extract",
            "extract_failed" => "提取文字失败",
            "unreadable" => "没有权限或读取失败",
            other => other,
        };
        let english = match reason {
            "too_large" => "too large",
            "not_utf8" => "not UTF-8",
            "no_text" => "no text in the PDF (scanned or images only)",
            "no_extractor" => "miyu-extract is missing",
            "extract_failed" => "text extraction failed",
            "unreadable" => "no permission or read failed",
            other => other,
        };
        match self {
            Language::Chinese => format!("  （无法读取：{chinese}）"),
            Language::English => format!("  (unreadable: {english})"),
        }
    }

    /// 挂的目录用不了的库，接在那一行后面。
    pub(crate) fn kb_unavailable(&self, why: &str) -> String {
        match self {
            Language::Chinese => format!("（不可用：{why}）"),
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
            Language::Chinese => format!("已创建 {kb}"),
            Language::English => format!("created {kb}"),
        }
    }

    /// `edit` 改好了。
    pub fn kb_updated(&self, kb: &str) -> String {
        match self {
            Language::Chinese => format!("已修改 {kb}"),
            Language::English => format!("updated {kb}"),
        }
    }

    /// 不认的格式（施工 K-7 修）：只收哪几种、是哪个文件。
    pub(crate) fn kb_not_document(&self, name: &str) -> Problem {
        let listed = |gap: &str| {
            EXTENSIONS
                .iter()
                .map(|extension| format!(".{extension}"))
                .collect::<Vec<_>>()
                .join(gap)
        };
        match self {
            Language::Chinese => Problem::new(format!("无法添加 {name}：不支持的格式"))
                .hint(format!("支持的格式：{}", listed("、"))),
            Language::English => Problem::new(format!("cannot add {name}: unsupported format"))
                .hint(format!("supported formats: {}", listed(", "))),
        }
    }

    /// 库的编号写错了（施工 K-7 修）：说清写法（`miyu_store::layers::valid`）。
    pub(crate) fn kb_bad_id(&self, id: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("知识库编号无效：{id}")).hint(
                "编号以小写字母开头，只含小写字母、数字、- 和 _，最多 64 个字符",
            ),
            Language::English => Problem::new(format!("invalid knowledge base id: {id}")).hint(
                "start with a lowercase letter, use only lowercase letters, digits, - and _, at most 64 characters",
            ),
        }
    }

    /// 页码出界（施工 K-7 修）：第 `page` 页没有，一共 `pages` 页。
    pub(crate) fn kb_no_page(&self, page: u64, pages: u64) -> Problem {
        Problem::new(match (self, pages) {
            (Language::Chinese, _) => format!("没有第 {page} 页：共 {pages} 页"),
            (Language::English, 1) => format!("no page {page}: the document has 1 page"),
            (Language::English, _) => format!("no page {page}: the document has {pages} pages"),
        })
    }

    /// `edit` 什么都没写。
    pub(crate) fn kb_edit_nothing(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("没有要修改的内容")
                .hint("加上 '--name'、'--description'、'--auto' 或 '--no-auto'"),
            Language::English => Problem::new("nothing to change")
                .hint("pass '--name', '--description', '--auto' or '--no-auto'"),
        }
    }

    /// `list` 里软件包带来的那一格（施工 K-8）：哪个包。
    pub(crate) fn kb_from(&self, package: &str) -> String {
        match self {
            Language::Chinese => format!("来自 {package}"),
            Language::English => format!("from {package}"),
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
            Language::Chinese => format!("已添加 {kb}/{path}"),
            Language::English => format!("added {kb}/{path}"),
        }
    }

    /// `remove` 挪进回收站了，在 `trash`。
    pub fn kb_removed(&self, trash: &str) -> String {
        match self {
            Language::Chinese => format!("已移到回收站：{trash}"),
            Language::English => format!("moved to trash: {trash}"),
        }
    }

    /// `mount` 挂上了，有几篇。
    pub fn kb_mounted(&self, kb: &str, documents: u64) -> String {
        match self {
            Language::Chinese => format!("已挂载 {kb}：{documents} 篇"),
            Language::English => format!("mounted {kb}: {}", self.kb_documents(documents)),
        }
    }

    /// `delete` 删掉了，整个库在回收站的 `trash`。
    pub fn kb_deleted(&self, kb: &str, trash: &str) -> String {
        match self {
            _ if trash.is_empty() => self.kb_deleted_only(kb),
            Language::Chinese => format!("已删除 {kb}，移到回收站：{trash}"),
            Language::English => format!("deleted {kb}, moved to trash: {trash}"),
        }
    }

    /// `delete` 挂进来的库：只拿掉了登记，没有进回收站的。
    fn kb_deleted_only(&self, kb: &str) -> String {
        match self {
            Language::Chinese => format!("已删除 {kb}"),
            Language::English => format!("deleted {kb}"),
        }
    }

    /// `reindex` 重建好了，有几篇。
    pub fn kb_reindexed(&self, kb: &str, documents: u64) -> String {
        match self {
            Language::Chinese => format!("已重建 {kb} 的索引：{documents} 篇"),
            Language::English => format!("reindexed {kb}: {}", self.kb_documents(documents)),
        }
    }
}
