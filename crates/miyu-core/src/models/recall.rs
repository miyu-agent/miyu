//! 目录不常驻，核心这一头（施工 V-25 下，`docs/blueprint/models.md`「怎么走」第二条第 2 条）：记下的那份
//! （`state/models/catalog.json`，`miyu_session::ModelData` 管怎么记、怎么换）认哪一份目录、起来时读不读整份目录、临时怎么读。
//!
//! - 版本串 `of`（[`Version::of`]）：整理好的目录现成的指纹（`super::digest::fingerprint`：哪一份、原文多大、什么时候改的、
//!   `fetched`，程序多大、什么时候改的），加认原厂的表（`models/vendors.toml`）的内容哈希。对不上的整份作废、重算。
//! - 起来时（[`early`]）：先读记下的那份、算排第一那份目录的版本串，对上了就不读整份目录——省内存就省在这里；对不上、没有、
//!   读不懂的读整份目录（照旧的路），由 `super::begin` 把配置里写过的都查一遍记下、写盘，再放掉整份目录。
//! - 没有缓存目录（整理好的那份放不下，临时读一次要现解原文）、不知道程序在哪（指纹算不出来）、没有 `state` 的（测试、
//!   `models::routes()`）不走这条：照旧常驻。

use std::hash::{DefaultHasher, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use miyu_models::catalog::{CatalogSource, Loaded, Recalled};
use miyu_session::{CatalogReader, RECALL_FILE};

use super::catalog::{self, FILE, Places};
use super::digest;
use crate::TARGET;

/// 算版本串要的：程序在哪、认原厂的表的哈希。后台更新拉到新目录时也照它算（[`super::refresh::Refresher`]）。
#[derive(Debug, Clone)]
pub struct Version {
    /// 程序的真实位置。
    pub program: PathBuf,
    /// 认原厂的表的内容哈希。
    pub vendors: String,
}

impl Version {
    /// 照快照所在的目录 `snapshot`（认原厂的表在它旁边）和程序 `program` 造；表读不了、不知道程序在哪的没有（照旧常驻）。
    pub fn of_places(snapshot: &Path, program: Option<&Path>) -> Option<Version> {
        let text = std::fs::read_to_string(snapshot.join("vendors.toml")).ok()?;
        Some(Version {
            program: program?.to_path_buf(),
            vendors: hash(&text),
        })
    }

    /// `source` 那一份目录（原文是 `file`，`meta` 里写的 `fetched`）的版本串；原文、程序的大小和改动时间读不到的没有。
    pub fn of(&self, source: CatalogSource, file: &Path, fetched: &str) -> Option<String> {
        let print = digest::fingerprint(source, file, fetched, Some(&self.program))?;
        Some(format!("{print} vendors={}", self.vendors))
    }
}

/// 认原厂的表的内容哈希：同一个程序算出来的一定一样（换了程序时指纹本来就变了，哈希的算法跟着变也无妨）。
fn hash(text: &str) -> String {
    let mut hasher = DefaultHasher::new();
    hasher.write(text.as_bytes());
    format!("{:016x}", hasher.finish())
}

/// 起来时读到的（[`early`]）。
pub enum Read {
    /// 整份目录（照旧的路）：都读不了的没有；带上它的版本串的，由 `super::begin` 照它记下配置里写过的、放掉整份目录。
    Full(Option<Loaded>, Option<String>),
    /// 记下的那份，版本对上了：没读整份目录。
    Recalled(Recalled),
}

/// 起来时（在读目录的那个线程里）：`version`、`state`（`state/models`）都有、又有缓存目录的，先读记下的那份，版本串和排第一
/// 那份目录的对上了就交回它、不读整份目录，记一行 `INFO catalog recalled`；别的读整份目录。记下的那份读不懂、写法的版本不对
/// 的记一行 `WARN catalog recall unreadable`，当没有。在阻塞线程里调。
pub fn early(places: &Places, version: Option<&Version>, state: Option<&Path>) -> Read {
    let Some((version, state)) = version.zip(state).filter(|_| places.cache.is_some()) else {
        return Read::Full(catalog::load(places), None);
    };
    let started = Instant::now();
    let first = catalog::ranked(places)
        .into_iter()
        .next()
        .and_then(|(source, dir, meta)| {
            let fetched = meta.map(|meta| meta.fetched).unwrap_or_default();
            version.of(source, &dir.join(FILE), &fetched)
        });
    if let (Some(first), Some(recalled)) = (first, read(state))
        && recalled.of() == first
    {
        let (providers, models) = recalled.len();
        tracing::info!(
            target: TARGET,
            source = recalled.source().as_str(),
            fetched = recalled.fetched(),
            providers,
            models,
            ms = started.elapsed().as_millis(),
            "catalog recalled"
        );
        return Read::Recalled(recalled);
    }
    let loaded = catalog::load(places);
    let of = loaded
        .as_ref()
        .and_then(|loaded| of_loaded(places, version, loaded));
    Read::Full(loaded, of)
}

/// 读回来的整份目录的版本串：照它是哪一份、`fetched` 现算。
fn of_loaded(places: &Places, version: &Version, loaded: &Loaded) -> Option<String> {
    let dir = catalog::dir_of(places, loaded.source)?;
    version.of(loaded.source, &dir.join(FILE), &loaded.fetched)
}

/// 读 `state/models/catalog.json`：没有的没有；读不了、读不懂、写法的版本不对的记一行 `WARN`，当没有（整份重算）。
fn read(state: &Path) -> Option<Recalled> {
    let path = state.join(RECALL_FILE);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return None,
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "catalog recall unreadable");
            return None;
        }
    };
    Recalled::from_json(&text)
        .map_err(
            |error| tracing::warn!(target: TARGET, error = %error, "catalog recall unreadable"),
        )
        .ok()
}

/// 临时读整份目录的办法（交给 `ModelData`）：照起来时同样的路挑一份读（读完那一行记 `DEBUG`），连同它的版本串；版本串算不
/// 出来的给空字（和哪一份记下的都对不上，照它重算）。
pub fn reader(places: Places, version: Version) -> CatalogReader {
    Arc::new(move || {
        let loaded = catalog::load_again(&places)?;
        let of = of_loaded(&places, &version, &loaded).unwrap_or_default();
        Some((loaded, of))
    })
}
