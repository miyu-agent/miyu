//! 读一个知识库里的文档（施工 K-1 上，`docs/blueprint/knowledge.md`「怎么走」第五条）：照路径直接读目录里的文件（真相），
//! 不读索引；一页 [`PAGE`] 字节，截在字的边界上。路径只认库里的相对路径：空的、绝对的、带 `.`、`..`、以点开头的一段、反斜杠、
//! 冒号的不认，不是文档的（扩展名）不认。

use std::path::{Path, PathBuf};

use super::KnowledgeError;
use super::catch_up::document;

/// 一页最多几个字节：一次交回去的字，约 8000 token 上下。
const PAGE: usize = 32 * 1024;

/// 文档的一页。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Page {
    /// 这一页的字。
    pub text: String,
    /// 第几页，从 1 起。
    pub page: usize,
    /// 一共几页（空文档是一页空的）。
    pub pages: usize,
}

/// 读库目录 `dir` 里的文档 `path` 的第 `page` 页。
pub(super) fn page(dir: &Path, path: &str, page: usize) -> Result<Page, KnowledgeError> {
    let file = locate(dir, path).ok_or(KnowledgeError::UnknownDocument)?;
    let text = match std::fs::read_to_string(&file) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(KnowledgeError::UnknownDocument);
        }
        Err(error) => return Err(KnowledgeError::Io(error.to_string())),
    };
    let pages = split(&text);
    let Some(chosen) = page.checked_sub(1).and_then(|n| pages.get(n)) else {
        return Err(KnowledgeError::BadPage);
    };
    Ok(Page {
        text: (*chosen).to_string(),
        page,
        pages: pages.len(),
    })
}

/// 库里的相对路径 `path` 对着的文件：合写法的、是文档、在库里、是个文件（不是链接）。
fn locate(dir: &Path, path: &str) -> Option<PathBuf> {
    let file = target(dir, path)?;
    let metadata = std::fs::symlink_metadata(&file).ok()?;
    metadata.is_file().then_some(file)
}

/// 库目录 `dir` 里的相对路径 `path` 照写法该在哪（不看在不在）：照 `/` 分，每一段不是空的、不以点开头、没有反斜杠和冒号，
/// 最后一段是文档（扩展名）。不合的没有。读、写（施工 K-3）同一套。
pub(super) fn target(dir: &Path, path: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = path.split('/').collect();
    let good = parts
        .iter()
        .all(|part| !part.is_empty() && !part.starts_with('.') && !part.contains(['\\', ':']));
    let name = parts.last()?;
    if !good || !document(name) {
        return None;
    }
    Some(
        parts
            .iter()
            .fold(dir.to_path_buf(), |path, part| path.join(part)),
    )
}

/// 照 [`PAGE`] 分页，截在字的边界上；空的是一页空的。
fn split(text: &str) -> Vec<&str> {
    let mut pages = Vec::new();
    let mut rest = text;
    while rest.len() > PAGE {
        let mut end = PAGE;
        while !rest.is_char_boundary(end) {
            end -= 1;
        }
        pages.push(&rest[..end]);
        rest = &rest[end..];
    }
    pages.push(rest);
    pages
}
