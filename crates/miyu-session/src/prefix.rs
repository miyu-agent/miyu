//! 文件改动的版本（施工 OV-1 下，`docs/construction/OV-1-同名覆盖工具说明和提示词.md` 第十节）：核心盯着数据根里的覆盖、人格、
//! 预设那几棵树（技能不算：那一块钉在快照里，施工 PT-2 上），变一下加一；会话记着它最后一次拼快照时看到的号，回合开始只比号，变了照现在的文件重拼。号由协议端点给
//! （[`EditsVersion`]），只在内存里。
//!
//! 同一份 [`Prefix`] 会话的 actor 和 [`crate::Handle`] 共用：端点照它判「前缀要变了」要不要先问人（`prefix_changing`），人放行
//! 过的那一批记在 [`Prefix::allow`]。

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// 取这时的号：协议端点给的，没交的（测试里造的）当一直是零。
#[derive(Clone)]
pub struct EditsVersion(pub Arc<dyn Fn() -> u64 + Send + Sync>);

impl std::fmt::Debug for EditsVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("EditsVersion")
    }
}

/// 一个会话看到的号、放行过的号。
#[derive(Debug, Default)]
pub struct Prefix {
    seen: AtomicU64,
    allowed: AtomicU64,
}

impl Prefix {
    /// 造会话、载入时看到的是 `seen`。
    pub fn new(seen: u64) -> Prefix {
        Prefix {
            seen: AtomicU64::new(seen),
            allowed: AtomicU64::new(seen),
        }
    }

    /// 最后一次拼快照时看到的号。
    pub fn seen(&self) -> u64 {
        self.seen.load(Ordering::SeqCst)
    }

    /// 照号 `version` 拼过了。
    pub(crate) fn see(&self, version: u64) {
        self.seen.fetch_max(version, Ordering::SeqCst);
    }

    /// 快照不跟着文件换的会话（整理记忆的、fork 子代理的）：前缀不会因为文件变，从不问。
    pub(crate) fn hold(&self) {
        self.see(u64::MAX);
    }

    /// 人放行到哪一个号了（「直接发送」「压缩上下文并发送」）。
    pub fn allowed(&self) -> u64 {
        self.allowed.load(Ordering::SeqCst)
    }

    /// 记下人放行到号 `version`。
    pub fn allow(&self, version: u64) {
        self.allowed.fetch_max(version, Ordering::SeqCst);
    }
}
