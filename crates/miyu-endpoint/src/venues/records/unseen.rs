//! 之前的记录里哪几条写引用的内容（施工 QT-1，[`miyu_assemble::group::Picked::unseen`]）：带着引用的内容、它以前的日志里没有被
//! 引用的那一条的。边读边记（`pass.rs`）和记录窗（`recent.rs`）照先后收每一条的时候，手里的「编号的哈希 → 最后一条的序号」
//! 正好是它以前的，当场判下来记着，问的时候不再往回找：
//!
//! - 哈希没记着：从没挤掉过编号的，它以前没有（写）；挤掉过的说不准。
//! - 记着、又记在补拉回来的那几条的哈希里：说不准（补拉回来的不算「有」，主会话 2026-11-14 定；同一个编号别处也可能有）。
//! - 记着：它以前有（不写）。不再照序号读回那一条对编号：64 位的哈希撞上的话只是少写一段引用的内容（同 QT-1 以前），不值得
//!   为它让判官每问一次都读日志。
//!
//! 说不准的，问的一方照整份读（`records_in` 照有效历史一条条比）。

use std::collections::{HashMap, HashSet};

use miyu_assemble::group::quoting;
use miyu_kernel::event::Event;
use miyu_kernel::id::Seq;

use crate::sweep::bones::hash;

/// 一条的引用的内容怎么写。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Quoted {
    /// 不带引用的内容。
    Plain,
    /// 它以前没有被引用的那一条：写。
    Write,
    /// 它以前有：不写。
    Seen,
    /// 说不准（编号挤掉过）。
    Unsure,
}

/// `event` 这一条怎么写：`quotes` 是它以前的编号的哈希 → 最后一条的序号，`backfilled` 是它以前补拉回来的那几条的编号的哈希，
/// `evicted` 是挤掉过。
pub(super) fn judged(
    event: &Event,
    quotes: &HashMap<u64, Seq>,
    backfilled: &HashSet<u64>,
    evicted: bool,
) -> Quoted {
    let Some(reply_to) = quoting(event) else {
        return Quoted::Plain;
    };
    let key = hash(reply_to);
    match (quotes.contains_key(&key), evicted) {
        (true, _) if backfilled.contains(&key) => Quoted::Unsure,
        (true, _) => Quoted::Seen,
        (false, true) => Quoted::Unsure,
        (false, false) => Quoted::Write,
    }
}
