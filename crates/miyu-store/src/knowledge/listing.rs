//! 列出库里的文档（施工 K-10，`docs/blueprint/knowledge.md`「对外的样子」`kb.documents`）：先照文件补，交回补的时候列出来的
//! 每一篇（照路径排）、索引里它有几块、这一回读不了的为什么，和读不了的子目录。不是文档的文件、点开头的、链接不列，同补的时候。

use std::collections::BTreeMap;

use miyu_kernel::id::AccountId;
use miyu_kernel::time::Timestamp;

use super::{DocumentKind, Knowledge, KnowledgeError, Reason, Skipped};

/// 库里的一篇文档：列出来的样子。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    /// 库里的相对路径，照 `/` 分。
    pub path: String,
    /// 哪一种。
    pub kind: DocumentKind,
    /// 文件的字节数。
    pub size: u64,
    /// 文件改的时刻。
    pub at: Timestamp,
    /// 索引里有几块：这一回读不了的、以前进过索引的是留着的旧块。
    pub chunks: usize,
    /// 这一回补的时候读不了的：哪一种、为什么（英文的一句）。
    pub skipped: Option<(Reason, String)>,
}

/// 一个库里的文档。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Listing {
    /// 照路径排。
    pub documents: Vec<Document>,
    /// 读不了的子目录（[`Skipped::folder`]），照路径排。
    pub folders: Vec<Skipped>,
}

impl Knowledge {
    /// 库 `id` 里的文档：先照文件补，交回列出来的样子和跳过的（调的一方照它记运行日志，同 [`Knowledge::list`]）。
    ///
    /// # Errors
    ///
    /// 没有这个库；挂的目录用不了（[`KnowledgeError::Unavailable`]）；目录、索引读不了。
    pub fn listing(
        &self,
        account: &AccountId,
        id: &str,
    ) -> Result<(Listing, Vec<Skipped>), KnowledgeError> {
        let caught = self.caught(account, id)?;
        let mut chunks: BTreeMap<String, usize> = BTreeMap::new();
        for key in self.shelf(account, id).index.keys()? {
            if let Some((source, _)) = key.rsplit_once('#') {
                *chunks.entry(source.to_string()).or_default() += 1;
            }
        }
        let unread: BTreeMap<&str, &Skipped> = caught
            .skipped
            .iter()
            .filter(|one| !one.folder)
            .map(|one| (one.path.as_str(), one))
            .collect();
        let mut documents = Vec::new();
        for document in &caught.documents {
            // 改的时刻读不出来的照 1970 年，同补的时候。
            let Some(at) = Timestamp::from_unix_millis(document.modified)
                .or_else(|| Timestamp::from_unix_millis(0))
            else {
                continue;
            };
            documents.push(Document {
                path: document.source.clone(),
                kind: document.kind,
                size: document.size,
                at,
                chunks: chunks.get(&document.source).copied().unwrap_or(0),
                skipped: unread
                    .get(document.source.as_str())
                    .map(|one| (one.reason, one.why.clone())),
            });
        }
        documents.sort_by(|a, b| a.path.cmp(&b.path));
        let mut folders: Vec<Skipped> = caught
            .skipped
            .iter()
            .filter(|one| one.folder)
            .cloned()
            .collect();
        folders.sort_by(|a, b| a.path.cmp(&b.path));
        Ok((Listing { documents, folders }, caught.skipped))
    }
}
