//! 知识库抽字的小程序（施工 K-5，`docs/blueprint/knowledge.md`「怎么走」第二条第 5、6 款）：一次抽一份 PDF 或网页里的字，照
//! JSON 交回去就退。核心照子进程拉起它（主程序真实位置的旁边），不链抽字的两个库：编进核心空闲时多 1.6 MB 上下的常驻，过了
//! 设计 23 的预算；放在用完就退的小程序里，抽坏文件卡住了核心也杀得掉。
//!
//! 用法：`miyu-extract pdf <文件>`、`miyu-extract html <文件>`。抽成了的标准输出写一个 JSON 的字符串数组（PDF 一页一个，网页
//! 只有一个），退出码 0；抽不出来的标准错误写一句英文的为什么，退出码 1。
//!
//! - PDF（`pdf-extract`）：一页一页抽，每页去掉头尾的空白（排版留下的空行），部首的码位换回平常的字（`radicals.rs`）。抽的库碰到
//!   坏文件可能 panic，接住了当抽不出来。
//! - 网页（`html2text` 的 `plain_no_decorate`）：标题写成 Markdown 的 `#`，脚本、样式不要，粗体这些不加记号，链接、图只留字；
//!   不折行（折了的行照段落切会断在半句）。

mod radicals;

use std::ffi::OsString;
use std::path::Path;

/// 网页换成文字时一行多宽：给得很宽就是不折行。
const WIDE: usize = 1 << 20;

/// 照命令行的参数 `args`（不带程序名）抽一份：交回要写到标准输出的 JSON。
///
/// # Errors
///
/// 参数不对、文件读不了、抽不出来：英文的一句为什么。
pub fn run(args: &[OsString]) -> Result<String, String> {
    let [kind, file] = args else {
        return Err("usage: miyu-extract <pdf|html> <file>".to_string());
    };
    let bytes = std::fs::read(Path::new(file))
        .map_err(|error| format!("{}: {error}", Path::new(file).display()))?;
    let pages = match kind.to_str() {
        Some("pdf") => pdf(&bytes)?,
        Some("html") => vec![html(&bytes)?],
        _ => return Err(format!("unknown kind {}", kind.to_string_lossy())),
    };
    serde_json::to_string(&pages).map_err(|error| error.to_string())
}

/// PDF 一页一页的字：每页去掉头尾的空白，部首的码位换回平常的字。
///
/// # Errors
///
/// 不是 PDF、坏了、加了密：英文的一句为什么。
pub fn pdf(bytes: &[u8]) -> Result<Vec<String>, String> {
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

/// 网页换成的文字。
///
/// # Errors
///
/// 解析不了：英文的一句为什么。
pub fn html(bytes: &[u8]) -> Result<String, String> {
    html2text::config::plain_no_decorate()
        .string_from_read(bytes, WIDE)
        .map_err(|error| error.to_string())
}
