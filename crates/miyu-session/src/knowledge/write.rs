//! 她写知识库（施工 K-3，`docs/blueprint/knowledge.md`「怎么走」第七条，19 N14）：写哪几个、能不能建新库，交给知识库那一层
//! 去写。写得了的是她这一刻查得到的库（第六条第 1 款）；写到还没有的库就是建它，人格收窄了的不建（建了她也查不到），账号下
//! 有、人格没列的当没有。

use miyu_store::knowledge::Knowledge;
use miyu_store::layers::valid;
use miyu_tool::{KbRefused, KbSave, KbTrashed, KbWritten};

use super::{Shelves, refused};

/// 写一篇：库是她查得到的照写；还没有的、人格没收窄的先建，`about` 写进它的 `kb.toml`。路径不合写法的不建库。
pub(super) fn save(shelves: &Shelves, save: KbSave) -> Result<KbWritten, KbRefused> {
    let bases = shelves.bases().map_err(KbRefused::Failed)?;
    let created = !bases.contains(&save.kb);
    if created {
        creatable(shelves, &save.kb, &save.path)?;
        shelves
            .knowledge
            .create(&shelves.account, &save.kb, save.about.as_deref())
            .map_err(|error| refused(error, save.kb.clone(), save.path.clone()))?;
    }
    let written = shelves
        .knowledge
        .save(
            &shelves.account,
            &save.kb,
            &save.path,
            &save.text,
            save.append,
        )
        .map_err(|error| refused(error, save.kb, save.path))?;
    Ok(KbWritten {
        path: written.path,
        before: written.before,
        after: written.after,
        created,
    })
}

/// 照原文改一段：库得是她查得到的。
pub(super) fn edit(
    shelves: &Shelves,
    kb: String,
    path: String,
    old: &str,
    new: &str,
) -> Result<KbWritten, KbRefused> {
    visible(shelves, &kb)?;
    let written = shelves
        .knowledge
        .edit(&shelves.account, &kb, &path, old, new)
        .map_err(|error| refused(error, kb, path))?;
    Ok(KbWritten {
        path: written.path,
        before: written.before,
        after: written.after,
        created: false,
    })
}

/// 挪进回收站：库得是她查得到的。
pub(super) fn delete(shelves: &Shelves, kb: String, path: String) -> Result<KbTrashed, KbRefused> {
    visible(shelves, &kb)?;
    let (file, trash) = shelves
        .knowledge
        .trash(&shelves.account, &kb, &path, shelves.home.as_deref())
        .map_err(|error| refused(error, kb, path))?;
    Ok(KbTrashed { path: file, trash })
}

/// 库 `kb` 她这一刻查得到。
fn visible(shelves: &Shelves, kb: &str) -> Result<(), KbRefused> {
    match shelves
        .bases()
        .map_err(KbRefused::Failed)?
        .iter()
        .any(|id| id == kb)
    {
        true => Ok(()),
        false => Err(KbRefused::NoBase(kb.to_string())),
    }
}

/// 建得了库 `kb`：人格没收窄（收窄了的建了她也查不到；账号下有、人格没列的也就落在这里，当没有）；编号合写法（不合的说清
/// 怎么写，施工 K-3 补：R-9 真跑时她把编号写成中文，听到「没有这个库」又试了两次）；`path` 也得合写法（不然建了个空库却
/// 什么都没写）。没收窄的她查得到账号下全部，走到这里的库一定还没有。
fn creatable(shelves: &Shelves, kb: &str, path: &str) -> Result<(), KbRefused> {
    if shelves.narrowing().map_err(KbRefused::Failed)?.is_some() {
        return Err(KbRefused::NoBase(kb.to_string()));
    }
    if !valid(kb) {
        return Err(KbRefused::BadId(kb.to_string()));
    }
    Knowledge::writable(path).map_err(|error| refused(error, kb.to_string(), path.to_string()))
}
