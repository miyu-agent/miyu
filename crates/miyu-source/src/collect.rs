//! 收录（施工 G-2 下，设计 `37-软件源.md` 第三节第 4、5 条）：照 [`Rules`] 列出仓库，读每个仓库最新的 Release 里的 `.miyu`，
//! 下载、算哈希、解开照 Miyu 自己那一套读清单，写成索引的一条。编号先到先得（`names.toml`），拉黑的去掉，官方的标上。
//!
//! - 一个仓库最新的 Release 里可以有几个包（施工 G-16：扩展和它带的插件住一个仓库）；照编号分开，每个编号各自收成一条、各自
//!   先到先得。同一个编号的几个文件要是同一个版本：不分系统的一个文件，或者一种系统一个文件；文件名要和清单对得上
//!   （[`archive::file_name`]）。对不上的、读不成的整个仓库这一次不收，记一笔。
//! - 网络走 [`Fetch`]：小程序接 HTTP，测试接内存里的一张表。
//! - 包自带的图标（施工 F-18 下，`[package] logo`）从包文件里取出来，放进 [`Collected::logos`]（照 [`Logo::path_for`]），和索引
//!   一起发；条目记位置、哈希、大小。在包目录外、不是 PNG / WebP、超过 [`LOGO_MAX_BYTES`] 的不收图标、记一笔，包照收。
//! - 社区那种来源（`kind = "topic"`，施工 CC-1，第三节第 5 条）：照 GitHub 的搜索列出打了话题的仓库（`topic` 模块管跳过谁），
//!   和官方的走同一条路收，`official` 照在不在 `official.toml`。官方来源已经列了的不再列一遍：官方那一条为准。不晾，
//!   打了话题的最新 Release 下一趟就收；作恶的事后写进 `blocked.toml`（2026-11-02 项目主人定）。
//! - 来源仓库被拉黑的（`blocked.toml` 写了 `origin`）连 Release 都不问：社区仓库能把包文件做得很大，拉黑了就别再下。

mod folder;
pub mod rules;
mod tally;
mod topic;

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use miyu_config::package::{self, LOGO_MAX_BYTES, Manifest, Target};

use crate::archive;
use crate::index::{Entry, File, Index, Logo, VERSION};
pub use folder::from_folder;
pub use rules::{Rules, Source};
use tally::Tally;

/// 一个包文件最多多大。
pub const MOST_FILE: u64 = 2 << 30;

/// Release 的说明最多多大。
const MOST_RELEASE: u64 = 4 << 20;

/// 拿一个地址的全部字节，最多 `limit` 字节。
pub trait Fetch {
    /// 拿 `url`。
    ///
    /// # Errors
    ///
    /// 拿不到、超过 `limit`：英文一句。
    fn get(&self, url: &str, limit: u64) -> Result<Vec<u8>, String>;
}

/// 收完的：索引（没签名）、更新过的 `names.toml`、这一趟的记录（给 Actions 的摘要）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Collected {
    /// 索引。
    pub index: Index,
    /// 编号到来源仓库，这一趟新占的加上了。
    pub names: BTreeMap<String, String>,
    /// 一行一件事：收了的、跳过的和为什么。
    pub notes: Vec<String>,
    /// 收了的包自带的图标（施工 F-18 下）：相对输出目录的位置（同条目里 `logo.path`）到字节。
    pub logos: BTreeMap<String, Vec<u8>>,
}

/// 一个仓库里的一个包收成的：一条、它的图标、要记的一笔。
struct Gathered {
    entry: Entry,
    logo: Option<Vec<u8>>,
    note: Option<String>,
}

/// GitHub 的 Release 说明里用得上的。
#[derive(Debug, Deserialize)]
struct Release {
    #[serde(default)]
    assets: Vec<Asset>,
}

#[derive(Debug, Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
}

/// 照 `rules` 收一趟。`api` 是 GitHub API 的地址（`https://api.github.com`），`scratch` 是解包用的空目录，`generated`
/// 写进索引的时刻。
pub fn collect(
    rules: &Rules,
    fetch: &dyn Fetch,
    api: &str,
    scratch: &Path,
    generated: &str,
) -> Collected {
    let mut notes = Vec::new();
    let mut repos: Vec<String> = Vec::new();
    for source in &rules.sources {
        match source {
            Source::Official => {
                for repo in &rules.official {
                    if !has_repo(&repos, repo) {
                        repos.push(repo.clone());
                    }
                }
            }
            Source::Topic(topic) => {
                for repo in topic::search(topic, fetch, api, &mut notes) {
                    if has_repo(&repos, &repo) {
                        notes.push(format!("{repo}: skipped in topic {topic}, already listed"));
                    } else {
                        repos.push(repo);
                    }
                }
            }
        }
    }
    let mut names = rules.names.clone();
    let mut packages: Vec<Entry> = Vec::new();
    let mut logos = BTreeMap::new();
    for (n, repo) in repos.iter().enumerate() {
        let origin = rules::origin(repo);
        if let Some(blocked) = rules
            .blocked
            .iter()
            .find(|blocked| blocked.origin.as_deref() == Some(origin.as_str()))
        {
            notes.push(format!("{repo}: blocked, {}", blocked.reason));
            continue;
        }
        let official = has_repo(&rules.official, repo);
        let place = scratch.join(n.to_string());
        let gathered = one_repo(repo, &origin, official, fetch, api, &place);
        discard(&place);
        let gathered = match gathered {
            Ok(all) => all,
            Err(why) => {
                notes.push(format!("{repo}: skipped, {why}"));
                continue;
            }
        };
        for one in gathered {
            let mut tally = Tally {
                names: &mut names,
                notes: &mut notes,
                logos: &mut logos,
                packages: &mut packages,
            };
            tally.take(one, repo, &origin, rules);
        }
    }
    packages.sort_by(|a, b| a.id.cmp(&b.id));
    Collected {
        index: Index {
            version: VERSION,
            generated: generated.to_string(),
            packages,
            blocked: rules.blocked.clone(),
        },
        names,
        notes,
        logos,
    }
}

/// `repo` 在不在 `repos` 里：GitHub 的 `owner/repo` 不分大小写，搜索交回的写法可能和 `official.toml` 的不一样。
fn has_repo(repos: &[String], repo: &str) -> bool {
    repos.iter().any(|listed| listed.eq_ignore_ascii_case(repo))
}

/// 一个仓库最新的 Release 收成的几个包，照编号第一次出现的先后。
fn one_repo(
    repo: &str,
    origin: &str,
    official: bool,
    fetch: &dyn Fetch,
    api: &str,
    place: &Path,
) -> Result<Vec<Gathered>, String> {
    let url = format!("{}/repos/{repo}/releases/latest", api.trim_end_matches('/'));
    let release: Release = serde_json::from_slice(&fetch.get(&url, MOST_RELEASE)?)
        .map_err(|error| format!("unreadable release: {error}"))?;
    let assets: Vec<&Asset> = release
        .assets
        .iter()
        .filter(|asset| asset.name.ends_with(&format!(".{}", archive::EXTENSION)))
        .collect();
    let mut gathered: Vec<(Gathered, Vec<File>)> = Vec::new();
    for (n, asset) in assets.iter().enumerate() {
        let bytes = fetch.get(&asset.browser_download_url, MOST_FILE)?;
        let read = read_package(
            &asset.name,
            &asset.browser_download_url,
            &bytes,
            &place.join(n.to_string()),
            (origin, official),
        )?;
        let Read {
            gathered: one,
            file,
        } = read;
        match gathered
            .iter_mut()
            .find(|(first, _)| first.entry.id == one.entry.id)
        {
            None => gathered.push((one, vec![file])),
            Some((first, _)) if first.entry.version != one.entry.version => {
                return Err(format!(
                    "files of {} in different versions in one release",
                    one.entry.id
                ));
            }
            Some((first, files)) => {
                first.entry.installed = first.entry.installed.max(one.entry.installed);
                files.push(file);
            }
        }
    }
    if gathered.is_empty() {
        return Err("no package file in the latest release".to_string());
    }
    Ok(gathered
        .into_iter()
        .map(|(mut one, mut files)| {
            files.sort_by(|a, b| a.target.cmp(&b.target));
            one.entry.files = files;
            one
        })
        .collect())
}

/// 一个包文件读成的：这一条（还没有文件）、它的图标、要记的一笔，和它作为一个文件的那一格。
struct Read {
    gathered: Gathered,
    file: File,
}

/// 读一个包文件（名字 `name`，下载地址 `url`，字节 `bytes`）：在 `dir` 里解开，照 Miyu 自己那一套读清单，写成一条和一格文件。
/// `source` 是来源（`origin`）和是不是官方的。文件名要和清单对得上，读不成的交回为什么（英文）。
fn read_package(
    name: &str,
    url: &str,
    bytes: &[u8],
    dir: &Path,
    (origin, official): (&str, bool),
) -> Result<Read, String> {
    std::fs::create_dir_all(dir).map_err(|error| error.to_string())?;
    let file = dir.join("package.miyu");
    std::fs::write(&file, bytes).map_err(|error| error.to_string())?;
    let unpacked = archive::unpack(&file, &dir.join("unpacked"))
        .map_err(|error| format!("{name}: {error}"))?;
    // 编号是数据根里保留的名字的不收（施工 FH-5下，设计 55 第三节第 3 条）：装不上的包不进索引，也不占编号。
    if let Some(problem) = package::reserved_id(&unpacked.id) {
        return Err(format!("{name}: {}", problem.message));
    }
    let text = std::fs::read_to_string(unpacked.folder.join("package.toml"))
        .map_err(|_| format!("{name}: no package.toml"))?;
    let manifest = package::read(&text).map_err(|problem| format!("{name}: {problem}"))?;
    let version = manifest
        .version
        .clone()
        .ok_or_else(|| format!("{name}: no version"))?;
    let target = target_of(name, &unpacked.id, &version, &manifest.targets)?;
    let mut entry = entry_of(
        &unpacked.id,
        &version,
        &manifest,
        &unpacked.folder,
        origin,
        official,
    );
    entry.installed = Some(unpacked.bytes);
    let (mut logo, mut note) = (None, None);
    match logo_of(&manifest, &unpacked.folder) {
        Ok(Some((at, bytes))) => {
            entry.logo = Some(at);
            logo = Some(bytes);
        }
        Ok(None) => {}
        Err(why) => note = Some(format!("logo left out, {why}")),
    }
    Ok(Read {
        gathered: Gathered { entry, logo, note },
        file: File {
            target,
            url: url.to_string(),
            sha256: hex(&Sha256::digest(bytes)),
            size: bytes.len() as u64,
        },
    })
}

/// 包自带的图标：清单写了的照包目录里的真实位置读，认 PNG、WebP 开头的字节，不超过 [`LOGO_MAX_BYTES`]。没写的没有；不合的
/// 交回为什么（英文）。边长不在这里量：打包时已经缩过，客户端读的时候再查。
fn logo_of(manifest: &Manifest, folder: &Path) -> Result<Option<(Logo, Vec<u8>)>, String> {
    let Some(path) = manifest.logo.as_deref() else {
        return Ok(None);
    };
    let root = std::fs::canonicalize(folder).map_err(|error| error.to_string())?;
    let real = std::fs::canonicalize(folder.join(path))
        .ok()
        .filter(|real| real.starts_with(&root) && real.is_file())
        .ok_or_else(|| format!("{path} is not in the package"))?;
    let size = std::fs::metadata(&real)
        .map_err(|error| error.to_string())?
        .len();
    if size > LOGO_MAX_BYTES {
        return Err(format!("{path} is over {LOGO_MAX_BYTES} bytes"));
    }
    let bytes = std::fs::read(&real).map_err(|error| error.to_string())?;
    let extension = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        "png"
    } else if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        "webp"
    } else {
        return Err(format!("{path} is not a PNG or WebP"));
    };
    let sha256 = hex(&Sha256::digest(&bytes));
    let logo = Logo {
        path: Logo::path_for(&sha256, extension),
        sha256,
        size: bytes.len() as u64,
    };
    Ok(Some((logo, bytes)))
}

/// 文件名照清单算是给哪一种系统的；对不上的不收。
fn target_of(
    name: &str,
    id: &str,
    version: &str,
    targets: &[Target],
) -> Result<Option<String>, String> {
    if targets.is_empty() {
        return (name == archive::file_name(id, version, None))
            .then_some(None)
            .ok_or_else(|| format!("{name} should be {}", archive::file_name(id, version, None)));
    }
    targets
        .iter()
        .find(|target| name == archive::file_name(id, version, Some(**target)))
        .map(|target| Some(target.as_str().to_string()))
        .ok_or_else(|| format!("{name} does not match {id} {version} for any of its targets"))
}

/// 照清单和解开的文件夹写一条（还没有文件）。
fn entry_of(
    id: &str,
    version: &str,
    manifest: &Manifest,
    folder: &Path,
    origin: &str,
    official: bool,
) -> Entry {
    Entry {
        id: id.to_string(),
        version: version.to_string(),
        name: manifest.name.clone(),
        summary: manifest.summary.clone(),
        origin: origin.to_string(),
        official,
        kind: Some(manifest.kind.as_str().to_string()),
        icon: manifest.icon.clone(),
        logo: None,
        provides: crate::provides::of(manifest, folder),
        capabilities: manifest
            .process
            .iter()
            .flat_map(|process| process.capabilities.iter().map(|c| c.as_str().to_string()))
            .collect(),
        depends: manifest.depends.iter().map(ToString::to_string).collect(),
        recommends: manifest
            .recommends
            .iter()
            .map(ToString::to_string)
            .collect(),
        author: manifest.author.clone(),
        upstream: manifest.upstream.clone(),
        license: manifest.license.clone(),
        installed: None,
        // 上线的时刻、下载次数（施工 DL-1）：收录时还没有，生成索引的那一步照上一份、GitHub 填（施工 DL-2）。
        published: None,
        downloads: None,
        downloads_week: None,
        tools: crate::tools::of(manifest, folder),
        files: Vec::new(),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// 删掉解包用的目录；删不掉的留着。
#[expect(
    clippy::let_underscore_must_use,
    reason = "解包用的临时目录删不掉就留着，收录照样往下走"
)]
fn discard(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir);
}

#[cfg(test)]
mod tests;
