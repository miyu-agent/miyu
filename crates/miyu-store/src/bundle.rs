//! 会话的原始包（施工 SE-1 上，设计 `58-会话导出和导入.md` 第一节、`07-存储.md` 第三节）：几个会话的日志各段原文、它们引用
//! 到的 blob、一份清单，打成一个 ZIP。存档、查问题、导入（SE-1 下）都认它。
//!
//! - 日志一个字节不改：每段只取到最后一个换行（后面没写完的半行不要）；写了 `upto` 的（分叉来源）取到那一条为止。在跑的会话
//!   照这一刻已经落盘的整行取，就是一致的一份：日志只追加。
//! - blob：每一行照 JSON 读一遍，凡是写成内容哈希的字（`sha256:` 加 64 位小写十六进制）都算引用。blob 库里有的放进包，
//!   边拷边重新算哈希，对不上整个失败（照 dsh 的 fail-loud）；库里没有的（有的哈希本来就不是 blob，例如请求字节的哈希）
//!   记进清单的 `absent`，不算错。
//! - 先写进同目录的临时文件，写完再挂到 `out`：`out` 已经有了的不覆盖（[`BundleError::Exists`]），写到一半失败的删掉临时文件。

mod import;
mod manifest;

use std::collections::BTreeSet;
use std::fs::{self, File};
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde_json::Value;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use miyu_kernel::id::{ContentHash, Hasher, Seq, SessionId};

use crate::blob::Blobs;
use crate::durable::{create_temp, discard, temp_name};
pub use import::{ImportError, Imported, MARKER, Marker, import};
pub use manifest::{FORMAT, Manifest, ManifestBlob, ManifestSegment, ManifestSession};

/// 包里放清单的那一项。
pub const MANIFEST: &str = "manifest.json";

/// 包里的一个会话：编号、会话目录、取到第几条（分叉来源写分叉点，别的不写）。
#[derive(Debug, Clone)]
pub struct Part {
    /// 会话编号。
    pub session: SessionId,
    /// 会话目录 `root/sessions/<编号>/`。
    pub dir: PathBuf,
    /// 只取到这一条为止；不写的取到最后一条落了盘的。
    pub upto: Option<Seq>,
}

/// 清单里除了会话、blob 以外的几格。
#[derive(Debug, Clone)]
pub struct About {
    /// 主会话：导的是哪一个。
    pub main: SessionId,
    /// 导它的核心的版本。
    pub miyu: String,
    /// 导出的时刻，RFC 3339。
    pub exported: String,
}

/// 写出来的包。
#[derive(Debug, Clone)]
pub struct Written {
    /// 包有多少字节。
    pub bytes: u64,
    /// 清单。
    pub manifest: Manifest,
}

/// 打包失败。
#[derive(Debug)]
pub enum BundleError {
    /// `out` 已经有了：不覆盖。
    Exists,
    /// 一个 blob 的内容和它的名字对不上：写明是哪一个。
    BlobMismatch(ContentHash),
    /// 读日志、读 blob、写包时出错。
    Io(io::Error),
}

impl From<io::Error> for BundleError {
    fn from(error: io::Error) -> BundleError {
        BundleError::Io(error)
    }
}

impl From<zip::result::ZipError> for BundleError {
    fn from(error: zip::result::ZipError) -> BundleError {
        BundleError::Io(io::Error::other(error))
    }
}

/// 把 `parts` 打成一个包写到 `out`，blob 从 `blobs` 取。见模块的说明。
///
/// # Errors
///
/// [`BundleError`]：`out` 已经有了、blob 对不上、读写出错。失败时 `out` 不会出现，临时文件删掉。
pub fn write(
    parts: &[Part],
    blobs: &Blobs,
    about: &About,
    out: &Path,
) -> Result<Written, BundleError> {
    if out.symlink_metadata().is_ok() {
        return Err(BundleError::Exists);
    }
    let dir = out
        .parent()
        .filter(|dir| !dir.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = out
        .file_name()
        .map_or_else(|| "bundle".into(), |name| name.to_string_lossy());
    let (temp, file) = create_temp(dir, || temp_name(&name))?;
    let written = fill(parts, blobs, about, file).and_then(|manifest| {
        let bytes = fs::metadata(&temp)?.len();
        publish(&temp, out)?;
        Ok(Written { bytes, manifest })
    });
    if written.is_err() {
        discard(&temp);
    }
    written
}

/// 往开着的临时文件里写整个包，交回清单。
fn fill(parts: &[Part], blobs: &Blobs, about: &About, file: File) -> Result<Manifest, BundleError> {
    let mut zip = ZipWriter::new(BufWriter::new(file));
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .large_file(true);
    let mut referenced = BTreeSet::new();
    let mut sessions = Vec::with_capacity(parts.len());
    for part in parts {
        sessions.push(one_session(&mut zip, options, part, &mut referenced)?);
    }
    let (kept, absent) = copy_blobs(&mut zip, options, blobs, &referenced)?;
    let manifest = Manifest {
        format: FORMAT,
        miyu: about.miyu.clone(),
        exported: about.exported.clone(),
        main: about.main.clone(),
        sessions,
        blobs: kept,
        absent,
    };
    zip.start_file(MANIFEST, options)?;
    let mut text = serde_json::to_vec_pretty(&manifest).map_err(io::Error::other)?;
    text.push(b'\n');
    zip.write_all(&text)?;
    let mut inner = zip.finish()?;
    inner.flush()?;
    inner.get_ref().sync_all()?;
    Ok(manifest)
}

/// 一个会话的各段：照名字（第一条的序号）从小到大，每段取到最后一个换行、写了 `upto` 的取到那一条，放进
/// `sessions/<编号>/<段名>`；顺手收下每一行引用到的内容哈希。
fn one_session<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    options: SimpleFileOptions,
    part: &Part,
    referenced: &mut BTreeSet<String>,
) -> Result<ManifestSession, BundleError> {
    let mut segments = Vec::new();
    let mut last: Option<u64> = None;
    for name in segment_names(&part.dir)? {
        let bytes = fs::read(part.dir.join(&name))?;
        let (kept, seen, stop) = whole_lines(&bytes, part.upto, referenced);
        if kept.is_empty() {
            if stop {
                break;
            }
            continue;
        }
        last = seen.or(last);
        zip.start_file(format!("sessions/{}/{name}", part.session), options)?;
        zip.write_all(kept)?;
        segments.push(ManifestSegment {
            name,
            bytes: kept.len() as u64,
            sha256: ContentHash::of(kept),
        });
        if stop {
            break;
        }
    }
    Ok(ManifestSession {
        id: part.session.clone(),
        upto: last,
        segments,
        archived: archived(&part.dir),
    })
}

/// 一段里要的那一截：到最后一个换行为止的整行，写了 `upto` 的到序号不大于它的最后一行为止。交回那一截、最后一行的序号、
/// 是不是碰到了 `upto` 以后的（后面的段不用看了）；每一行引用到的内容哈希收进 `referenced`。
fn whole_lines<'a>(
    bytes: &'a [u8],
    upto: Option<Seq>,
    referenced: &mut BTreeSet<String>,
) -> (&'a [u8], Option<u64>, bool) {
    let mut end = 0;
    let mut last = None;
    for line in bytes.split_inclusive(|byte| *byte == b'\n') {
        if line.last() != Some(&b'\n') {
            break;
        }
        let value: Value = match serde_json::from_slice(line) {
            Ok(value) => value,
            Err(_) => {
                end += line.len();
                continue;
            }
        };
        let seq = value.get("seq").and_then(Value::as_u64);
        if let (Some(seq), Some(upto)) = (seq, upto)
            && seq > upto.get()
        {
            return (&bytes[..end], last, true);
        }
        hashes_in(&value, referenced);
        last = seq.or(last);
        end += line.len();
    }
    (&bytes[..end], last, false)
}

/// 会话目录 `dir` 里的 [`MARKER`] 写着它是只读存档（施工 SE-1 下）：读不了、读不懂的当不是。核心拒改存档的方法、记忆不给
/// 存档抽取都照它。
pub fn archived(dir: &Path) -> bool {
    fs::read(dir.join(MARKER))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Marker>(&bytes).ok())
        .is_some_and(|marker| marker.archive)
}

/// 一个 JSON 值里写成内容哈希的字。
pub(crate) fn hashes_in(value: &Value, found: &mut BTreeSet<String>) {
    match value {
        Value::String(text) if ContentHash::parse(text).is_ok() => {
            found.insert(text.clone());
        }
        Value::Array(items) => items.iter().for_each(|item| hashes_in(item, found)),
        Value::Object(map) => map.values().for_each(|item| hashes_in(item, found)),
        _ => {}
    }
}

/// 会话目录里的段：名字是 12 位数字加 `.jsonl` 的，照名字排。
pub(crate) fn segment_names(dir: &Path) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            name.strip_suffix(".jsonl")
                .is_some_and(|stem| stem.len() == 12 && stem.bytes().all(|b| b.is_ascii_digit()))
        })
        .collect();
    names.sort();
    Ok(names)
}

/// 引用到的哈希里 blob 库有的，边读边拷进 `blobs/<64 位>`、重新算哈希；没有的交回放进 `absent`。
fn copy_blobs<W: Write + io::Seek>(
    zip: &mut ZipWriter<W>,
    options: SimpleFileOptions,
    blobs: &Blobs,
    referenced: &BTreeSet<String>,
) -> Result<(Vec<ManifestBlob>, Vec<ContentHash>), BundleError> {
    let (mut kept, mut absent) = (Vec::new(), Vec::new());
    for text in referenced {
        let Ok(hash) = ContentHash::parse(text) else {
            continue;
        };
        let mut source = match File::open(blobs.path(&hash)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                absent.push(hash);
                continue;
            }
            Err(error) => return Err(error.into()),
        };
        zip.start_file(format!("blobs/{}", hash.hex()), options)?;
        let (mut digest, mut bytes, mut buffer) = (Hasher::default(), 0u64, vec![0u8; 64 * 1024]);
        loop {
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            digest.update(&buffer[..n]);
            zip.write_all(&buffer[..n])?;
            bytes += n as u64;
        }
        if digest.finish() != hash {
            return Err(BundleError::BlobMismatch(hash));
        }
        kept.push(ManifestBlob { hash, bytes });
    }
    Ok((kept, absent))
}

/// 写好的临时文件挂到 `out`：硬链接只许新建，已经有了的报 [`BundleError::Exists`]；系统不支持硬链接的退回「没有才改名」。
fn publish(temp: &Path, out: &Path) -> Result<(), BundleError> {
    match fs::hard_link(temp, out) {
        Ok(()) => {
            discard(temp);
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Err(BundleError::Exists),
        Err(_) if out.symlink_metadata().is_err() => Ok(fs::rename(temp, out)?),
        Err(_) => Err(BundleError::Exists),
    }
}

#[cfg(test)]
mod tests;
