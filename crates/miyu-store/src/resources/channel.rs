//! 这一轮的话从哪来那几句（施工 VN-1，`miyu_kernel::facts::Channels`）：`core/facts/channel/` 下 `other.txt`、`nobody.txt`，
//! 和 `head/` 下一种头一份（文件名去掉 `.txt` 是头的种类）。哪几种头有自己的一句是数据：加一种头只加一份文件。

use std::collections::BTreeMap;

use miyu_policy::ChannelTexts;

use super::{ResourceRoot, SourceError};

impl ResourceRoot {
    /// 这一轮的话从哪来那几句：`head/` 里照文件名排，不是 `.txt` 的不算。
    ///
    /// # Errors
    ///
    /// 列不出 `head/`、读不了哪一份，写明是哪一份。
    pub(super) fn channels(&self) -> Result<ChannelTexts, SourceError> {
        let dir = self
            .path
            .join("core")
            .join("facts")
            .join("channel")
            .join("head");
        let listed = std::fs::read_dir(&dir).map_err(|error| SourceError::Read {
            path: dir.clone(),
            error,
        })?;
        let mut kinds: Vec<String> = listed
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                name.strip_suffix(".txt").map(str::to_string)
            })
            .collect();
        kinds.sort();
        let mut heads = BTreeMap::new();
        for kind in kinds {
            let file = format!("{kind}.txt");
            let text = self.read(&["core", "facts", "channel", "head", &file])?;
            heads.insert(kind, text);
        }
        Ok(ChannelTexts {
            heads,
            other: self.read(&["core", "facts", "channel", "other.txt"])?,
            nobody: self.read(&["core", "facts", "channel", "nobody.txt"])?,
        })
    }
}
