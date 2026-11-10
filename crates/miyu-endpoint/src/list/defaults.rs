//! 会话列表每项的 `default_workspace`（施工 5-14 中，网页要的）：这个会话的默认工作区，主会话是它自己的
//! `workspace/<会话编号>/`，子会话、分叉出来的顺着父会话一路追到主会话（同 `session.created` 的 `workspace_from`）。
//! 头拿 `cwd` 和它比，一样的写「默认工作区」。

use std::collections::BTreeMap;

use miyu_kernel::id::SessionId;
use miyu_store::index::Link;
use miyu_store::root::DataRoot;

use super::Listed;

/// 一路追到主会话：`parent_of` 交回一个会话的父会话，不认识它的交回空的。父会话不认识的（删了）停在它这一层；绕回来的
/// 不会有，照走过的挡一下。
pub(crate) fn main_of(
    parent_of: impl Fn(&SessionId) -> Option<Option<SessionId>>,
    id: &SessionId,
) -> SessionId {
    let mut at = id.clone();
    let mut seen = vec![at.clone()];
    while let Some(Some(parent)) = parent_of(&at) {
        if seen.contains(&parent) {
            break;
        }
        seen.push(parent.clone());
        at = parent;
    }
    at
}

/// 列出来的每一项填上默认工作区：照索引这一刻的行认父会话，列出来的父会话也算（先照列出来的）。只顺着列出来的往上追，
/// 不把整张表抄一份（施工 YC-13）。
pub(crate) fn fill(listed: &mut [Listed], links: &BTreeMap<SessionId, Link>, root: &DataRoot) {
    let own: BTreeMap<SessionId, Option<SessionId>> = listed
        .iter()
        .map(|one| (one.id.clone(), one.parent.clone()))
        .collect();
    let parent_of = |id: &SessionId| {
        own.get(id)
            .cloned()
            .or_else(|| links.get(id).map(|link| link.parent.clone()))
    };
    for one in listed {
        let main = main_of(parent_of, &one.id);
        one.default_workspace = Some(root.session_workspace(&main).to_string_lossy().into_owned());
    }
}

#[cfg(test)]
mod tests;
