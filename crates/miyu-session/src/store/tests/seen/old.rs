//! 以前撤销、恢复以后重算她看过的文件（施工 V-41 中以前的 `effects.rs` 的 `seen_in` 和它用的几样，原样抄来当准绳）：
//! 整份读进内存（`Store::events`），再照先后过一遍。

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use miyu_kernel::event::{Body, Effect, Event, FileChanged};
use miyu_kernel::id::TurnId;
use miyu_tool::Seen;

/// 照内核的效果记下她看过的文件：读过的记读的时候整份的哈希，写过的记改后的。移进了回收站的，不在了。
pub(super) fn saw(seen: &mut Seen, effects: &[Effect]) {
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
            | Effect::PackageState(_)
            | Effect::FileShown(_)
            | Effect::Unknown(_) => {}
        }
    }
}

/// 从日志里重建她看过的文件：照先后过一遍每一条工具结果的效果，现在还撤着的回合里的不算（有效历史，
/// `03-事件模型.md` 第七节）。
pub(super) fn seen_in(events: &[Event]) -> Seen {
    let mut scan = SeenScan::default();
    for event in events {
        scan.note(event);
    }
    scan.seen()
}

/// 边读日志边记她看过的文件（施工 V-15：载入时一块块读，读过的放掉）：工具结果的效果连同它的回合照先后记下，撤销、恢复
/// 照先后认；读完了 [`SeenScan::seen`] 照现在还撤着的回合挑，同 [`seen_in`]。
#[derive(Debug, Default)]
pub(super) struct SeenScan {
    /// 现在还撤着的回合。
    reverted: BTreeSet<TurnId>,
    /// 带了效果的工具结果：回合、效果，照先后。
    results: Vec<(Option<TurnId>, Vec<Effect>)>,
}

impl SeenScan {
    /// 记下一条。
    pub(super) fn note(&mut self, event: &Event) {
        match &event.body {
            Body::TurnReverted(undone) => self.reverted.extend(undone.turns.iter().copied()),
            Body::TurnUnreverted(redone) => {
                for turn in &redone.turns {
                    self.reverted.remove(turn);
                }
            }
            Body::ToolResult(result) if !result.effects.is_empty() => {
                // 只留哈希（施工 V-18）：长会话载入时不把改过的原文全攒在内存里。
                let effects = result.effects.iter().cloned().map(hashes_only).collect();
                self.results.push((event.turn, effects));
            }
            _ => {}
        }
    }

    /// 她看过的文件：现在还撤着的回合里的不算。
    pub(super) fn seen(&self) -> Seen {
        let mut seen = Seen::new();
        for (turn, effects) in &self.results {
            if !turn.is_some_and(|turn| self.reverted.contains(&turn)) {
                saw(&mut seen, effects);
            }
        }
        seen
    }
}

/// 去掉改了的文件的原文，只留哈希（施工 V-18）。别的效果照原样。
fn hashes_only(effect: Effect) -> Effect {
    match effect {
        Effect::FileChanged(changed) => Effect::FileChanged(FileChanged {
            before_text: None,
            after_text: None,
            ..changed
        }),
        other => other,
    }
}
