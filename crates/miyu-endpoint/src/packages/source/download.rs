//! 下包文件（施工 G-4 上，设计 `37-软件源.md` 第四节第 4、5 条、第五节）：照镜像的 `files/<文件名>`、索引写的地址的先后下，边下
//! 边算 SHA-256、数字节，超过索引写的大小就停，下完对不上的试下一处；下好的放 `state/packages/.cache/<文件名>`（照
//! `/var/cache/pacman/pkg`），缓存里已经有、哈希对得上的不再下。进度照 `report` 报（下了多少、一共多少）。

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use sha2::{Digest, Sha256};

use miyu_http::Proxy;
use miyu_source::index::File;

/// 两块字节之间最多等多久：大文件不设总时限，卡住的才算不成。
const STALL: Duration = Duration::from_secs(60);

/// 一个包文件要下的：名字、索引里那一份、镜像（结尾带 `/`），和要不要照地址闸只连公网（个人层的源，施工 U23 下下）。
pub(super) struct Wanted<'a> {
    pub(super) name: &'a str,
    pub(super) file: &'a File,
    pub(super) mirrors: &'a [String],
    pub(super) guarded: Option<&'a miyu_http::Guarded>,
}

/// 缓存目录：`state/packages/.cache/`。
pub(super) fn cache_dir(state_root: &Path) -> PathBuf {
    state_root.join(".cache")
}

/// 下一个包文件到 `cache`，交回它的位置。
///
/// # Errors
///
/// 哪一处都没下成：每一处的原因（英文，带地址），照先后用 `; ` 接起来。
pub(super) async fn fetch(
    cache: &Path,
    wanted: &Wanted<'_>,
    report: &(dyn Fn(u64, u64) + Sync),
) -> Result<PathBuf, String> {
    let target = cache.join(wanted.name);
    let total = wanted.file.size;
    if sha256_of(&target).as_deref() == Some(wanted.file.sha256.as_str()) {
        report(total, total);
        return Ok(target);
    }
    std::fs::create_dir_all(cache).map_err(|error| error.to_string())?;
    let urls = wanted
        .mirrors
        .iter()
        .map(|mirror| format!("{mirror}files/{}", wanted.name))
        .chain(std::iter::once(wanted.file.url.clone()));
    let mut reasons: Vec<String> = Vec::new();
    for url in urls {
        let partial = cache.join(format!(".{}.part", wanted.name));
        match one((&url, wanted.guarded), &partial, wanted.file, report).await {
            Ok(()) => {
                std::fs::rename(&partial, &target).map_err(|error| error.to_string())?;
                return Ok(target);
            }
            Err(why) => {
                if let Err(error) = std::fs::remove_file(&partial)
                    && error.kind() != std::io::ErrorKind::NotFound
                {
                    tracing::debug!(target: super::super::TARGET, error = %error, "partial package file left");
                }
                reasons.push(format!("{url}: {why}"));
            }
        }
    }
    Err(reasons.join("; "))
}

/// 从 `url` 下到 `partial`：大小、哈希都对上才算成。`guarded` 的照地址闸只连公网，跳转每一跳过闸。
async fn one(
    (url, guarded): (&str, Option<&miyu_http::Guarded>),
    partial: &Path,
    file: &File,
    report: &(dyn Fn(u64, u64) + Sync),
) -> Result<(), String> {
    let mut response = match guarded {
        Some(guarded) => guarded.open(url, STALL).await?,
        None => {
            let proxy = if miyu_http::is_loopback_url(url) {
                Proxy::Off
            } else {
                Proxy::FromEnvironment
            };
            let client = miyu_http::fetcher(proxy).map_err(|error| error.to_string())?;
            tokio::time::timeout(STALL, client.get(url).send())
                .await
                .map_err(|_| "timed out".to_string())?
                .map_err(|error| error.without_url().to_string())?
        }
    };
    if !response.status().is_success() {
        return Err(format!("HTTP {}", response.status().as_u16()));
    }
    let mut out = std::fs::File::create(partial).map_err(|error| error.to_string())?;
    let mut hasher = Sha256::new();
    let mut done: u64 = 0;
    report(0, file.size);
    loop {
        let chunk = tokio::time::timeout(STALL, response.chunk())
            .await
            .map_err(|_| "stalled".to_string())?
            .map_err(|error| error.without_url().to_string())?;
        let Some(chunk) = chunk else {
            break;
        };
        done += chunk.len() as u64;
        if done > file.size {
            return Err(format!("larger than the {} bytes in the index", file.size));
        }
        hasher.update(&chunk);
        out.write_all(&chunk).map_err(|error| error.to_string())?;
        report(done, file.size);
    }
    out.sync_all().map_err(|error| error.to_string())?;
    if done != file.size {
        return Err(format!("{done} bytes, the index says {}", file.size));
    }
    if hex(&hasher.finalize()) != file.sha256 {
        return Err("the SHA-256 does not match the index".to_string());
    }
    Ok(())
}

/// 文件的 SHA-256；读不了的没有。
fn sha256_of(path: &Path) -> Option<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).ok()?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Some(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
