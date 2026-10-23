//! 子目录的项目指令（施工 P-14 下，`docs/blueprint/project-docs.md`「子目录的」）：她第一次读、改工作目录下面某个子目录里的
//! 文件时，到那个文件所在目录为止、工作目录以下每一层的 `AGENTS.md`（没有的找 `CLAUDE.md`），这一段上下文里还没交过的，接在
//! 那次工具结果后面一块交给她（照 Claude Code）。回合开始交的是仓库根到工作目录的那几层（P-14 上），不重。
//!
//! - 谁读：同回合开始那一块，本机的会话、预设没关 `project-docs` 的（回合开始照会话定下 [`Look`]）。
//! - 交过的：执行器记着每份文件交过的内容指纹，一样的不再交、变了的再交一份；压缩、撤销、恢复以后整份清掉重来（交过的那几块
//!   可能已经不在她的上下文里了）。重启以后也从空的起，最多多交一次。
//! - 每次补交单算 32 KiB 的上限（同回合开始那一块），超了截在字的边界上、末尾说一句。

use std::collections::BTreeMap;
use std::path::PathBuf;

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, Effect, Event};
use miyu_kernel::id::ContentHash;
use miyu_store::resources::{ProjectDocTexts, ResourceRoot};

use crate::project_docs::{self, Found, LIMIT};

#[cfg(test)]
mod tests;

/// 这一轮找不找、照哪个工作目录找（真实的路径）、给她的字从哪读。
#[derive(Debug, Clone)]
pub(crate) struct Look {
    pub(crate) cwd: PathBuf,
    pub(crate) resources: ResourceRoot,
}

/// 一次工具结果里碰到的子目录的那几份：找到的文件和字，和外壳要的字。
#[derive(Debug)]
pub(crate) struct Below {
    texts: ProjectDocTexts,
    docs: Vec<(PathBuf, String)>,
}

/// 执行器手里的：这一轮的 [`Look`]，交过的每份文件的内容指纹。
#[derive(Debug, Default)]
pub(super) struct Docs {
    look: Option<Look>,
    told: BTreeMap<PathBuf, ContentHash>,
}

impl Docs {
    /// 回合开始照会话定下这一轮找不找。
    pub(super) fn look(&mut self, look: Option<Look>) {
        self.look = look;
    }

    /// 派一次调用时带走的：这一轮要找的话交一份。
    pub(super) fn for_call(&self) -> Option<Look> {
        self.look.clone()
    }

    /// 一批落了盘的事件里有压缩、撤销、恢复的，交过的整份清掉。
    pub(super) fn note(&mut self, events: &[Event]) {
        if events.iter().any(|event| {
            matches!(
                event.body,
                Body::ContextCompacted(_) | Body::TurnReverted(_) | Body::TurnUnreverted(_)
            )
        }) {
            self.told.clear();
        }
    }

    /// 结果送回来时：没交过的、变了的几份包成一块，记下交过；都交过的没有。
    pub(super) fn tell(&mut self, below: Option<Below>) -> Option<Block> {
        let below = below?;
        let fresh: Vec<(PathBuf, String)> = below
            .docs
            .into_iter()
            .filter(|(path, text)| self.told.get(path) != Some(&ContentHash::of(text.as_bytes())))
            .collect();
        if fresh.is_empty() {
            return None;
        }
        let (mut docs, mut left, mut cut) = (Vec::new(), LIMIT, false);
        for (path, text) in fresh {
            self.told
                .insert(path.clone(), ContentHash::of(text.as_bytes()));
            if cut {
                continue;
            }
            if text.len() > left {
                docs.push((path, project_docs::cut_at(&text, left).to_string()));
                cut = true;
                continue;
            }
            left -= text.len();
            docs.push((path, text));
        }
        let found = Found {
            root: PathBuf::new(),
            docs,
            cut,
        };
        Some(Block::Text(Text {
            text: project_docs::render(&below.texts, &found),
        }))
    }
}

/// 工具报的效果里读过、改过的文件，照 `look` 找它们上面子目录的项目指令；一份都没有的没有。读不了字的当没有。碰磁盘，在
/// 存效果的那个阻塞线程里调。
pub(super) fn below(look: Option<&Look>, effects: &[Effect]) -> Option<Below> {
    let look = look?;
    // 效果里的是真实的路径：工作目录也换成真实的（macOS 的 `/tmp` 是链接）。
    let cwd = std::fs::canonicalize(&look.cwd).unwrap_or_else(|_| look.cwd.clone());
    let mut docs: Vec<(PathBuf, String)> = Vec::new();
    for effect in effects {
        let path = match effect {
            Effect::FileRead(read) if read.skill.is_none() => &read.path,
            Effect::FileChanged(changed) => &changed.path,
            _ => continue,
        };
        for (doc, text) in project_docs::below(&cwd, std::path::Path::new(path)) {
            if !docs.iter().any(|(known, _)| *known == doc) {
                docs.push((doc, text));
            }
        }
    }
    if docs.is_empty() {
        return None;
    }
    let texts = look.resources.project_doc_texts().ok()?;
    project_docs::check(&texts).ok()?;
    Some(Below { texts, docs })
}
