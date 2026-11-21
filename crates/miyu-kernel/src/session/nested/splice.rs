//! 送完一条输入时，把父级结束时记下的「已取消」放进追加的那一批（施工 M-1，蓝图 `kernel/session.md`「嵌套调用」第 7 条）。
//! 从 `nested.rs` 挪来（施工 LW-8：那个文件到了行数上限）。

use super::super::Session;
use super::super::action::Action;
use super::NestedAction;

impl Session {
    /// 送完一条输入：父级结束时记下的「已取消」放进追加的那一批，排在父级那一条前面；要掐的接在最后。
    ///
    /// # Panics
    ///
    /// 记下的「已取消」后面那一条不在这一次的动作里：造事件的一处没把父级那一条交出去，是内核的 bug。
    pub(in crate::session) fn splice_nested(&mut self, actions: &mut Vec<Action>) {
        // 同 [`Self::dispatch_nested`]：没有记下的什么都不用做（施工 LW-8）。
        if self.nested.flushed.is_empty() && self.nested.stops.is_empty() {
            return;
        }
        crate::apart::sync(|| self.splice_flushed(actions));
    }

    /// [`Self::splice_nested`] 有记下的时：插进去、接上要掐的。
    ///
    /// # Panics
    ///
    /// 同 [`Self::splice_nested`]。
    fn splice_flushed(&mut self, actions: &mut Vec<Action>) {
        let mut flushed = std::mem::take(&mut self.nested.flushed);
        flushed.sort_by_key(|event| event.seq);
        // 从后往前插：每一条插在序号紧接着它的那一条前面，那一条也可能是刚插进去的「已取消」。
        for event in flushed.into_iter().rev() {
            let after = event.seq.next();
            let placed = actions.iter_mut().find_map(|action| match action {
                Action::Append(events) => events
                    .iter()
                    .position(|later| later.seq == after)
                    .map(|k| (events, k)),
                _ => None,
            });
            let Some((events, k)) = placed else {
                panic!(
                    "nested call {} was cancelled but its parent's event is not appended, a kernel bug",
                    event.seq
                );
            };
            events.insert(k, event);
        }
        actions.extend(
            std::mem::take(&mut self.nested.stops)
                .into_iter()
                .map(|id| Action::Nested(NestedAction::Cancel { id })),
        );
    }
}
