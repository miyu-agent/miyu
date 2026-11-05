//! 只算不做的几样：安装标记（`install.toml`）认的是谁装的、版本号怎么比、发布列表里挑哪一个、压缩包叫什么、校验和清单里它的
//! 那一行。

use std::cmp::Ordering;

use serde_json::Value;

/// 谁装的（程序真实位置旁边的 `install.toml` 写的 `channel`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Channel {
    /// 安装脚本装的：`miyu upgrade` 自己下载、核对、换。
    Script,
    /// AUR 的包装的：照包管理器升。
    Aur,
    /// Homebrew 的配方装的：照包管理器升。
    Brew,
}

/// 标记文件的名字：在程序真实位置的旁边。
pub(crate) const MARKER: &str = "install.toml";

/// 照标记文件的内容认谁装的：`channel = "script" | "aur" | "brew"`，`#` 起的是注释。读不懂、没写、写的不认识的交回原因。
pub(crate) fn channel(text: &str) -> Result<Channel, String> {
    let mut found = None;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            return Err(format!("not `key = \"value\"`: {line}"));
        };
        if key.trim() != "channel" {
            continue;
        }
        let value = value.trim();
        let quoted = value.len() >= 2 && value.starts_with('"') && value.ends_with('"');
        if !quoted {
            return Err(format!("channel is not a quoted string: {value}"));
        }
        found = Some(match &value[1..value.len() - 1] {
            "script" => Channel::Script,
            "aur" => Channel::Aur,
            "brew" => Channel::Brew,
            other => return Err(format!("unknown channel `{other}`")),
        });
    }
    found.ok_or_else(|| "no channel".to_string())
}

/// 版本号：`<主>.<次>.<补丁>`，可以接 `-<预发布>`（`1.0.0-rc.2`）。比法照 semver：数字逐段比，带预发布的排在同号正式版前面，
/// 预发布逐段比（都是数字的照数字，不然照字）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Version {
    numbers: [u64; 3],
    pre: Vec<String>,
}

impl Version {
    /// 照写法认（不带开头的 `v`）；不认识的没有。
    pub(crate) fn parse(text: &str) -> Option<Version> {
        let (core, pre) = match text.split_once('-') {
            Some((core, pre)) => (core, Some(pre)),
            None => (text, None),
        };
        let mut parts = core.split('.');
        let mut numbers = [0u64; 3];
        for slot in &mut numbers {
            let part = parts.next()?;
            if part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()) {
                return None;
            }
            *slot = part.parse().ok()?;
        }
        if parts.next().is_some() {
            return None;
        }
        let pre = match pre {
            None => Vec::new(),
            Some(pre) => {
                let fields: Vec<String> = pre.split('.').map(str::to_string).collect();
                let fine = fields.iter().all(|field| {
                    !field.is_empty() && field.bytes().all(|b| b.is_ascii_alphanumeric())
                });
                if !fine {
                    return None;
                }
                fields
            }
        };
        Some(Version { numbers, pre })
    }

    /// 是不是预发布。
    pub(crate) fn is_pre(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl std::fmt::Display for Version {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [major, minor, patch] = self.numbers;
        write!(f, "{major}.{minor}.{patch}")?;
        if self.is_pre() {
            write!(f, "-{}", self.pre.join("."))?;
        }
        Ok(())
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Version) -> Ordering {
        self.numbers.cmp(&other.numbers).then_with(|| {
            match (self.pre.is_empty(), other.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater,
                (false, true) => Ordering::Less,
                (false, false) => pre_order(&self.pre, &other.pre),
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Version) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 预发布逐段比：都是数字的照数字（`rc.10` 在 `rc.9` 后面），数字排在字前面，别的照字；前面都一样的，段少的在前。
fn pre_order(left: &[String], right: &[String]) -> Ordering {
    for (a, b) in left.iter().zip(right) {
        let numeric = |text: &str| text.bytes().all(|b| b.is_ascii_digit());
        let order = match (numeric(a), numeric(b)) {
            (true, true) => a
                .parse::<u64>()
                .unwrap_or(u64::MAX)
                .cmp(&b.parse::<u64>().unwrap_or(u64::MAX)),
            (true, false) => Ordering::Less,
            (false, true) => Ordering::Greater,
            (false, false) => a.cmp(b),
        };
        if order != Ordering::Equal {
            return order;
        }
    }
    left.len().cmp(&right.len())
}

/// 发布里的一个文件。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Asset {
    /// 文件名。
    pub(crate) name: String,
    /// 下载地址。
    pub(crate) url: String,
}

/// 挑中的一个发布。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Release {
    /// 照标签认的版本。
    pub(crate) version: Version,
    /// 带的文件。
    pub(crate) assets: Vec<Asset>,
}

impl Release {
    /// 叫 `name` 的那个文件。
    pub(crate) fn asset(&self, name: &str) -> Option<&Asset> {
        self.assets.iter().find(|asset| asset.name == name)
    }
}

/// GitHub 发布列表（`/repos/<仓库>/releases`）里最新的一个：草稿不算；预发布只在 `pre` 时算；标签要是 `v<版本>`，认不出的跳过。
pub(crate) fn newest(list: &Value, pre: bool) -> Option<Release> {
    list.as_array()?
        .iter()
        .filter(|release| release["draft"].as_bool() != Some(true))
        .filter(|release| pre || release["prerelease"].as_bool() != Some(true))
        .filter_map(|release| {
            let version = Version::parse(release["tag_name"].as_str()?.strip_prefix('v')?)?;
            if version.is_pre() && !pre {
                return None;
            }
            let assets = release["assets"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|asset| {
                    Some(Asset {
                        name: asset["name"].as_str()?.to_string(),
                        url: asset["browser_download_url"].as_str()?.to_string(),
                    })
                })
                .collect();
            Some(Release { version, assets })
        })
        .max_by(|a, b| a.version.cmp(&b.version))
}

/// 列表里有没有预发布（草稿不算，标签要是 `v<版本>`；标成预发布的、标签带 `-` 的都算）：没有正式版时照它提示加 `--prerelease`
/// （施工 RL-4补2）。
pub(crate) fn has_pre(list: &Value) -> bool {
    list.as_array().into_iter().flatten().any(|release| {
        let version = release["tag_name"]
            .as_str()
            .and_then(|tag| tag.strip_prefix('v'))
            .and_then(Version::parse);
        release["draft"].as_bool() != Some(true)
            && version.is_some_and(|version| {
                version.is_pre() || release["prerelease"].as_bool() == Some(true)
            })
    })
}

/// 这一版、这一种系统的压缩包名（同 `cargo xtask dist`）：Windows 是 `.zip`，别的 `.tar.gz`。
pub(crate) fn archive_name(version: &Version, target: &str) -> String {
    let suffix = if target.starts_with("windows") {
        "zip"
    } else {
        "tar.gz"
    };
    format!("miyu-{version}-{target}.{suffix}")
}

/// 校验和清单里 `name` 那一行的 SHA-256（小写十六进制）；没有这一行的没有。清单先照 `miyu_source::sums::check` 查过。
pub(crate) fn hash_of(sums: &str, name: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once("  ")?;
        (file == name).then(|| hash.to_string())
    })
}

#[cfg(test)]
mod tests;
