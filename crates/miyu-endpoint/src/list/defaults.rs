//! 会话列表每项的 `default_workspace`（施工 5-14 中，网页要的）：这个会话的默认工作区，主会话是它自己的
//! `home/<账号>/workspace/<会话编号>/`，子会话、分叉出来的顺着父会话一路追到主会话（同 `session.created` 的 `workspace_from`）。
//! 头拿 `cwd` 和它比，一样的写「默认工作区」。

use std::collections::BTreeMap;

use miyu_kernel::id::{AccountId, SessionId};
use miyu_store::index::SessionIndex;
use miyu_store::root::DataRoot;

use super::Listed;

/// 一路追到主会话：`parents` 是会话到父会话。父会话不在索引里的（删了）停在它这一层；绕回来的不会有，照走过的挡一下。
pub(crate) fn main_of(
    parents: &BTreeMap<SessionId, Option<SessionId>>,
    id: &SessionId,
) -> SessionId {
    let mut at = id.clone();
    let mut seen = vec![at.clone()];
    while let Some(Some(parent)) = parents.get(&at) {
        if seen.contains(parent) {
            break;
        }
        at = parent.clone();
        seen.push(at.clone());
    }
    at
}

/// 列出来的每一项填上默认工作区：照索引这一刻的行认父会话，列出来的父会话也算。
pub(crate) fn fill(
    listed: &mut [Listed],
    index: &SessionIndex,
    root: &DataRoot,
    account: &AccountId,
) {
    let mut parents: BTreeMap<SessionId, Option<SessionId>> = index
        .rows()
        .map(|rows| rows.into_values().map(|row| (row.id, row.parent)).collect())
        .unwrap_or_default();
    for one in listed.iter() {
        parents.insert(one.id.clone(), one.parent.clone());
    }
    for one in listed {
        let main = main_of(&parents, &one.id);
        one.default_workspace = Some(
            root.session_workspace(account, &main)
                .to_string_lossy()
                .into_owned(),
        );
    }
}

#[cfg(test)]
mod tests;
