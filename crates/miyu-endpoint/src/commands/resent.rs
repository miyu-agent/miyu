//! `command.run` 重发先查编号（施工 YC-7补，`docs/blueprint/protocol.md`「`command.run`」第 7 条）。
//!
//! 斜杠命令先做事（`/loop off` 删循环、换工作目录这些）、再把 `<编号>/ran` 交给内核记：只靠内核照编号去重，重发的那一条
//! 已经又做了一遍。所以协议这一层自己记最近执行了的 `command.run`：会话、编号、参数的指纹、当时的回应。同一个编号再来，
//! 参数一样的照当时的回应答、不再执行，不一样的拒 `id_reused`、不执行。
//!
//! 只在内存里，最多 [`CAPACITY`] 个：核心重启以后、挤掉了的编号再来，照老规矩（命令再跑一遍，内核照编号去重它交的那几条）。
//! 被拒的不记：再发，重新判。

use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::Value;

use miyu_kernel::id::CommandId;

use crate::refusal::Refusal;

/// 最多记几个编号：同内核每个会话记的个数（`miyu-kernel` 的 `session/recent.rs`），这里所有会话合着算。
const CAPACITY: usize = 1024;

/// 哪个会话（照请求里写的原样）的哪个编号。
type Key = (String, CommandId);

/// 一个编号的那一格：空着的是正在执行、还没执行成的；执行了的记着参数的指纹和回应。锁着的时候是有一条正在执行，同一个
/// 编号后到的等它完。
type Slot = Arc<tokio::sync::Mutex<Option<(u64, Value)>>>;

/// 最近执行了的 `command.run`。
#[derive(Default)]
pub(crate) struct Resent {
    seen: Mutex<Seen>,
}

#[derive(Default)]
struct Seen {
    /// 编号到它那一格。
    slots: HashMap<Key, Slot>,
    /// 先后，最早的在前，满了就从这头丢。
    order: VecDeque<Key>,
}

impl Resent {
    /// 会话 `session` 的编号 `id` 只执行一次：没执行过的跑 `run`，成了记下回应；执行过、指纹一样的交回当时的回应，不跑。
    /// 同一个编号同时来两条：后到的等先到的完，再照这几条判。
    ///
    /// # Errors
    ///
    /// 执行过、指纹不一样的 [`Refusal::ID_REUSED`]，不跑；`run` 拒的照原样，不记。
    pub(crate) async fn once<F>(
        &self,
        session: &str,
        id: &CommandId,
        digest: u64,
        run: F,
    ) -> Result<Value, Refusal>
    where
        F: Future<Output = Result<Value, Refusal>>,
    {
        let key = (session.to_string(), id.clone());
        let slot = self.slot(&key);
        let mut held = slot.lock().await;
        match &*held {
            Some((seen, reply)) if *seen == digest => return Ok(reply.clone()),
            Some(_) => return Err(Refusal::ID_REUSED),
            None => {}
        }
        let outcome = run.await;
        match &outcome {
            Ok(reply) => *held = Some((digest, reply.clone())),
            Err(_) => self.forget(&key, &slot),
        }
        outcome
    }

    /// 这个编号的那一格，没有的开一格空的。满了，丢掉最早的。
    fn slot(&self, key: &Key) -> Slot {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(slot) = seen.slots.get(key) {
            return Arc::clone(slot);
        }
        let slot = Slot::default();
        seen.slots.insert(key.clone(), Arc::clone(&slot));
        seen.order.push_back(key.clone());
        while seen.order.len() > CAPACITY {
            if let Some(oldest) = seen.order.pop_front() {
                seen.slots.remove(&oldest);
            }
        }
        slot
    }

    /// 被拒的不占地方：拿掉这一格。已经换成别的一格的（挤掉以后又来的）不动。
    fn forget(&self, key: &Key, slot: &Slot) {
        let mut seen = self.seen.lock().unwrap_or_else(PoisonError::into_inner);
        if seen
            .slots
            .get(key)
            .is_some_and(|now| Arc::ptr_eq(now, slot))
        {
            seen.slots.remove(key);
            seen.order.retain(|other| other != key);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn id(text: &str) -> CommandId {
        CommandId::parse(text).expect("合写法")
    }

    #[tokio::test]
    async fn a_refused_run_is_judged_again_and_an_accepted_one_is_replayed() {
        let resent = Resent::default();
        let refused = resent
            .once("s", &id("k1"), 1, async { Err(Refusal::BAD_PARAMS) })
            .await;
        assert!(refused.is_err());
        // 被拒的不记：换了参数再来照样跑。
        let ran = resent.once("s", &id("k1"), 2, async { Ok(json!(1)) }).await;
        assert_eq!(ran.ok(), Some(json!(1)));
        let again = resent
            .once("s", &id("k1"), 2, async { panic!("不该再跑") })
            .await;
        assert_eq!(again.ok(), Some(json!(1)));
        let changed = resent
            .once("s", &id("k1"), 3, async { panic!("不该跑") })
            .await;
        assert_eq!(
            changed.err().map(|r| r.reason),
            Some(Refusal::ID_REUSED.reason)
        );
        // 别的会话的同一个编号是另一条。
        let other = resent.once("t", &id("k1"), 3, async { Ok(json!(2)) }).await;
        assert_eq!(other.ok(), Some(json!(2)));
    }

    #[tokio::test]
    async fn only_the_newest_are_kept() {
        let resent = Resent::default();
        for n in 0..=CAPACITY {
            let ran = resent
                .once("s", &id(&format!("k{n}")), 1, async { Ok(json!(n)) })
                .await;
            assert!(ran.is_ok());
        }
        // 最早的挤掉了：再来当新的。
        let first = resent
            .once("s", &id("k0"), 9, async { Ok(json!("新")) })
            .await;
        assert_eq!(first.ok(), Some(json!("新")));
        let last = resent
            .once("s", &id(&format!("k{CAPACITY}")), 1, async {
                panic!("不该再跑")
            })
            .await;
        assert_eq!(last.ok(), Some(json!(CAPACITY)));
    }
}
