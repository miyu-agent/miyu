//! 一次性入口照登记的档位发旁路请求（施工 Q-2，设计 `43-旁路请求和档位池.md` 第三节第 3 条，`miyu_models::side`；从 `once.rs`
//! 拆出来守 500 行上限）：整理记忆走这里，会话端口的那一条在 `route/side.rs`。

use miyu_kernel::event::ErrorClass;
use miyu_models::side::{self, Side};
use miyu_store::blob::Blobs;

use super::{Answer, Ask, OneShot, Unanswered, Watched};
use crate::TARGET;
use crate::config::TurnConfig;

impl OneShot {
    /// 照登记 `side` 的档位发一次旁路请求（施工 Q-2，设计 43 第三节第 3 条，`miyu_models::side`）：`ask` 的 `model` 不看。
    /// 档位池是空的，照 `fallback`（请求所属那个会话这一刻的引用；没有的照这一刻的 `models.chat`）发；池里换遍了还没答成
    /// （全在冷却、限速、可重试、认证失败，而且还没给出一个字），退到 `fallback` 再发一次，记一行 `INFO side fallback`。
    /// `fallback` 就是这个池的，不再退。
    ///
    /// # Errors
    ///
    /// 同 [`OneShot::call`]。
    pub async fn call_side(
        &self,
        config: &TurnConfig,
        blobs: &Blobs,
        ask: Ask,
        side: &Side,
        fallback: Option<String>,
    ) -> Result<Answer, Unanswered> {
        self.call_side_watching(config, blobs, ask, (side, fallback), &mut |_| {})
            .await
    }

    /// 同 [`OneShot::call_side`]，正文边收边交给 `watch`（[`OneShot::call_watching`]）；退到会话的模型时先交一个
    /// `Watched::Restart`。`side` 是登记和后备的引用。
    ///
    /// # Errors
    ///
    /// 同 [`OneShot::call`]。
    pub async fn call_side_watching(
        &self,
        config: &TurnConfig,
        blobs: &Blobs,
        mut ask: Ask,
        (side, fallback): (&Side, Option<String>),
        watch: &mut (dyn FnMut(Watched<'_>) + Send),
    ) -> Result<Answer, Unanswered> {
        let pool = side::reference(side);
        if side::resolved(&config.resolved.values(), side).is_none() {
            ask.model = fallback;
            return self.call_watching(config, blobs, ask, watch).await;
        }
        let first = Ask {
            model: Some(pool.clone()),
            ..ask.clone()
        };
        let mut said = false;
        let answered = self
            .call_watching(config, blobs, first, &mut |watched| {
                said |= matches!(watched, Watched::Text(_));
                watch(watched);
            })
            .await;
        match answered {
            Err(unanswered)
                if !said && falls_back(&unanswered) && fallback.as_deref() != Some(&pool) =>
            {
                tracing::info!(
                    target: TARGET,
                    side = side.id,
                    reason = unanswered.reason(),
                    "side fallback"
                );
                watch(Watched::Restart);
                ask.model = fallback;
                self.call_watching(config, blobs, ask, watch).await
            }
            other => other,
        }
    }
}

/// 旁路请求换遍了档位池还没答成、可以退到会话的模型的（施工 Q-2）：全在冷却、没有能用的成员，或者出的是池里会换人的错
/// （限速、可重试、认证失败）。写错了引用的、别的错不退。
fn falls_back(unanswered: &Unanswered) -> bool {
    match unanswered {
        Unanswered::Cooling { .. } | Unanswered::NoModel(_) => true,
        Unanswered::Failed(error) => matches!(
            error.class,
            ErrorClass::RateLimited | ErrorClass::Retryable | ErrorClass::Auth
        ),
        Unanswered::UnknownModel(_) => false,
    }
}
