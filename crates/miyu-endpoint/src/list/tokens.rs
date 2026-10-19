//! 会话列表的 `tokens` 一列（终端要的）：这个会话连同它派出去的子代理（一层层往下）一共用了多少 token，和终端底栏的 `Σ` 同一个
//! 算法。每个会话自己的记在索引那一行（[`Row::tokens`]），这里照父会话一层层加上去。

use std::collections::BTreeMap;

use miyu_kernel::id::SessionId;
use miyu_store::index::{Row, SessionIndex};

use super::Listed;

/// `of` 连同它的子孙会话的 token：照 `rows` 里的父会话认子孙。`rows` 里没有 `of` 的是 0。
pub(crate) fn family(rows: &BTreeMap<SessionId, Row>, of: &SessionId) -> u64 {
    let mut children: BTreeMap<&SessionId, Vec<&SessionId>> = BTreeMap::new();
    for row in rows.values() {
        if let Some(parent) = &row.parent {
            children.entry(parent).or_default().push(&row.id);
        }
    }
    let mut total = 0;
    let mut pending = vec![of];
    while let Some(id) = pending.pop() {
        total += rows.get(id).map_or(0, |row| row.tokens);
        if let Some(below) = children.get(id) {
            pending.extend(below);
        }
    }
    total
}

/// 列出来的每一项换成连同子孙的：照索引这一刻的行（列的时候刚补好）。索引读不了的照各自的。
pub(crate) fn fill(listed: &mut [Listed], index: &SessionIndex) {
    let Ok(rows) = index.rows() else {
        return;
    };
    for one in listed {
        one.tokens = one.tokens.max(family(&rows, &one.id));
    }
}
