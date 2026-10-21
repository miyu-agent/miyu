//! 效果（`docs/designs/10-自带软件.md` 第五节，施工 4-6 上）：工具报的效果带着改前改后的内容，这里存成 blob、
//! 换成哈希，写成内核的效果；照内核的效果记下她看过的文件，会话里记、载入时从日志里重建，走的是同一个函数。
//! 撤掉的那几轮里看过的、改过的不算（施工 4-7 上）：撤销、恢复落了盘，照日志重算一遍。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use miyu_kernel::event::{Body, Effect, Event, FileChanged, FileRead, FileShown, FileTrashed};
use miyu_kernel::id::{ContentHash, TurnId};
use miyu_store::blob::Blobs;
use miyu_tool::Seen;

use crate::TARGET;

/// 工具报的效果写成内核的：改前改后的内容存成 blob，换成它们的哈希；派出去的任务照原样。存不下来的（磁盘满了之类）照样记下哈希，
/// 写一条运行日志：撤销时发现 blob 没了，说改前的内容没存下来。碰磁盘，在阻塞线程里调。
pub(crate) fn store(blobs: &Blobs, effects: Vec<miyu_tool::Effect>) -> Vec<Effect> {
    effects
        .into_iter()
        .map(|effect| match effect {
            miyu_tool::Effect::Read { path, lines, hash } => Effect::FileRead(FileRead {
                path: text(&path),
                lines,
                hash,
                skill: None,
                display_name: Default::default(),
            }),
            miyu_tool::Effect::Changed {
                path,
                before,
                after,
            } => Effect::FileChanged(FileChanged {
                path: text(&path),
                before: before.map(|content| put(blobs, &content)),
                after: put(blobs, &after),
            }),
            miyu_tool::Effect::Trashed { path, trash } => Effect::FileTrashed(FileTrashed {
                path: text(&path),
                trash,
            }),
            // 派出去的任务、给子代理留的言、订的会话照原样（施工 7-5、7-7、C-6）：没有要存的内容。
            miyu_tool::Effect::JobStarted(started) => Effect::JobStarted(started),
            miyu_tool::Effect::JobMessaged(messaged) => Effect::JobMessaged(messaged),
            miyu_tool::Effect::PeerWatch(watch) => Effect::PeerWatch(watch),
            miyu_tool::Effect::TodoWritten(written) => Effect::TodoWritten(written),
            // 给人看的文件（施工 D-5）：内容存成 blob。
            miyu_tool::Effect::Shown(shown) => Effect::FileShown(FileShown {
                path: text(&shown.path),
                blob: put(blobs, &shown.content),
                name: shown.name,
                media_type: shown.media_type,
                size: shown.content.len() as u64,
                title: shown.title,
                language: shown.language,
                width: shown.size.map(|(width, _)| width),
                height: shown.size.map(|(_, height)| height),
            }),
        })
        .collect()
}

/// 读了开着的技能文件夹 `shelves`（真实的位置）里的 `SKILL.md` 的，`file.read` 标上技能的名字（文件夹的名字，施工 X-10 下，
/// `10-自带软件.md` 第五节「用过的技能」）：压后照它重建。它在 `names` 里有给人看的名字的，一并记下（施工 9-8 五补）：历史
/// 照记下的写，卸了包、改了名字照旧。别的照原样。只比路径，不碰磁盘。
pub(crate) fn skilled(
    effects: Vec<Effect>,
    shelves: &[PathBuf],
    names: &crate::skills::Names,
) -> Vec<Effect> {
    effects
        .into_iter()
        .map(|effect| match effect {
            Effect::FileRead(mut read) => {
                let path = Path::new(&read.path);
                let skill = path
                    .parent()
                    .filter(|dir| {
                        path.file_name() == Some(SKILL_FILE.as_ref())
                            && shelves.iter().any(|shelf| shelf == dir)
                    })
                    .and_then(Path::file_name)
                    .map(|name| name.to_string_lossy().into_owned());
                read.skill = read.skill.or(skill);
                if read.display_name.is_empty()
                    && let Some(names) = read.skill.as_ref().and_then(|skill| names.get(skill))
                {
                    read.display_name.clone_from(names);
                }
                Effect::FileRead(read)
            }
            other => other,
        })
        .collect()
}

/// 技能文件的名字。
const SKILL_FILE: &str = "SKILL.md";

/// 照内核的效果记下她看过的文件：读过的记读的时候整份的哈希，写过的记改后的。移进了回收站的，不在了。
pub(crate) fn saw(seen: &mut Seen, effects: &[Effect]) {
    for effect in effects {
        match effect {
            Effect::FileRead(read) => {
                seen.insert(PathBuf::from(&read.path), read.hash.clone());
            }
            Effect::FileChanged(changed) => {
                seen.insert(PathBuf::from(&changed.path), changed.after.clone());
            }
            Effect::FileTrashed(trashed) => {
                seen.remove(Path::new(&trashed.path));
            }
            // 派出去的任务、留的言、订的别的会话、写的待办不是看过的文件（施工 7-1、7-7、C-1、D-3）；给人看的她没拿到内容，
            // 也不算看过（施工 D-5）。
            Effect::JobStarted(_)
            | Effect::JobMessaged(_)
            | Effect::PeerWatch(_)
            | Effect::TodoWritten(_)
            | Effect::FileShown(_)
            | Effect::Unknown(_) => {}
        }
    }
}

/// 从日志里重建她看过的文件：照先后过一遍每一条工具结果的效果，现在还撤着的回合里的不算（有效历史，
/// `03-事件模型.md` 第七节）。
pub(crate) fn seen_in(events: &[Event]) -> Seen {
    let mut scan = SeenScan::default();
    for event in events {
        scan.note(event);
    }
    scan.seen()
}

/// 边读日志边记她看过的文件（施工 V-15：载入时一块块读，读过的放掉）：工具结果的效果连同它的回合照先后记下，撤销、恢复
/// 照先后认；读完了 [`SeenScan::seen`] 照现在还撤着的回合挑，同 [`seen_in`]。
#[derive(Debug, Default)]
pub(crate) struct SeenScan {
    /// 现在还撤着的回合。
    reverted: BTreeSet<TurnId>,
    /// 带了效果的工具结果：回合、效果，照先后。
    results: Vec<(Option<TurnId>, Vec<Effect>)>,
}

impl SeenScan {
    /// 记下一条。
    pub(crate) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::TurnReverted(undone) => self.reverted.extend(undone.turns.iter().copied()),
            Body::TurnUnreverted(redone) => {
                for turn in &redone.turns {
                    self.reverted.remove(turn);
                }
            }
            Body::ToolResult(result) if !result.effects.is_empty() => {
                self.results.push((event.turn, result.effects.clone()));
            }
            _ => {}
        }
    }

    /// 她看过的文件：现在还撤着的回合里的不算。
    pub(crate) fn seen(&self) -> Seen {
        let mut seen = Seen::new();
        for (turn, effects) in &self.results {
            if !turn.is_some_and(|turn| self.reverted.contains(&turn)) {
                saw(&mut seen, effects);
            }
        }
        seen
    }
}

/// 这一批里有没有撤销、恢复：有的，她看过的要照日志重算。
pub(crate) fn reverts(events: &[Event]) -> bool {
    events
        .iter()
        .any(|event| matches!(event.body, Body::TurnReverted(_) | Body::TurnUnreverted(_)))
}

/// 存一份内容，交回它的哈希。存不下来的照样算出哈希交回。
fn put(blobs: &Blobs, content: &[u8]) -> ContentHash {
    match blobs.put(content) {
        Ok(hash) => hash,
        Err(error) => {
            tracing::warn!(target: TARGET, error = %error, "effect content not stored");
            ContentHash::of(content)
        }
    }
}

/// 路径写成事件里的字。
fn text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests;
