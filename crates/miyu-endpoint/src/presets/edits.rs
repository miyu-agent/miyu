//! 文件改动的版本（施工 OV-1 下，`docs/construction/OV-1-同名覆盖工具说明和提示词.md` 第十节）：盯主人家目录（施工 SU-3 下：系统区
//! 那一层拿掉）的 `tools/`、`prompts/`、`personas/`、`presets/`，往下递归（`skills/` 施工 PT-2 上拿掉了：技能那一块钉在快照里，
//! 中途变了走尾巴注入，system 不变、前缀不变，问人是误报）；变一下记一条：第几下、谁的、照数据根写的相对路径。会话回合开始只比
//! 号（`miyu_session::prefix`），发送前照它问人「前缀要变了」（`prefix_changing`）。
//!
//! - 只在内存里：核心重启从零数，会话也从零记，不会误判。
//! - 还没有的目录登记时建空的：这几个本来就是给人放自定义的地方。
//! - 用到时才盯（开会话、载入时，[`Core::watch_edits`]）。
//! - 只留最近 [`KEPT`] 条：太久以前的改动问人时只说「还有几个」。

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use miyu_kernel::id::AccountId;
use miyu_store::watch::{Watch, watch_tree};

use crate::Core;

/// 盯的几棵树，在每一层下面的名字。
const TREES: [&str; 4] = ["tools", "prompts", "personas", "presets"];

/// 留几条改动。
const KEPT: usize = 64;

/// 运行日志的目标。
const TARGET: &str = "miyu::edits";

/// 一条改动。
#[derive(Debug, Clone)]
struct Change {
    /// 第几下。
    seq: u64,
    /// 谁的。
    whose: AccountId,
    /// 照数据根写的相对路径。
    path: String,
}

/// 改动的表。
#[derive(Default)]
struct State {
    /// 一共几下。
    seq: u64,
    /// 每个家目录最后一下的号。
    latest: BTreeMap<AccountId, u64>,
    /// 最近的几条。
    log: VecDeque<Change>,
    /// 盯上了的家目录。
    watched: BTreeSet<AccountId>,
    /// 盯着的：丢掉就不盯了。
    watches: Vec<Watch>,
}

/// 文件改动的版本：核心一份。
#[derive(Default)]
pub(crate) struct Edits(Mutex<State>);

impl Edits {
    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 账号 `owner` 这时的号：他家目录的最后一下。
    pub(crate) fn version(&self, owner: &AccountId) -> u64 {
        self.state().latest.get(owner).copied().unwrap_or(0)
    }

    /// 账号 `owner` 号 `after` 以后改了的文件，照先后、不重；最多 `most` 个，多的交回还有几个。早就挪出表的那些也算进「还有」。
    pub(crate) fn since(&self, owner: &AccountId, after: u64, most: usize) -> (Vec<String>, usize) {
        let state = self.state();
        let mut files: Vec<String> = Vec::new();
        for change in state
            .log
            .iter()
            .filter(|change| change.seq > after && change.whose == *owner)
        {
            if !files.contains(&change.path) {
                files.push(change.path.clone());
            }
        }
        // 挪出表的：号比表里最早的那一条还早、又比 `after` 新的，至少有一个。
        let lost = state
            .log
            .front()
            .is_some_and(|first| first.seq > after + 1 && state.seq > KEPT as u64);
        let more = files.len().saturating_sub(most) + usize::from(lost);
        files.truncate(most);
        (files, more)
    }

    /// 记一下：`whose` 家目录里的 `path` 变了。
    fn record(&self, whose: AccountId, path: String) {
        let mut state = self.state();
        state.seq += 1;
        let seq = state.seq;
        state.latest.insert(whose.clone(), seq);
        state.log.push_back(Change { seq, whose, path });
        while state.log.len() > KEPT {
            state.log.pop_front();
        }
    }
}

impl Core {
    /// 账号 `owner` 这时的号：测试等盯着的那一头看到改动用。
    #[cfg(feature = "testkit")]
    #[must_use]
    pub fn edits_version(&self, owner: &AccountId) -> u64 {
        self.edits.version(owner)
    }

    /// 盯上账号 `owner` 的家目录（盯过的不再盯）：开会话、载入时调。
    pub(crate) fn watch_edits(self: &Arc<Self>, owner: &AccountId) {
        let mut state = self.edits.state();
        if !state.watched.insert(owner.clone()) {
            return;
        }
        let layer = self.root.path().to_path_buf();
        for tree in TREES {
            let dir = layer.join(tree);
            if let Err(error) = std::fs::create_dir_all(&dir) {
                tracing::warn!(target: TARGET, kind = ?error.kind(), "edits not watched");
                continue;
            }
            let (weak, whose) = (Arc::downgrade(self), owner.clone());
            let watched = watch_tree(&dir, move |path: &Path| {
                if let Some(core) = weak.upgrade() {
                    let shown = path
                        .strip_prefix(core.root.path())
                        .unwrap_or(path)
                        .to_string_lossy()
                        .replace('\\', "/");
                    core.edits.record(whose.clone(), shown);
                }
            });
            match watched {
                Ok(watch) => state.watches.push(watch),
                Err(error) => {
                    tracing::warn!(target: TARGET, error = error.as_str(), "edits not watched");
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
