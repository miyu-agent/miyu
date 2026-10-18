//! `miyu pkg pack`（施工 G-1，设计 `37-软件源.md` 第二节第 4 条）：照装的那一套读清单，把包文件夹打成 `.miyu`，印文件名、
//! SHA-256、大小。不连核心：作者在自己的机器、CI 上打。

use std::io::Write;
use std::path::Path;

use serde_json::json;

use miyu_config::package::{self, Target};
use miyu_source::archive;

use super::joined;
use crate::exit;
use crate::language::Language;
use crate::shown::say;

/// 要打的：哪个文件夹、打给哪一种系统、写到哪个目录。
pub(super) struct Wanted<'a> {
    pub(super) folder: &'a str,
    pub(super) target: Option<&'a str>,
    pub(super) out: Option<&'a str>,
}

/// 打一次，交回退出码：打好了印在 `out`（`json` 的印一串），不成的原因印在 `err`、退出码 1。
pub(super) fn pack(
    wanted: &Wanted<'_>,
    cwd: &Path,
    language: &Language,
    json: bool,
    (out, err): (&mut dyn Write, &mut dyn Write),
) -> u8 {
    match packed(wanted, cwd, language) {
        Ok((packed, id, version, target)) if json => {
            let shown = json!({
                "package": id,
                "version": version,
                "target": target.map(Target::as_str),
                "path": packed.path,
                "sha256": packed.sha256,
                "size": packed.size,
            });
            say(out, &shown.to_string());
            exit::OK
        }
        Ok((packed, ..)) => {
            say(out, &language.packed(&packed.path.display().to_string()));
            say(out, &format!("SHA-256: {}", packed.sha256));
            say(out, &language.packed_size(packed.size));
            exit::OK
        }
        Err(reason) => {
            say(err, &reason);
            exit::ERROR
        }
    }
}

/// 打好的、编号、版本、系统；不成的是给人看的那一句。
fn packed(
    wanted: &Wanted<'_>,
    cwd: &Path,
    language: &Language,
) -> Result<(archive::Packed, String, String, Option<Target>), String> {
    let mut folder = joined(cwd, wanted.folder);
    if folder.file_name().and_then(|name| name.to_str()) == Some("package.toml") {
        folder.pop();
    }
    let id = folder
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string();
    if !miyu_store::personas::valid(&id) {
        return Err(language.not_a_package_id(&id));
    }
    let text = std::fs::read_to_string(folder.join("package.toml"))
        .map_err(|_| language.unreadable(&folder.join("package.toml").display().to_string()))?;
    let manifest =
        package::read(&text).map_err(|problem| language.cannot_pack(&problem.to_string()))?;
    let version = manifest.version.ok_or_else(|| language.no_version())?;
    let target = target(&manifest.targets, wanted.target, language)?;
    let dir = wanted
        .out
        .map_or_else(|| cwd.to_path_buf(), |out| joined(cwd, out));
    std::fs::create_dir_all(&dir).map_err(|error| language.cannot_pack(&error.to_string()))?;
    let file = dir.join(archive::file_name(&id, &version, target));
    let packed = archive::pack(&folder, &id, &file)
        .map_err(|error| language.cannot_pack(&error.to_string()))?;
    Ok((packed, id, version, target))
}

/// 打给哪一种：清单没写 `targets` 的不许写 `--target`；写了的必须挑其中一种。
fn target(
    targets: &[Target],
    asked: Option<&str>,
    language: &Language,
) -> Result<Option<Target>, String> {
    let listed = || {
        targets
            .iter()
            .map(|target| target.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    };
    match (targets.is_empty(), asked) {
        (true, None) => Ok(None),
        (true, Some(_)) => Err(language.no_targets()),
        (false, None) => Err(language.needs_target(&listed())),
        (false, Some(asked)) => Target::parse(asked)
            .filter(|target| targets.contains(target))
            .map(Some)
            .ok_or_else(|| language.not_listed_target(asked, &listed())),
    }
}
