//! `skill.remove`（施工 W-13，网页的技能页：单个删、多选删）：把管理员自己放的技能整个文件夹挪进系统回收站，同 `kb.remove`。
//!
//! 只删管理员家目录里放的（`skills/<文件夹>/`）；随包来的（出厂的、装的包带的）不能删，拒绝
//! `skill_from_package`；别家共用目录（`~/.agents/skills/`，施工 PT-2 中）里的也不删，别的客户端也在用，拒绝 `skill_shared`。找法同 `skill.list`：同名的照核心实际用的那一份认，自己放的盖住了包里的，删的就是自己放的那一份，
//! 删了包里的那一份重新露出来。几个名字先全查一遍，有一个不行就一个都不删；查过了才挪，挪到一半挪不动的，前面挪了的
//! 已经在回收站里，拒绝 `trash_failed`。技能文件夹是链接的（施工 X-37）只挪链接本身，不碰它指到的文件夹。

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::Core;
use crate::refusal::Refusal;

/// `skill.remove` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveParams {
    /// 技能名，至少一个。
    names: Vec<String>,
}

/// `skill.remove`：`{"trash": [<回收站里的位置>, …]}`，照 `names` 的先后。
///
/// # Errors
///
/// 一个名字都没写、写了重复的：`bad_params`。没有这个技能：`unknown_skill`；随包来的：`skill_from_package`（两种都带
/// `data.name`）。挪不进回收站：`trash_failed`。找、挪的线程崩了：内部出错。
pub(crate) async fn remove(core: &Core, params: RemoveParams) -> Result<Value, Refusal> {
    let names = params.names;
    let mut seen = std::collections::BTreeSet::new();
    if names.is_empty() || !names.iter().all(|name| seen.insert(name.as_str())) {
        return Err(Refusal::BAD_PARAMS);
    }
    // 只删调用者自己放的（施工 Y-2 中上）：系统区的、包里的照「随包来的」拒。
    let (packages, root) = (core.packages_of(), core.root.clone());
    let home = core.home.clone();
    let external = super::external(core);
    tokio::task::spawn_blocking(move || {
        let found = miyu_store::skills::find(&packages, &root, &external).0;
        let own = std::fs::canonicalize(miyu_store::skills::own_dir(&root)).ok();
        let folders = names
            .iter()
            .map(|name| {
                let skill = found
                    .iter()
                    .find(|skill| skill.head.name == *name)
                    .ok_or_else(|| Refusal::unknown_skill(name))?;
                if skill.package.is_some() {
                    return Err(Refusal::skill_from_package(name));
                }
                // 别的目录里的不删（施工 PT-2 中、PT-2 外）：别的客户端也在用。
                if skill.external.is_some() {
                    return Err(Refusal::skill_shared(name));
                }
                match &skill.link {
                    Some(link) => own_link(link, own.as_deref()),
                    None => folder(&skill.file, own.as_deref()),
                }
                .ok_or_else(|| Refusal::unknown_skill(name))
            })
            .collect::<Result<Vec<PathBuf>, Refusal>>()?;
        let trash = folders
            .iter()
            .map(|folder| {
                miyu_fs::trash::put(folder, home.as_deref()).map_err(|refused| {
                    Refusal::trash_failed(match refused {
                        miyu_fs::trash::Refused::Unavailable => "no trash on this disk".to_string(),
                        miyu_fs::trash::Refused::Lost => {
                            "moved but not found in the trash".to_string()
                        }
                        miyu_fs::trash::Refused::Failed(error) => error.to_string(),
                    })
                })
            })
            .collect::<Result<Vec<String>, Refusal>>()?;
        Ok(json!({ "trash": trash }))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}

/// 技能 `SKILL.md` 所在的文件夹，照真实的位置；它得正好在管理员自己放技能的文件夹 `own` 下面一层，不然不删（防着链接
/// 指到别处）。
fn folder(file: &Path, own: Option<&Path>) -> Option<PathBuf> {
    let folder = std::fs::canonicalize(file.parent()?).ok()?;
    (folder.parent()? == own?).then_some(folder)
}

/// 链接成的技能（施工 X-37）：链接本身，它得正好在管理员自己放技能的文件夹 `own` 下面一层。
fn own_link(link: &Path, own: Option<&Path>) -> Option<PathBuf> {
    let (place, name) = (
        std::fs::canonicalize(link.parent()?).ok()?,
        link.file_name()?,
    );
    (place == own?).then(|| place.join(name))
}
