//! 文档里的字（施工 K-5，`docs/blueprint/knowledge.md`「对外的样子」文档那一段、「怎么走」第二、五条）：Markdown、纯文本照原样
//! （得是 UTF-8）；PDF 一页一页抽（`pdf-extract`），部首的码位换回平常的字（[`radicals`]）；网页换成文字（`html2text`，标题写成
//! Markdown 的 `#`，脚本、样式不要，链接只留字），照 Markdown 切。补、读共用这一套。

mod radicals;

use super::catch_up::BIGGEST;

/// PDF、网页的文件最大几个字节：PDF 常带图，比文字的文档大得多；抽出来的字照旧以 [`BIGGEST`] 封顶。
pub(super) const BIGGEST_FILE: u64 = 64 * 1024 * 1024;

/// 网页换成文字时一行多宽：给得很宽就是不折行（折了的行照段落切会断在半句）。
const WIDE: usize = 1 << 20;

/// 一个文档是哪一种：照扩展名（小写比）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    /// `.md`、`.markdown`：照标题切。
    Markdown,
    /// `.txt`：照段落切。
    Plain,
    /// `.pdf`：一页一页抽，照页切。
    Pdf,
    /// `.html`、`.htm`：换成文字，照标题切。
    Html,
}

impl Kind {
    /// 文件名 `name` 是哪一种文档；扩展名不认的、只有扩展名的（`.md`）不是文档。
    pub(super) fn of(name: &str) -> Option<Kind> {
        let (stem, extension) = name.rsplit_once('.')?;
        if stem.is_empty() {
            return None;
        }
        match extension.to_ascii_lowercase().as_str() {
            "md" | "markdown" => Some(Kind::Markdown),
            "txt" => Some(Kind::Plain),
            "pdf" => Some(Kind::Pdf),
            "html" | "htm" => Some(Kind::Html),
            _ => None,
        }
    }

    /// 她写得了的：文字的两种（施工 K-5：PDF、网页照原样存着，写进字会弄坏）。
    pub(super) fn writable(self) -> bool {
        matches!(self, Kind::Markdown | Kind::Plain)
    }

    /// 文件最大几个字节。
    pub(super) fn biggest(self) -> u64 {
        match self {
            Kind::Markdown | Kind::Plain => BIGGEST,
            Kind::Pdf | Kind::Html => BIGGEST_FILE,
        }
    }
}

/// 文件的字节 `bytes` 抽出来的字：PDF 一页一份，别的只有一份。抽不出来的交回为什么（英文的一句）。
pub(super) fn pages(kind: Kind, bytes: Vec<u8>) -> Result<Vec<String>, String> {
    match kind {
        Kind::Markdown | Kind::Plain => match String::from_utf8(bytes) {
            Ok(text) => Ok(vec![text]),
            Err(_) => Err("not UTF-8".to_string()),
        },
        Kind::Html => html2text::config::plain_no_decorate()
            .string_from_read(bytes.as_slice(), WIDE)
            .map(|text| vec![text])
            .map_err(|error| error.to_string()),
        Kind::Pdf => pdf(&bytes),
    }
}

/// 一页一页抽 PDF 的字，每页去掉头尾的空白（排版留下的空行）。抽的库碰到坏文件可能 panic，接住了当抽不出来。
fn pdf(bytes: &[u8]) -> Result<Vec<String>, String> {
    let extracted = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem_by_pages(bytes));
    match extracted {
        Ok(Ok(pages)) => Ok(pages
            .iter()
            .map(|page| radicals::plain(page.trim()))
            .collect()),
        Ok(Err(error)) => Err(format!("unreadable PDF: {error}")),
        Err(_) => Err("unreadable PDF".to_string()),
    }
}
