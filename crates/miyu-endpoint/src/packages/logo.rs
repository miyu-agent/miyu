//! 包自带的图标（施工 F-18 上，`[package] logo`）：包目录里一张 PNG 或 WebP，不超过 [`LOGO_MAX_BYTES`]、每边不超过
//! [`LOGO_MAX_SIDE`]。核心只查（看开头的字节认格式、只看文件头量宽高，同人格的头像），不解码、不缩：缩是打包时的事
//! （`miyu pkg pack`）。不合的当没有图标、包照装，`miyu check` 报一条；`package.list` 每一项带它的版本（内容的 SHA-256 前 16 位，
//! 头照版本换缓存），图用 `package.logo` 读。软件源里的（`package.search` 每一行的 `logo`）写 `from` 读（施工 F-18 下，
//! [`super::source::logo`]）。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use miyu_config::package::{LOGO_MAX_BYTES, LOGO_MAX_SIDE, Manifest};
use miyu_kernel::id::AccountId;
use miyu_store::packages::Found;

use crate::Core;
use crate::refusal::Refusal;

/// 一张合规矩的图标：字节、媒体类型、版本。
pub(crate) struct Logo {
    pub(crate) bytes: Vec<u8>,
    pub(crate) media_type: &'static str,
    pub(crate) version: String,
}

/// 包 `found`（清单 `manifest`）的图标不合规矩的原因代码（`miyu check` 照它报）：不在（照真实路径，出了包目录的也算不在）
/// `logo_missing`，不是 PNG、WebP 的 `logo_not_image`，超过字节或边长的 `logo_too_big`。没写、合规矩的没有。碰磁盘。
pub(crate) fn problem(found: &Found, manifest: &Manifest) -> Option<&'static str> {
    manifest.logo.as_ref()?;
    read(found, manifest).err()
}

/// 包 `found` 合规矩的图标；没写的、不合的没有。碰磁盘。
pub(crate) fn of(found: &Found, manifest: &Manifest) -> Option<Logo> {
    read(found, manifest).ok()
}

/// 读包 `found` 的图标、查规矩：不合的交回原因代码。
fn read(found: &Found, manifest: &Manifest) -> Result<Logo, &'static str> {
    let path = manifest.logo.as_deref().ok_or("logo_missing")?;
    let real = inside(&found.files_dir(), path).ok_or("logo_missing")?;
    let size = std::fs::metadata(&real).map_err(|_| "logo_missing")?.len();
    if size > LOGO_MAX_BYTES {
        return Err("logo_too_big");
    }
    let bytes = std::fs::read(&real).map_err(|_| "logo_missing")?;
    let media_type = miyu_tool::picture::kind(&bytes)
        .filter(|kind| matches!(*kind, "image/png" | "image/webp"))
        .ok_or("logo_not_image")?;
    let (width, height) = miyu_tool::picture::measure(&bytes).ok_or("logo_not_image")?;
    if width > LOGO_MAX_SIDE || height > LOGO_MAX_SIDE {
        return Err("logo_too_big");
    }
    let version = Sha256::digest(&bytes)
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Ok(Logo {
        bytes,
        media_type,
        version,
    })
}

/// 包目录 `dir` 里的 `path`：照真实路径，在包目录里、是普通文件的才有（顺着链接出了包目录的不算）。
fn inside(dir: &Path, path: &str) -> Option<PathBuf> {
    let root = std::fs::canonicalize(dir).ok()?;
    let real = std::fs::canonicalize(dir.join(path)).ok()?;
    (real.starts_with(&root) && real.is_file()).then_some(real)
}

/// `package.logo` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LogoParams {
    package: String,
    /// 读哪个软件源里的（施工 F-18 下，同 `package.install` 的 `from`）：不写的读装着的。
    #[serde(default)]
    from: Option<String>,
}

/// `package.logo {"package"}`：`account` 看得到的装着的包的图标，回 `{"package", "logo", "media_type", "data"}`（`data` 是
/// base64，`logo` 是版本）；没写、不合规矩的三格都是 `null`。没装的 `unknown_package`。写了 `from` 的读那个软件源索引里的
/// （[`super::source::logo::of`]）。
pub(crate) async fn method(
    core: &Arc<Core>,
    account: &AccountId,
    params: LogoParams,
) -> Result<Value, Refusal> {
    if let Some(from) = &params.from {
        let got = super::source::logo::of(core, account, (&params.package, from)).await?;
        return Ok(answer(
            &params.package,
            got.map(|(bytes, media_type, version)| Logo {
                bytes,
                media_type,
                version,
            }),
        ));
    }
    let seen = core.packages_seen(account);
    let found = seen
        .iter()
        .find(|one| one.id == params.package)
        .cloned()
        .ok_or(Refusal::UNKNOWN_PACKAGE)?;
    let logo = tokio::task::spawn_blocking(move || {
        found
            .read
            .as_ref()
            .ok()
            .and_then(|manifest| of(&found, manifest))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    Ok(answer(&params.package, logo))
}

/// `package.logo` 的回应：包 `package` 的图标 `logo`，没有的三格 `null`。
fn answer(package: &str, logo: Option<Logo>) -> Value {
    match logo {
        Some(logo) => json!({
            "package": package,
            "logo": logo.version,
            "media_type": logo.media_type,
            "data": STANDARD.encode(&logo.bytes),
        }),
        None => json!({"package": package, "logo": null, "media_type": null, "data": null}),
    }
}
