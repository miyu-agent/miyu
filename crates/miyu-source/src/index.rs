//! 索引（施工 G-2 上，设计 `37-软件源.md` 第三节）：软件源里有哪些包、各在哪下、哈希是多少。`index.json` 一份，签名另放
//! `index.json.sig`（[`crate::sign`]）。
//!
//! - 读的时候认 `version` 是 1 的；以后加格子不升版本，旧的客户端不认识的格子照略过（不用 `deny_unknown_fields`）。
//! - 签名签的是收到的那一串字节，存的时候原样存，不重新排版。
//! - `generated` 是 RFC 3339 的时刻：本机存着的更新的，拿来的旧的不收（第三节第 3 条，防有人拿一份签过名的旧索引让人
//!   装回旧版）。

use std::collections::BTreeMap;
use std::fmt;

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

/// 认得的索引版本。
pub const VERSION: u32 = 1;

/// 一份索引。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    /// 写法的版本，现在是 [`VERSION`]。
    pub version: u32,
    /// 什么时候生成的，RFC 3339。
    pub generated: String,
    /// 收录的包，照编号排。
    #[serde(default)]
    pub packages: Vec<Entry>,
    /// 拉黑的：装着的人那边提示（设计 31 第八节第 5 条）。
    #[serde(default)]
    pub blocked: Vec<Blocked>,
}

/// 索引里的一个包：都从包文件里的清单读出来，作者不另写。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    /// 编号。
    pub id: String,
    /// 版本：只列最新的那一版。
    pub version: String,
    /// 名字，语言到字。
    #[serde(default)]
    pub name: BTreeMap<String, String>,
    /// 一句说明，语言到字。
    #[serde(default)]
    pub summary: BTreeMap<String, String>,
    /// 来源仓库，例如 `github.com/miyu-agent/miyu-embed`：编号跟着它（`names.toml`），升级时来源变了的不升。
    pub origin: String,
    /// 来源仓库是不是项目主人自己的（`official.toml`）。
    #[serde(default)]
    pub official: bool,
    /// 包的种类（施工 G-11，网页的软件市场照它筛）：清单里 `[package] kind` 的原样写法（`process`、`theme`、`bundle` 这些），
    /// 以后多了的照样原样。以前生成的索引没有这一格。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// Lucide 的图标名（施工 F-18 下）：清单 `[package] icon` 的原样；没写的、以前生成的索引没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// 包自带的图标（施工 F-18 下）：放在软件源的哪、哈希、大小；没写、不合规矩的、以前生成的索引没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub logo: Option<Logo>,
    /// 带了哪几样：种类（`features`、`skills`、`personas` 这些）到名字。
    #[serde(default)]
    pub provides: BTreeMap<String, Vec<String>>,
    /// 扩展要的能力。
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// 依赖的包的编号。
    #[serde(default)]
    pub depends: Vec<String>,
    /// 推荐的包的编号（施工 IN-1）：清单 `[recommends]` 的原样，照写的先后。没写的、以前生成的索引没有。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub recommends: Vec<String>,
    /// 作者（施工 IN-1）：清单 `[package] author` 的原样；没写的、以前生成的索引没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// 上游（施工 IN-1）：清单 `[package] upstream` 的原样；没写的、以前生成的索引没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub upstream: Option<String>,
    /// 装好以后占多少字节（施工 IN-1）：包文件解开以后所有普通文件的字节数之和；一个包有几份文件（分系统的）时取最大的那一份。
    /// 以前生成的索引没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installed: Option<u64>,
    /// 包文件：不分系统的一份，或者一种系统一份。
    pub files: Vec<File>,
}

impl Entry {
    /// 写法对不对（施工 AS-2）：编号、依赖、推荐照包编号的写法（`plain_id`），版本只许字母、数字、`.`、`_`、`+`、`-`、
    /// 不以 `.` 开头、最多 64 个。编号、版本要拼进缓存里的文件名（`<编号>-<版本>.miyu`），别人的软件源写的索引不全信：带
    /// `/`、`..`、`\` 的能让下载落到缓存外面。同 [`Logo::well_formed`]，读索引时不合的整条丢掉。
    pub fn well_formed(&self) -> bool {
        plain_id(&self.id)
            && plain_version(&self.version)
            && self.depends.iter().all(|id| plain_id(id))
            && self.recommends.iter().all(|id| plain_id(id))
    }
}

/// 包编号的写法：小写字母开头，小写字母、数字、`-`、`_`，最多 64 个（同 `miyu_store::personas::valid`，这个 crate 不依赖
/// `miyu-store`，照写一份）。
fn plain_id(id: &str) -> bool {
    let mut chars = id.chars();
    chars.next().is_some_and(|first| first.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && id.len() <= 64
}

/// 版本的写法：字母、数字、`.`、`_`、`+`、`-`，不以 `.` 开头，最多 64 个。
fn plain_version(version: &str) -> bool {
    !version.is_empty()
        && !version.starts_with('.')
        && version.len() <= 64
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '+' | '-'))
}

/// 一个包文件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct File {
    /// 给哪一种系统（`linux-x86_64` 这些）；`None` 是哪种都行。
    #[serde(default)]
    pub target: Option<String>,
    /// 在哪下。
    pub url: String,
    /// SHA-256，小写十六进制。
    pub sha256: String,
    /// 多少字节。
    pub size: u64,
}

/// 索引里一个包自带的图标（施工 F-18 下）：收录时从包文件里取出来，和索引放在一处发。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Logo {
    /// 相对软件源地址的位置：`logos/<SHA-256>.png`、`.webp`（[`Logo::path_for`]）。
    pub path: String,
    /// SHA-256，小写十六进制。
    pub sha256: String,
    /// 多少字节。
    pub size: u64,
}

impl Logo {
    /// 内容的 SHA-256 `sha256`、扩展名 `extension`（`png`、`webp`）的图标放在哪。
    pub fn path_for(sha256: &str, extension: &str) -> String {
        format!("logos/{sha256}.{extension}")
    }

    /// 图标的版本：SHA-256 的前 16 位，同装着的包（`package.list` 的 `logo`）。
    pub fn version(&self) -> &str {
        self.sha256.get(..16).unwrap_or(&self.sha256)
    }

    /// 写法对不对：哈希是 64 位小写十六进制，位置正好是 [`Logo::path_for`] 照它算出来的那样。别人的软件源写的索引不全信：
    /// 位置写成别的（出了 `logos/`、带 `..`、带查询串）的客户端不去下。
    pub fn well_formed(&self) -> bool {
        self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            && ["png", "webp"]
                .iter()
                .any(|extension| self.path == Logo::path_for(&self.sha256, extension))
    }
}

/// 拉黑的一条：编号或者来源仓库，和一句理由。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Blocked {
    /// 拉黑的编号。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// 拉黑的来源仓库。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub origin: Option<String>,
    /// 为什么。
    pub reason: String,
}

/// 读不成的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// 不是这个写法的 JSON。
    Malformed(String),
    /// 版本不是 [`VERSION`]。
    Unsupported(u32),
    /// `generated` 不是 RFC 3339 的时刻。
    BadTime(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Malformed(why) => write!(f, "malformed index: {why}"),
            Error::Unsupported(version) => write!(f, "unsupported index version {version}"),
            Error::BadTime(time) => write!(f, "bad index time: {time}"),
        }
    }
}

impl std::error::Error for Error {}

impl Index {
    /// 照收到的字节读。编号、版本、依赖写法不对的条目丢掉（[`Entry::well_formed`]），别的照读。
    ///
    /// # Errors
    ///
    /// 不是这个写法的 JSON、版本不对、时刻不对。
    pub fn parse(bytes: &[u8]) -> Result<Index, Error> {
        let mut index: Index =
            serde_json::from_slice(bytes).map_err(|error| Error::Malformed(error.to_string()))?;
        if index.version != VERSION {
            return Err(Error::Unsupported(index.version));
        }
        index.time()?;
        index.packages.retain(Entry::well_formed);
        Ok(index)
    }

    /// `generated` 读成时刻。
    ///
    /// # Errors
    ///
    /// 不是 RFC 3339 的时刻。
    pub fn time(&self) -> Result<Timestamp, Error> {
        self.generated
            .parse::<Timestamp>()
            .map_err(|_| Error::BadTime(self.generated.clone()))
    }

    /// 编号是 `id` 的那一个。
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.packages.iter().find(|entry| entry.id == id)
    }

    /// 编号 `id`、来源仓库 `origin` 被拉黑的那一条。
    pub fn blocked(&self, id: &str, origin: &str) -> Option<&Blocked> {
        self.blocked.iter().find(|blocked| {
            blocked.id.as_deref() == Some(id) || blocked.origin.as_deref() == Some(origin)
        })
    }
}

/// 能不能拿 `fresh` 换掉本机的 `kept`：`fresh` 不比 `kept` 旧才行（一样新的照换，同一份重拿不算退回）。
pub fn may_replace(kept: &Index, fresh: &Index) -> bool {
    match (kept.time(), fresh.time()) {
        (Ok(kept), Ok(fresh)) => fresh >= kept,
        (Err(_), Ok(_)) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
