//! 导入原始包（施工 SE-1 下，设计 `58-会话导出和导入.md` 第二节）：先核对，再落盘；有一样不对整个拒，数据根里什么都不留
//! （blob 例外：先放进 blob 库的，没人引用、照 blob 的回收办）。
//!
//! 1. 清单：认得的格式版本（比 [`FORMAT`] 新的拒）；每一段、每一个 blob 在包里、字节数和 SHA-256 对得上。
//! 2. 每个会话的段：段名是 12 位数字加 `.jsonl`、照名字排；每一行是一整行 JSON、序号从段名那一号起一条接一条；每一行里写成
//!    内容哈希的字，要么在包里的 blob 里，要么在清单的 `absent` 里。
//! 3. 段先写进会话目录旁边的暂存目录（名字点开头，不算会话），全部核对完再一个个改名进位置，各写一份 [`MARKER`]。
//! 4. 这个数据根里已经有同编号的会话：包里那几段是它日志的开头（一个字节不差）的，当已经导过、跳过；不是的整个拒
//!    （[`ImportError::Exists`]）。

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use zip::ZipArchive;

use miyu_kernel::id::{ContentHash, Hasher, SessionId};

use super::{FORMAT, MANIFEST, Manifest, ManifestSession, hashes_in, segment_names};
use crate::blob::Blobs;
use crate::durable::sync_dir;

/// 导进来的会话目录里放的标记：从哪个包来、是不是存档。日志一个字节不改：人格、场所照日志里写的。
pub const MARKER: &str = "imported.json";

/// 清单最多多大。
const MOST_MANIFEST: u64 = 16 << 20;

/// 一段最多多大：段的上限的两倍（`log::SEGMENT_LIMIT` 是写满了才开下一段，最后一批可能超一点）。
const MOST_SEGMENT: u64 = 2 * crate::log::SEGMENT_LIMIT;

/// [`MARKER`] 的内容。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Marker {
    /// 只读存档：能翻、能搜，不能接着说。
    pub archive: bool,
    /// 导它的核心的版本（清单的 `miyu`）。
    pub miyu: String,
    /// 导出的时刻（清单的 `exported`）。
    pub exported: String,
}

/// 导进来的。
#[derive(Debug, Clone)]
pub struct Imported {
    /// 清单。
    pub manifest: Manifest,
    /// 这一次新落盘的会话。
    pub added: Vec<SessionId>,
    /// 早就在的（导过了），跳过的。
    pub present: Vec<SessionId>,
}

/// 导入失败。
#[derive(Debug)]
pub enum ImportError {
    /// 不是认得的包：不是 ZIP、没有清单、清单读不懂、段名不对。
    Bad(String),
    /// 包里的东西和清单对不上：缺了、字节数或哈希不对、序号不连、引用了包里没有的 blob。
    Mismatch(String),
    /// 清单的格式版本比这个核心新。
    TooNew(u32),
    /// 这个数据根里已经有同编号、内容不一样的会话。
    Exists(SessionId),
    /// 读写出错。
    Io(io::Error),
}

impl From<io::Error> for ImportError {
    fn from(error: io::Error) -> ImportError {
        ImportError::Io(error)
    }
}

/// 读包 `bundle`，把会话导进 `sessions`（数据根的 `root/sessions/`），blob 放进 `blobs`，每个导进来的会话放一份
/// `marker` 的内容（`miyu`、`exported` 照清单填）。见模块的说明。
///
/// # Errors
///
/// [`ImportError`]。失败时 `sessions` 里不多出任何会话。
pub fn import(
    bundle: &Path,
    sessions: &Path,
    blobs: &Blobs,
    marker: &Marker,
) -> Result<Imported, ImportError> {
    let mut archive = ZipArchive::new(File::open(bundle)?)
        .map_err(|error| ImportError::Bad(format!("not a zip: {error}")))?;
    let manifest = read_manifest(&mut archive)?;
    if manifest.format > FORMAT {
        return Err(ImportError::TooNew(manifest.format));
    }
    fs::create_dir_all(sessions)?;
    let staging = sessions.join(staging_name());
    fs::create_dir(&staging)?;
    let done = stage_and_place(&mut archive, &manifest, sessions, &staging, blobs, marker);
    // 暂存目录收不掉的（Windows 上还开着）不碍事：点开头，列会话时不认。
    drop(fs::remove_dir_all(&staging));
    done.map(|(added, present)| Imported {
        manifest,
        added,
        present,
    })
}

/// 核对、暂存、放 blob，最后改名进位置。
fn stage_and_place(
    archive: &mut ZipArchive<File>,
    manifest: &Manifest,
    sessions: &Path,
    staging: &Path,
    blobs: &Blobs,
    marker: &Marker,
) -> Result<(Vec<SessionId>, Vec<SessionId>), ImportError> {
    let mut referenced = BTreeSet::new();
    for session in &manifest.sessions {
        stage_session(
            archive,
            session,
            &staging.join(session.id.as_str()),
            &mut referenced,
        )?;
    }
    let shipped: BTreeSet<&str> = manifest
        .blobs
        .iter()
        .map(|blob| blob.hash.as_str())
        .chain(manifest.absent.iter().map(ContentHash::as_str))
        .collect();
    if let Some(missing) = referenced
        .iter()
        .find(|hash| !shipped.contains(hash.as_str()))
    {
        return Err(ImportError::Mismatch(format!(
            "{missing} is referenced but not in the bundle"
        )));
    }
    let mut present = Vec::new();
    for session in &manifest.sessions {
        let at = sessions.join(session.id.as_str());
        if at.symlink_metadata().is_ok() {
            if !prefix_of(&staging.join(session.id.as_str()), &at, session)? {
                return Err(ImportError::Exists(session.id.clone()));
            }
            present.push(session.id.clone());
        }
    }
    for blob in &manifest.blobs {
        put_blob(archive, blob, staging, blobs)?;
    }
    let mut added = Vec::new();
    let mut marker = marker.clone();
    marker.miyu.clone_from(&manifest.miyu);
    marker.exported.clone_from(&manifest.exported);
    for session in &manifest.sessions {
        if present.contains(&session.id) {
            continue;
        }
        let staged = staging.join(session.id.as_str());
        let mut one = marker.clone();
        one.archive |= session.archived;
        let text = serde_json::to_vec_pretty(&one).map_err(io::Error::other)?;
        // 写完先关上再改名：Windows 上目录里有开着的文件就挪不动（Access is denied）。
        {
            let mut file = File::create(staged.join(MARKER))?;
            file.write_all(&text)?;
            file.sync_all()?;
        }
        sync_dir(&staged)?;
        fs::rename(&staged, sessions.join(session.id.as_str()))?;
        added.push(session.id.clone());
    }
    sync_dir(sessions)?;
    Ok((added, present))
}

/// 读清单。
fn read_manifest(archive: &mut ZipArchive<File>) -> Result<Manifest, ImportError> {
    let entry = archive
        .by_name(MANIFEST)
        .map_err(|_| ImportError::Bad("no manifest.json".to_string()))?;
    let mut bytes = Vec::new();
    entry.take(MOST_MANIFEST + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MOST_MANIFEST {
        return Err(ImportError::Bad("manifest.json is too big".to_string()));
    }
    serde_json::from_slice(&bytes)
        .map_err(|error| ImportError::Bad(format!("manifest.json: {error}")))
}

/// 一个会话的各段：核对后写进 `dir`；收下每一行引用的内容哈希。
fn stage_session(
    archive: &mut ZipArchive<File>,
    session: &ManifestSession,
    dir: &Path,
    referenced: &mut BTreeSet<String>,
) -> Result<(), ImportError> {
    fs::create_dir(dir)?;
    let mut next: Option<u64> = None;
    for segment in &session.segments {
        let first = segment
            .name
            .strip_suffix(".jsonl")
            .filter(|stem| stem.len() == 12 && stem.bytes().all(|b| b.is_ascii_digit()))
            .and_then(|stem| stem.parse::<u64>().ok())
            .filter(|first| *first > 0)
            .ok_or_else(|| ImportError::Bad(format!("bad segment name {}", segment.name)))?;
        if next.is_some_and(|next| next != first) {
            return Err(ImportError::Mismatch(format!(
                "{}: segments are not contiguous",
                session.id
            )));
        }
        let name = format!("sessions/{}/{}", session.id, segment.name);
        let bytes = read_entry(archive, &name, segment.bytes, MOST_SEGMENT)?;
        if ContentHash::of(&bytes) != segment.sha256 {
            return Err(ImportError::Mismatch(format!(
                "{name}: sha256 does not match"
            )));
        }
        let last = check_lines(&bytes, first, &name, referenced)?;
        next = Some(last + 1);
        let mut file = File::create(dir.join(&segment.name))?;
        file.write_all(&bytes)?;
        file.sync_all()?;
    }
    if session.upto.is_some_and(|upto| next != Some(upto + 1)) {
        return Err(ImportError::Mismatch(format!(
            "{}: upto does not match",
            session.id
        )));
    }
    sync_dir(dir)?;
    Ok(())
}

/// 一段的每一行：整行、JSON、序号从 `first` 起一条接一条；交回最后一条的序号。
fn check_lines(
    bytes: &[u8],
    first: u64,
    name: &str,
    referenced: &mut BTreeSet<String>,
) -> Result<u64, ImportError> {
    if bytes.last() != Some(&b'\n') {
        return Err(ImportError::Mismatch(format!(
            "{name}: does not end with a whole line"
        )));
    }
    let mut expected = first;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        let value: Value = serde_json::from_slice(line)
            .map_err(|error| ImportError::Mismatch(format!("{name}: line {expected}: {error}")))?;
        if value.get("seq").and_then(Value::as_u64) != Some(expected) {
            return Err(ImportError::Mismatch(format!(
                "{name}: expected seq {expected}"
            )));
        }
        hashes_in(&value, referenced);
        expected += 1;
    }
    Ok(expected - 1)
}

/// 包里一项的全部字节：要和清单说的一样长、不超过 `most`。
fn read_entry(
    archive: &mut ZipArchive<File>,
    name: &str,
    bytes: u64,
    most: u64,
) -> Result<Vec<u8>, ImportError> {
    if bytes > most {
        return Err(ImportError::Bad(format!("{name} is too big")));
    }
    let entry = archive
        .by_name(name)
        .map_err(|_| ImportError::Mismatch(format!("{name} is missing")))?;
    let mut read = Vec::with_capacity(usize::try_from(bytes).unwrap_or(0));
    entry.take(bytes + 1).read_to_end(&mut read)?;
    if read.len() as u64 != bytes {
        return Err(ImportError::Mismatch(format!(
            "{name}: size does not match"
        )));
    }
    Ok(read)
}

/// 一个 blob：解到暂存目录里的一个文件，边读边算哈希，再交给 blob 库存；对不上的拒。
fn put_blob(
    archive: &mut ZipArchive<File>,
    blob: &super::ManifestBlob,
    staging: &Path,
    blobs: &Blobs,
) -> Result<(), ImportError> {
    let name = format!("blobs/{}", blob.hash.hex());
    let mut entry = archive
        .by_name(&name)
        .map_err(|_| ImportError::Mismatch(format!("{name} is missing")))?;
    let path: PathBuf = staging.join("blob");
    let mut file = File::create(&path)?;
    let (mut hasher, mut copied, mut buffer) = (Hasher::default(), 0u64, vec![0u8; 64 * 1024]);
    loop {
        let n = entry.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        copied += n as u64;
        if copied > blob.bytes {
            return Err(ImportError::Mismatch(format!(
                "{name}: size does not match"
            )));
        }
        hasher.update(&buffer[..n]);
        file.write_all(&buffer[..n])?;
    }
    drop(file);
    if copied != blob.bytes || hasher.finish() != blob.hash {
        return Err(ImportError::Mismatch(format!(
            "{name}: does not match its hash"
        )));
    }
    let stored = blobs.put_file(&path, |_| {})?;
    fs::remove_file(&path)?;
    if stored != blob.hash {
        return Err(ImportError::Mismatch(format!(
            "{name}: does not match its hash"
        )));
    }
    Ok(())
}

/// 暂存的这个会话（`staged`）是不是已经在的那一个（`at`）的开头：每一段一个字节不差（最后一段可以是已经在的那一段的开头）。
fn prefix_of(staged: &Path, at: &Path, session: &ManifestSession) -> io::Result<bool> {
    let existing = segment_names(at)?;
    for (n, segment) in session.segments.iter().enumerate() {
        if existing.get(n) != Some(&segment.name) {
            return Ok(false);
        }
        let ours = fs::read(staged.join(&segment.name))?;
        let theirs = fs::read(at.join(&segment.name))?;
        let last = n + 1 == session.segments.len();
        let same = if last {
            theirs.starts_with(&ours)
        } else {
            theirs == ours
        };
        if !same {
            return Ok(false);
        }
    }
    Ok(true)
}

/// 暂存目录的名字：点开头（不是会话编号，列会话时不认），带进程号和计数。
fn staging_name() -> String {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    format!(
        ".import-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}
