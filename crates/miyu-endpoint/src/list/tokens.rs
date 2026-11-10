//! 会话列表的 `tokens` 一列（终端要的）：这个会话连同它派出去的子代理（一层层往下）一共用了多少 token，和终端底栏的 `Σ` 同一个
//! 算法。每个会话自己的记在索引那一行（[`miyu_store::index::Row::tokens`]），这里照父会话一层层加上去。

use std::collections::{BTreeMap, BTreeSet};

use miyu_kernel::id::SessionId;
use miyu_store::index::Link;

use super::Listed;

/// 谁是谁的子会话：照 `links` 里的父会话排一次，列出来的每一项共用（施工 YC-13：以前每一项各排一遍，列全部时是会话数的
/// 平方）。
pub(crate) struct Families<'a> {
    links: &'a BTreeMap<SessionId, Link>,
    children: BTreeMap<&'a SessionId, Vec<&'a SessionId>>,
}

impl<'a> Families<'a> {
    /// 照 `links` 认子孙。
    pub(crate) fn new(links: &'a BTreeMap<SessionId, Link>) -> Families<'a> {
        let mut children: BTreeMap<&SessionId, Vec<&SessionId>> = BTreeMap::new();
        for (id, link) in links {
            if let Some(parent) = &link.parent {
                children.entry(parent).or_default().push(id);
            }
        }
        Families { links, children }
    }

    /// `of` 连同它的子孙会话的 token。`links` 里没有 `of` 的是 0。父子关系成了环的（索引坏了才会有），走过的不再走
    /// （施工 B-4）；加起来封顶不溢出。
    pub(crate) fn total(&self, of: &SessionId) -> u64 {
        let mut total: u64 = 0;
        let mut seen = BTreeSet::new();
        let mut pending = vec![of];
        while let Some(id) = pending.pop() {
            if !seen.insert(id) {
                continue;
            }
            total = total.saturating_add(self.links.get(id).map_or(0, |link| link.tokens));
            if let Some(below) = self.children.get(id) {
                pending.extend(below);
            }
        }
        total
    }
}

/// 列出来的每一项换成连同子孙的：照索引这一刻的行（列的时候刚补好）。
pub(crate) fn fill(listed: &mut [Listed], links: &BTreeMap<SessionId, Link>) {
    let families = Families::new(links);
    for one in listed {
        one.tokens = one.tokens.max(families.total(&one.id));
    }
}
