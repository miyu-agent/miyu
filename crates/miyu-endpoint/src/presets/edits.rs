//! 文件改动的版本（施工 OV-1 下，`docs/construction/OV-1-同名覆盖工具说明和提示词.md` 第十节）：盯每个账号两层（系统区、他的
//! 家目录）的 `tools/`、`prompts/`、`personas/`、`presets/`，往下递归（`skills/` 施工 PT-2 上拿掉了：技能那一块钉在快照里，
//! 中途变了走尾巴注入，system 不变、前缀不变，问人是误报）；变一下记一条：第几下、谁的（系统区的是大家的）、
//! 照数据根写的相对路径。会话回合开始只比号（`miyu_session::prefix`），发送前照它问人「前缀要变了」（`prefix_changing`）。
//!
//! - 只在内存里：核心重启从零数，会话也从零记，不会误判。
//! - 还没有的目录登记时建空的：这几个本来就是给人放自定义的地方。
//! - 每个账号用到时才盯（开会话、载入时，[`Core::watch_edits`]）；系统区的头一回登记时一起盯。
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
    /// 谁的：系统区的是 `None`，大家都算。
    whose: Option<AccountId>,
    /// 照数据根写的相对路径。
    path: String,
}

/// 改动的表。
#[derive(Default)]
struct State {
    /// 一共几下。
    seq: u64,
    /// 每一层最后一下的号：系统区的是 `None`。
    latest: BTreeMap<Option<AccountId>, u64>,
    /// 最近的几条。
    log: VecDeque<Change>,
    /// 盯上了的层。
    watched: BTreeSet<Option<AccountId>>,
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

    /// 账号 `owner` 这时的号：他那一层、系统区的最后一下，取大的。
    pub(crate) fn version(&self, owner: &AccountId) -> u64 {
        let state = self.state();
        let mine = state.latest.get(&Some(owner.clone())).copied().unwrap_or(0);
        let system = state.latest.get(&None).copied().unwrap_or(0);
        mine.max(system)
    }

    /// 账号 `owner` 号 `after` 以后改了的文件，照先后、不重；最多 `most` 个，多的交回还有几个。早就挪出表的那些也算进「还有」。
    pub(crate) fn since(&self, owner: &AccountId, after: u64, most: usize) -> (Vec<String>, usize) {
        let state = self.state();
        let mut files: Vec<String> = Vec::new();
        for change in state.log.iter().filter(|change| {
            change.seq > after && change.whose.as_ref().is_none_or(|whose| whose == owner)
        }) {
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

    /// 记一下：`whose` 那一层的 `path` 变了。
    fn record(&self, whose: Option<AccountId>, path: String) {
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

    /// 盯上账号 `owner` 那一层和系统区（盯过的不再盯）：开会话、载入时调。
    pub(crate) fn watch_edits(self: &Arc<Self>, owner: &AccountId) {
        for whose in [None, Some(owner.clone())] {
            let mut state = self.edits.state();
            if !state.watched.insert(whose.clone()) {
                continue;
            }
            let layer = match &whose {
                None => self.root.system(),
                Some(account) => self.root.account_dir(account),
            };
            for tree in TREES {
                let dir = layer.join(tree);
                if let Err(error) = std::fs::create_dir_all(&dir) {
                    tracing::warn!(target: TARGET, kind = ?error.kind(), "edits not watched");
                    continue;
                }
                let (weak, whose) = (Arc::downgrade(self), whose.clone());
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
}

#[cfg(test)]
mod tests;
