//! `assets: files` 的附件文件夹（施工 SE-1 中下下，调研第五节第 7 条）：Markdown 写到 `<目录>/<名字>.md` 时，图片、附件拷进
//! 同一个目录下的 `<名字>_files/`，Markdown 里写相对路径（`miyu_view::markdown::Files` 起名字、写链接）。
//!
//! - 文件夹的名字照 Markdown 的文件名去掉扩展名接 `_files`（`talk.md` 是 `talk_files`，没有扩展名的照整个名字），名字不是
//!   UTF-8 的照替换过的那一份：链接和真的文件夹是同一个名字。
//! - Markdown 或文件夹任一已经有了：`export_exists`，什么都不写（写之前先查，[`taken`]）。
//! - 一个附件都没拷的不建文件夹。建文件夹用 `create_dir`：别人刚好抢先建了的照样 `export_exists`。文件夹里的文件照 blob 拷，
//!   一个个新建（不覆盖）、同步；拷到一半出错、后面 Markdown 写不成的，整个文件夹删掉，不留半截。Markdown 最后写：看到它就是
//!   写完了的。
//! - blob 取得到没有，渲染前照 [`present`] 查（核对哈希）；取不到的渲染器照占位写，回应的 `assets.missing` 列出来。查过以后
//!   才没了的（拷的那一刻读不到、对不上）`session_broken`。

use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use miyu_kernel::id::ContentHash;
use miyu_store::blob::{BlobError, Blobs};
use miyu_view::markdown::Files;

use super::TARGET;
use crate::refusal::Refusal;

/// Markdown 文件 `out` 旁边的附件文件夹：交回文件夹的名字（链接用）和它的路径。
pub(super) fn folder_of(out: &Path) -> (String, PathBuf) {
    let stem = out
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let name = format!("{stem}_files");
    let path = out.with_file_name(&name);
    (name, path)
}

/// Markdown 或文件夹已经有了：`export_exists`。
///
/// # Errors
///
/// 任一已经有了。
pub(super) fn taken(out: &Path, folder: Option<&Path>) -> Result<(), Refusal> {
    let there = |path: &Path| path.symlink_metadata().is_ok();
    match there(out) || folder.is_some_and(there) {
        true => Err(Refusal::EXPORT_EXISTS),
        false => Ok(()),
    }
}

/// `wanted` 里取得到的 blob（核对过哈希）。
pub(super) fn present(blobs: &Blobs, wanted: BTreeSet<ContentHash>) -> Vec<ContentHash> {
    wanted
        .into_iter()
        .filter(|hash| blobs.check(hash).is_ok())
        .collect()
}

/// 把渲染时起了名字的 blob 拷进文件夹 `folder`：交回拷了几个。一个都没有的不建文件夹。
///
/// # Errors
///
/// 文件夹已经有了的 `export_exists`；建不了、写不进的 `export_unwritable`；查过以后 blob 没了、对不上的 `session_broken`。
/// 出错时建了的文件夹整个删掉。
pub(super) fn write(folder: &Path, files: &Files, blobs: &Blobs) -> Result<usize, Refusal> {
    let named = files.named();
    if named.is_empty() {
        return Ok(0);
    }
    if let Err(error) = fs::create_dir(folder) {
        return Err(match error.kind() {
            io::ErrorKind::AlreadyExists => Refusal::EXPORT_EXISTS,
            _ => unwritable(&error),
        });
    }
    for (hash, name) in &named {
        if let Err(refusal) = copy(blobs, hash, &folder.join(name)) {
            discard(folder);
            return Err(refusal);
        }
    }
    Ok(named.len())
}

/// 写不成以后，删掉建了的文件夹。删不掉的记一行，不盖过原来的错。
pub(super) fn discard(folder: &Path) {
    if let Err(error) = fs::remove_dir_all(folder) {
        tracing::warn!(target: TARGET, error = %error, "export folder not removed");
    }
}

/// 一个 blob 拷成文件 `to`：新建（不覆盖）、拷、同步。
fn copy(blobs: &Blobs, hash: &ContentHash, to: &Path) -> Result<(), Refusal> {
    let mut from = blobs.open_at(hash, 0).map_err(|error: BlobError| {
        tracing::warn!(target: TARGET, blob = %hash, error = %error, "export blob mismatch");
        Refusal::BROKEN
    })?;
    let written = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)
        .and_then(|mut file| {
            io::copy(&mut from, &mut file)?;
            file.sync_all()
        });
    written.map_err(|error| unwritable(&error))
}

/// 写不进：记一行（不记路径，运行日志不写路径），`export_unwritable`。
fn unwritable(error: &io::Error) -> Refusal {
    tracing::warn!(target: TARGET, error = %error, "export not written");
    Refusal::EXPORT_UNWRITABLE
}
