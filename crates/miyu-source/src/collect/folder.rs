//! 从一个文件夹收（施工 G-13，自建的软件源）：文件夹里的每个 `.miyu` 照收 GitHub 那一套读成一条（[`super::read_package`]），
//! 下载地址写成 `<源的地址>/files/<文件名>`。同一个包几种系统的文件并成一条；同一个编号有两个版本的、读不成的不收，记一笔。
//! 不查 `names.toml`、`blocked.toml`：自建的源自己管自己收什么。条目的来源（`origin`）写源的地址，不是官方的。

use std::collections::BTreeMap;
use std::path::Path;

use super::{Collected, Gathered, MOST_FILE, read_package};
use crate::archive;
use crate::index::{Entry, Index, VERSION};

/// 收 `folder` 里的 `.miyu`：`url` 是这个源发出去的地址（文件放在它下面的 `files/`），`scratch` 是解包用的空目录，`generated`
/// 写进索引的时刻。交回收成的索引（没签名）和图标；`names` 是空的。
///
/// # Errors
///
/// 文件夹读不了。
pub fn from_folder(
    folder: &Path,
    url: &str,
    scratch: &Path,
    generated: &str,
) -> Result<Collected, String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .map_err(|error| format!("{}: {error}", folder.display()))?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| name.ends_with(&format!(".{}", archive::EXTENSION)))
        .collect();
    names.sort();
    let base = url.trim_end_matches('/');
    let mut notes = Vec::new();
    let mut found: BTreeMap<String, Gathered> = BTreeMap::new();
    for (n, name) in names.iter().enumerate() {
        let bytes = match read_small(&folder.join(name)) {
            Ok(bytes) => bytes,
            Err(why) => {
                notes.push(format!("{name}: skipped, {why}"));
                continue;
            }
        };
        let place = scratch.join(n.to_string());
        let read = read_package(
            name,
            &format!("{base}/files/{name}"),
            &bytes,
            &place,
            (base, false),
        );
        super::discard(&place);
        let read = match read {
            Ok(read) => read,
            Err(why) => {
                notes.push(format!("{name}: skipped, {why}"));
                continue;
            }
        };
        let id = read.gathered.entry.id.clone();
        match found.get_mut(&id) {
            None => {
                if let Some(note) = &read.gathered.note {
                    notes.push(format!("{name}: {note}"));
                }
                let mut gathered = read.gathered;
                gathered.entry.files.push(read.file);
                found.insert(id, gathered);
            }
            Some(first) if first.entry.version != read.gathered.entry.version => {
                notes.push(format!(
                    "{name}: skipped, {id} {} is already collected",
                    first.entry.version
                ));
            }
            Some(first) => {
                first.entry.installed = first.entry.installed.max(read.gathered.entry.installed);
                first.entry.files.push(read.file);
            }
        }
    }
    let mut logos = BTreeMap::new();
    let mut packages: Vec<Entry> = Vec::new();
    for gathered in found.into_values() {
        let mut entry = gathered.entry;
        entry.files.sort_by(|a, b| a.target.cmp(&b.target));
        if let (Some(at), Some(bytes)) = (&entry.logo, gathered.logo) {
            logos.insert(at.path.clone(), bytes);
        }
        notes.push(format!("{} {}", entry.id, entry.version));
        packages.push(entry);
    }
    Ok(Collected {
        index: Index {
            version: VERSION,
            generated: generated.to_string(),
            packages,
            blocked: Vec::new(),
        },
        names: BTreeMap::new(),
        notes,
        logos,
    })
}

/// 读一个包文件：超过 [`MOST_FILE`] 的不收。
fn read_small(file: &Path) -> Result<Vec<u8>, String> {
    let size = std::fs::metadata(file)
        .map_err(|error| error.to_string())?
        .len();
    if size > MOST_FILE {
        return Err("too big".to_string());
    }
    std::fs::read(file).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests;
