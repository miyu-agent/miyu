//! 照文件补一个知识库的索引（施工 K-1 上，`docs/blueprint/knowledge.md`「怎么走」第三条）：列出库里的文档，每个算指纹（大小、
//! 改的时刻），和索引里记着的比；新的、变了的读出来切块、整份换（`RecallIndex::replace`），没了的拿掉。读不了、太大、不是
//! UTF-8 的跳过（原来的块照留），交回给调的一方记。挂进来的照挂的目录列（施工 K-4）；挂的目录用不了的不补、索引不动，交回
//! [`KnowledgeError::Unavailable`]；里面读不了的子目录跳过、照跳过的文档记（挂进来的常有，macOS 上受保护的目录）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;
use miyu_recall::chunk;

use super::{Base, Hit, Knowledge, KnowledgeError, Skipped};

/// 一个文档最大几个字节：再大的多半不是写给人看的笔记（日志、导出的数据），切出来几千块淹掉别的。
pub(super) const BIGGEST: u64 = 4 * 1024 * 1024;

/// 算文档的扩展名（小写比）。
pub(super) const EXTENSIONS: [&str; 3] = ["md", "markdown", "txt"];

/// 库里的一个文档。
struct Document {
    /// 库里的相对路径，照 `/` 分：来源、键的前一半。
    source: String,
    path: PathBuf,
    size: u64,
    /// 改的时刻（毫秒）。
    modified: i64,
}

impl Knowledge {
    /// 照文件补库 `id`，交回它列出来的样子和跳过的文档。
    pub(super) fn catch_up(
        &self,
        account: &AccountId,
        id: &str,
    ) -> Result<(Base, Vec<Skipped>), KnowledgeError> {
        let dir = self.documents(account, id)?.usable()?;
        let shelf = self.shelf(account, id);
        let _only_one = shelf
            .catching_up
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut documents = Vec::new();
        let mut skipped = Vec::new();
        let read =
            std::fs::read_dir(&dir).map_err(|error| KnowledgeError::Io(error.to_string()))?;
        scan(&dir, read, &mut documents, &mut |source, why| {
            skipped.push(Skipped {
                kb: id.to_string(),
                path: source,
                why,
            });
        });
        let stamps = shelf.index.stamps()?;
        for document in &documents {
            let stamp = format!("{}:{}", document.size, document.modified);
            if stamps.get(&document.source) == Some(&stamp) {
                continue;
            }
            let skip = |why: String| Skipped {
                kb: id.to_string(),
                path: document.source.clone(),
                why,
            };
            if document.size > BIGGEST {
                skipped.push(skip(format!("larger than {BIGGEST} bytes")));
                continue;
            }
            let text = match std::fs::read(&document.path).map(String::from_utf8) {
                Ok(Ok(text)) => text,
                Ok(Err(_)) => {
                    skipped.push(skip("not UTF-8".to_string()));
                    continue;
                }
                Err(error) => {
                    skipped.push(skip(error.to_string()));
                    continue;
                }
            };
            let chunks: Vec<String> = match markdown(&document.source) {
                true => chunk::markdown(&text),
                false => chunk::plain(&text),
            }
            .into_iter()
            .map(|chunk| chunk.text)
            .collect();
            // 改的时刻读不出来的（很少见）照 1970 年记：只影响搜到时带的时刻。
            let at = Timestamp::from_unix_millis(document.modified)
                .or_else(|| Timestamp::from_unix_millis(0))
                .ok_or_else(|| KnowledgeError::Io("no usable time".to_string()))?;
            shelf.index.replace(&document.source, &chunks, at, &stamp)?;
        }
        let present: BTreeSet<&str> = documents.iter().map(|doc| doc.source.as_str()).collect();
        for source in stamps
            .keys()
            .filter(|source| !present.contains(source.as_str()))
        {
            shelf.index.drop_source(source)?;
        }
        Ok((self.counted(account, id, None)?, skipped))
    }

    /// 库 `id` 列出来的样子：名字、说明照 `kb.toml`，数照索引里现有的；`unavailable` 是挂的目录用不了的为什么（施工 K-4），
    /// 没有的照 `kb.toml` 读不读得出。
    pub(super) fn counted(
        &self,
        account: &AccountId,
        id: &str,
        unavailable: Option<String>,
    ) -> Result<Base, KnowledgeError> {
        let (about, error) = about(&self.dir(account, id)?);
        let shelf = self.shelf(account, id);
        Ok(Base {
            id: id.to_string(),
            name: about.name.unwrap_or_else(|| id.to_string()),
            description: about.description.unwrap_or_default(),
            documents: shelf.index.stamps()?.len(),
            chunks: shelf.index.keys()?.len(),
            error: unavailable.or(error),
        })
    }
}

/// 读 `kb.toml`：没有的照空的；读不出来的照空的、交回为什么。
pub(super) fn about(dir: &Path) -> (miyu_recall::kb::About, Option<String>) {
    match std::fs::read_to_string(dir.join("kb.toml")) {
        Ok(text) => match miyu_recall::kb::about(&text) {
            Ok(about) => (about, None),
            Err(why) => (miyu_recall::kb::About::default(), Some(why)),
        },
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            (miyu_recall::kb::About::default(), None)
        }
        Err(error) => (miyu_recall::kb::About::default(), Some(error.to_string())),
    }
}

/// 列出 `entries`（一个目录读出来的）里的文档（子目录照样），接在 `out` 后面：以点开头的、链接不算，扩展名照
/// [`EXTENSIONS`]。`base` 是文档的目录：相对路径照它算。读不了的子目录跳过，照 `skip`（相对路径、为什么）交回。
fn scan(
    base: &Path,
    entries: std::fs::ReadDir,
    out: &mut Vec<Document>,
    skip: &mut dyn FnMut(String, String),
) {
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        if name.starts_with('.') || kind.is_symlink() {
            continue;
        }
        let path = entry.path();
        if kind.is_dir() {
            match std::fs::read_dir(&path) {
                Ok(inner) => scan(base, inner, out, skip),
                Err(error) => skip(relative(base, &path), error.to_string()),
            }
            continue;
        }
        if !kind.is_file() || !document(&name) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            continue;
        };
        let modified = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .and_then(|since| i64::try_from(since.as_millis()).ok())
            .unwrap_or(0);
        out.push(Document {
            source: relative(base, &path),
            path,
            size: metadata.len(),
            modified,
        });
    }
}

/// `path` 在 `base` 里的相对路径，照 `/` 分。
fn relative(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// 文件名是不是文档：扩展名照 [`EXTENSIONS`]（小写比）。
pub(super) fn document(name: &str) -> bool {
    name.rsplit_once('.').is_some_and(|(stem, extension)| {
        !stem.is_empty() && EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// 是不是 Markdown：照扩展名。
fn markdown(source: &str) -> bool {
    source.rsplit_once('.').is_some_and(|(_, extension)| {
        matches!(extension.to_ascii_lowercase().as_str(), "md" | "markdown")
    })
}

/// 索引里搜到的一条写成一块：键是 `<相对路径>#<第几块>`；Markdown 的头一行是标题路径（`# …`）。键不合写法的不要。
pub(super) fn hit(kb: &str, key: &str, text: &str, at: Timestamp) -> Option<Hit> {
    let (path, chunk) = key.rsplit_once('#')?;
    let chunk = chunk.parse().ok()?;
    let (heading, text) = match (markdown(path), text.strip_prefix("# ")) {
        (true, Some(rest)) => match rest.split_once('\n') {
            Some((heading, text)) => (heading.to_string(), text.to_string()),
            None => (rest.to_string(), String::new()),
        },
        _ => (String::new(), text.to_string()),
    };
    Some(Hit {
        kb: kb.to_string(),
        path: path.to_string(),
        chunk,
        heading,
        text,
        at,
    })
}
