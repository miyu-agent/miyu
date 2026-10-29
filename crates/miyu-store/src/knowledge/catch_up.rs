//! 照文件补一个知识库的索引（施工 K-1 上，`docs/blueprint/knowledge.md`「怎么走」第三条）：列出库里的文档，每个算指纹（大小、
//! 改的时刻），和索引里记着的比；新的、变了的读出来切块、整份换（`RecallIndex::replace`），没了的拿掉。读不了、太大、不是
//! UTF-8 的跳过（原来的块照留），交回给调的一方记。挂进来的照挂的目录列（施工 K-4）；挂的目录用不了的不补、索引不动，交回
//! [`KnowledgeError::Unavailable`]；里面读不了的子目录跳过、照跳过的文档记（挂进来的常有，macOS 上受保护的目录）。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use miyu_kernel::time::Timestamp;
use miyu_recall::chunk;

use super::extract::{self, Kind};
use super::watch::Progress;
use super::{Base, Hit, Knowledge, KnowledgeError, Reason, Skipped, skip_stamp};

/// 上次建好的时刻记在索引的哪一格（施工 W-15，`recall/meta.rs`）。
const INDEXED_AT: &str = "indexed_at";

/// 一个文字的文档最大几个字节：再大的多半不是写给人看的笔记（日志、导出的数据），切出来几千块淹掉别的。PDF、网页抽出来的字
/// 也照它封顶（施工 K-5）。
pub(super) const BIGGEST: u64 = 4 * 1024 * 1024;

/// 库里的一个文档。
pub(super) struct Document {
    /// 库里的相对路径，照 `/` 分：来源、键的前一半。
    pub(super) source: String,
    path: PathBuf,
    pub(super) kind: Kind,
    pub(super) size: u64,
    /// 改的时刻（毫秒）。
    pub(super) modified: i64,
}

/// 补完一个库：列出来的样子、跳过的（文档和读不了的子目录）、列出来的文档（施工 K-10：`kb.documents` 照它）。
pub(super) struct Caught {
    pub(super) base: Base,
    pub(super) skipped: Vec<Skipped>,
    pub(super) documents: Vec<Document>,
}

impl Knowledge {
    /// 照文件补库 `id`，交回它列出来的样子和跳过的文档。
    pub(super) fn catch_up(&self, id: &str) -> Result<(Base, Vec<Skipped>), KnowledgeError> {
        self.caught(id).map(|caught| (caught.base, caught.skipped))
    }

    /// 照文件补库 `id`，交回补完的样子（[`Caught`]）。
    pub(super) fn caught(&self, id: &str) -> Result<Caught, KnowledgeError> {
        let dir = self.documents(id)?.usable()?;
        let shelf = self.shelf(id);
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
                reason: Reason::Unreadable,
                why,
                folder: true,
            });
        });
        let stamps = shelf.index.stamps()?;
        let stamp_of = |document: &Document| format!("{}:{}", document.size, document.modified);
        // 要重建的先数出来，好报进度（施工 W-15）：指纹没变的跳过，不算。上次跳过的、文件没变的照记下的原因报跳过，不重读、不
        // 算（施工 W-15 再补：不记的话每次列库都当它变了，重读一遍、推一遍进度）。
        let extractor = self.extractor.is_some();
        let mut changed: Vec<&Document> = Vec::new();
        for document in &documents {
            let stamp = stamp_of(document);
            match stamps.get(&document.source) {
                Some(recorded) if *recorded == stamp => {}
                Some(recorded) => match skip_stamp::read(recorded, &stamp, extractor) {
                    Some((reason, why)) => skipped.push(Skipped {
                        kb: id.to_string(),
                        path: document.source.clone(),
                        reason,
                        why,
                        folder: false,
                    }),
                    None => changed.push(document),
                },
                None => changed.push(document),
            }
        }
        let mut progress = Progress::start(self.watcher.as_ref(), id, changed.len());
        for document in changed {
            progress.one();
            let stamp = stamp_of(document);
            if let Some((reason, why)) = self.rebuild(&shelf.index, document, &stamp)? {
                // 跳过的也记下指纹，连同为什么：原来的块照留。
                shelf
                    .index
                    .stamp_only(&document.source, &skip_stamp::skipped(&stamp, reason, &why))?;
                skipped.push(Skipped {
                    kb: id.to_string(),
                    path: document.source.clone(),
                    reason,
                    why,
                    folder: false,
                });
            }
        }
        let present: BTreeSet<&str> = documents.iter().map(|doc| doc.source.as_str()).collect();
        for source in stamps
            .keys()
            .filter(|source| !present.contains(source.as_str()))
        {
            shelf.index.drop_source(source)?;
        }
        if progress.rebuilt() {
            shelf.index.set_meta(INDEXED_AT, &now().to_string())?;
        }
        progress.finish();
        let mut base = self.counted(id, None)?;
        // 补的锁还拿在这一趟手里：补完了，不算在建。
        base.indexing = false;
        Ok(Caught {
            base,
            skipped,
            documents,
        })
    }

    /// 读一个变了的文档、切块、整份换掉，记下指纹 `stamp`；跳过的（太大、读不成、PDF 里没有字）交回原因和为什么，什么都不写。
    fn rebuild(
        &self,
        index: &crate::recall::RecallIndex,
        document: &Document,
        stamp: &str,
    ) -> Result<Option<(Reason, String)>, KnowledgeError> {
        let biggest = document.kind.biggest();
        if document.size > biggest {
            return Ok(Some((
                Reason::TooLarge,
                format!("larger than {biggest} bytes"),
            )));
        }
        let pages = match extract::pages(document.kind, &document.path, self.extractor.as_deref()) {
            Ok(pages) => pages,
            Err(why) => return Ok(Some(why)),
        };
        if pages.iter().map(String::len).sum::<usize>() as u64 > BIGGEST {
            return Ok(Some((
                Reason::TooLarge,
                format!("text larger than {BIGGEST} bytes"),
            )));
        }
        let chunks = chunks(document.kind, &pages);
        if document.kind == Kind::Pdf && chunks.is_empty() {
            return Ok(Some((
                Reason::NoText,
                "no text in the PDF (scanned or only pictures)".to_string(),
            )));
        }
        // 改的时刻读不出来的（很少见）照 1970 年记：只影响搜到时带的时刻。
        let at = Timestamp::from_unix_millis(document.modified)
            .or_else(|| Timestamp::from_unix_millis(0))
            .ok_or_else(|| KnowledgeError::Io("no usable time".to_string()))?;
        index.replace(&document.source, &chunks, at, stamp)?;
        Ok(None)
    }

    /// 库 `id` 列出来的样子：名字、说明照 `kb.toml`，数照索引里现有的；`unavailable` 是挂的目录用不了的为什么（施工 K-4），
    /// 没有的照 `kb.toml` 读不读得出。
    pub(super) fn counted(
        &self,
        id: &str,
        unavailable: Option<String>,
    ) -> Result<Base, KnowledgeError> {
        let place = self.place(id)?;
        let (about, error) = about(place.dir());
        let switched = self.switched(id, &place);
        let shelf = self.shelf(id);
        let keys = shelf.index.keys()?;
        Ok(Base {
            id: id.to_string(),
            name: about.name.unwrap_or_else(|| id.to_string()),
            description: about.description.unwrap_or_default(),
            documents: skip_stamp::indexed(&shelf.index.stamps()?, &keys),
            chunks: keys.len(),
            error: unavailable.or(error),
            auto: switched.or(about.auto).unwrap_or(false),
            package: place.package().map(str::to_string),
            mount: about.mount,
            indexed_at: shelf.index.meta(INDEXED_AT)?,
            indexing: shelf.catching_up.try_lock().is_err(),
        })
    }
}

/// 这一刻。系统时钟读不出来的（早于 1970 年）照 1970 年记：只影响「上次建好」显示的时刻。
fn now() -> Timestamp {
    let millis = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        });
    Timestamp::from_unix_millis(millis)
        .or_else(|| Timestamp::from_unix_millis(0))
        .expect("1970 年合写法")
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
/// [`Kind::of`]。`base` 是文档的目录：相对路径照它算。读不了的子目录跳过，照 `skip`（相对路径、为什么）交回。
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
        let Some(document) = Kind::of(&name).filter(|_| kind.is_file()) else {
            continue;
        };
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
            kind: document,
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

/// 抽出来的字切成块：Markdown、网页照标题切，纯文本照段落；PDF 一页一页照段落切，每块的头一行记着第几页（从 1 起，搜到的
/// 时候 [`hit`] 拆出来）。
fn chunks(kind: Kind, pages: &[String]) -> Vec<String> {
    let texts = |chunks: Vec<chunk::Chunk>| chunks.into_iter().map(|chunk| chunk.text);
    match kind {
        Kind::Markdown | Kind::Html => pages
            .iter()
            .flat_map(|page| texts(chunk::markdown(page)))
            .collect(),
        Kind::Plain => pages
            .iter()
            .flat_map(|page| texts(chunk::plain(page)))
            .collect(),
        Kind::Pdf => pages
            .iter()
            .enumerate()
            .flat_map(|(n, page)| {
                texts(chunk::plain(page)).map(move |text| format!("{}\n{text}", n + 1))
            })
            .collect(),
    }
}

/// 索引里搜到的一条写成一块：键是 `<相对路径>#<第几块>`；Markdown、网页的头一行是标题路径（`# …`），PDF 的头一行是第几页
/// （施工 K-5）。键不合写法的、不是文档的不要。
pub(super) fn hit(kb: &str, key: &str, text: &str, at: Timestamp) -> Option<Hit> {
    let (path, chunk) = key.rsplit_once('#')?;
    let chunk = chunk.parse().ok()?;
    let first = text.split_once('\n');
    let (heading, page, text) = match (Kind::of(path)?, text.strip_prefix("# ")) {
        (Kind::Markdown | Kind::Html, Some(rest)) => match rest.split_once('\n') {
            Some((heading, text)) => (heading.to_string(), None, text.to_string()),
            None => (rest.to_string(), None, String::new()),
        },
        (Kind::Pdf, _) => {
            let (page, text) = first?;
            (String::new(), Some(page.parse().ok()?), text.to_string())
        }
        _ => (String::new(), None, text.to_string()),
    };
    Some(Hit {
        kb: kb.to_string(),
        path: path.to_string(),
        chunk,
        heading,
        page,
        text,
        at,
    })
}
