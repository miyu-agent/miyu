//! 扩展驱动进行中要人确认（施工 ACP-8，蓝图 `session/driver-ask.md`），actor 这一头：读好参数交给内核问人，记着答的那一头；
//! 内核回了（[`Action::AnswerDriver`]）照对方给的选项写成 `result` 交回去。参数写得不对的当场回原因，不进内核。

use std::collections::BTreeMap;

use serde_json::Value;

use miyu_kernel::id::{ModuleId, Seq};
use miyu_kernel::session::{Action, DriverAnswer, DriverAsked, Input};
use miyu_kernel::time::Timestamp;
use miyu_models::provider::DriverRef;

use super::Actor;
use crate::driver_ask::{Choice, answered, read};
use crate::route::Answerer;

/// 交给内核、还没回的几问：编号到对方给的选项和答的那一头。
#[derive(Debug, Default)]
pub(super) struct Asks {
    last: u64,
    waiting: BTreeMap<u64, (Vec<Choice>, Answerer)>,
}

impl Actor {
    /// 驱动 `driver` 在请求 `seen` 进行中要人确认：读好了交给内核；读不了的当场回原因。
    pub(super) fn driver_ask(
        &mut self,
        at: Timestamp,
        (seen, driver): (Seq, DriverRef),
        params: &Value,
        reply: Answerer,
    ) -> Option<Input> {
        let (asked, module) =
            match read(params, &driver.package, &driver.to_string()).and_then(|asked| {
                let module = ModuleId::parse(&driver.package).map_err(|error| error.to_string())?;
                Ok((asked, module))
            }) {
                Ok(read) => read,
                Err(why) => {
                    reply.answer(Err(why));
                    return None;
                }
            };
        let asks = &mut self.driver_asks;
        asks.last += 1;
        let token = asks.last;
        asks.waiting.insert(token, (asked.choices, reply));
        Some(Input::DriverAsked(Box::new(DriverAsked {
            at,
            seen,
            token,
            module,
            access: asked.access,
            index: asked.index,
            detail: asked.detail,
        })))
    }

    /// 内核回了编号 `token` 的那一问（[`Action::AnswerDriver`]）：照选项写好交回去。别的动作不归这里。
    pub(super) fn driver_answered(&mut self, action: Action) -> Option<Input> {
        let Action::AnswerDriver { token, answer } = action else {
            return None;
        };
        if let Some((choices, reply)) = self.driver_asks.waiting.remove(&token) {
            reply.answer(answered(&answer, &choices));
        }
        if matches!(answer, DriverAnswer::Cancelled) {
            tracing::debug!(target: crate::TARGET, token, "driver approval cancelled");
        }
        None
    }
}
