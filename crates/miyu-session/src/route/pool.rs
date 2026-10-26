//! 池里挑哪一个成员（`docs/blueprint/models.md`「怎么走」第三条第 6、7 条、第四条那张表，施工 8-8；排候选施工 8-20 起是
//! 底子的一块，两个入口共用：「钉着的」是挑的一方交的，一次性调用没有，取指针指的）。
//!
//! - 钉住：造端口时钉上一个成员：载入的、最近一条发出去了的 `model.called` 是这个池的成员的，就是它；不是的、新造的，取
//!   指针指的那个，指针加一。以后每次请求先发给它。
//! - 轮换：每次请求从指针指的那个成员起，指针加一。
//! - 照先后（施工 Q-4 上）：每次请求从第一个成员起，不动指针、不钉；限额照第一个（主力），看不看得了图照全体。
//! - 窗口放不下的（施工 Q-4 上，[`Routes::fitting`]）：会话的主请求带着内核算的用量（同压缩判线），窗口说得出、比它不大的
//!   候选不挑、出错也不换过去；一个都不剩的照上下文超长报（内核先压再来）。窗口说不出的照旧算放得下。
//! - 候选：从挑中的那个起，照写的先后绕一圈，每个成员的 key 照 `route/choice.rs` 排。这时用不了的（那一家推不出驱动、地址，
//!   地址、key 取不到）跳过；都用不了的，交第一个的原话。钉住的池，钉着的成员只在成了时换成真发的那一个（施工 8-9，
//!   `route/ended.rs`：出错换过去的、这时用不了跳过去的都一样）。
//! - 认不出的成员（那一家没配）每次解析都记一行 `WARN pool member skipped`。
//! - 指针往前走一次，在阻塞线程里写一次 `state/models/pools.json`（[`ModelData::save_pointers`]）。
//! - 限额：钉住的照钉着的那个成员。轮换的取成员里窗口最小的、最大输出最小的（说得出的里面），一张图的算法只在成员都一样时
//!   给，模型写 `none`：每次请求的模型都不一样，锚总是对不上，用量全靠本地估（第三条第 7 条，认了的）。看不看得了图两种池
//!   一样：有一个认得出的成员看不了就算看不了（施工 8-17，第十三条第 1 条）。

use std::sync::Arc;

use miyu_config::Values;
use miyu_kernel::origin::Model;
use miyu_kernel::session::Limits;
use miyu_models::facts::facts;
use miyu_models::pools::{Member, Pool, Strategy};
use miyu_models::provider::{self, NoModel, Target};

use super::base::Seat;
use super::choice::{Choice, Unsent};
use super::{Routes, images, none, nothing};
use crate::TARGET;
use crate::config::TurnConfig;
use crate::route::shared::ModelData;

impl Routes {
    /// 造端口时：钉住的池钉上哪个成员、限额是什么（见模块的说明）。`sent` 是最近一条发出去了的 `model.called` 发给了谁。
    pub(super) fn pool_start(
        &self,
        config: &TurnConfig,
        pool: &Pool,
        sent: Option<&Model>,
    ) -> (Option<Member>, Limits) {
        let values = config.resolved.values();
        match pool.strategy {
            // 照先后的（施工 Q-4 上）：主力是第一个，限额照它；不钉，出错照先后换。
            Strategy::Ordered => {
                let member = &pool.members[0];
                let mut limits = target(&self.data, &values, member)
                    .map_or_else(|_| nothing(), |target| self.limits(config, &target));
                limits.blind = self.blind(config, &values, pool);
                (None, limits)
            }
            Strategy::Pin => {
                let at = sent
                    .and_then(|sent| pool.find(sent.endpoint.as_str(), sent.model.as_str()))
                    .unwrap_or_else(|| take(&self.data, pool));
                let member = pool.members[at].clone();
                let mut limits = target(&self.data, &values, &member)
                    .map_or_else(|_| nothing(), |target| self.limits(config, &target));
                // 出错会换到别的成员：有一个看不了图就算看不了（施工 8-17）。
                limits.blind = self.blind(config, &values, pool);
                (Some(member), limits)
            }
            Strategy::Rotate => (None, self.rotating(config, &values, pool)),
        }
    }

    /// 池里有一个认得出的成员看不了图（施工 8-17）：资料的 `inputs` 没有 `image`。
    fn blind(&self, config: &TurnConfig, values: &Values, pool: &Pool) -> bool {
        pool.members.iter().any(|member| {
            target(&self.data, values, member).is_ok_and(|target| {
                let (facts, _) = self.data.with(|knowledge| {
                    facts(&config.resolved, knowledge, &target.provider, &target.model)
                });
                !facts.driver_inputs().images
            })
        })
    }

    /// 轮换的池的限额：说得出的窗口、最大输出里小的；一张图的算法成员都一样才给；有一个成员看不了图就算看不了（施工 8-17）。
    fn rotating(&self, config: &TurnConfig, values: &Values, pool: &Pool) -> Limits {
        let mut limits = nothing();
        let mut tokens = Vec::new();
        for member in &pool.members {
            let Ok(target) = target(&self.data, values, member) else {
                continue;
            };
            let (facts, _) = self.data.with(|knowledge| {
                facts(&config.resolved, knowledge, &target.provider, &target.model)
            });
            limits.window = smaller(limits.window, facts.window.value);
            limits.max_output = smaller(limits.max_output, facts.max_output.value);
            limits.blind |= !facts.driver_inputs().images;
            tokens.push(target.provider.images);
        }
        if let Some(first) = tokens.first()
            && tokens.iter().all(|each| each == first)
        {
            limits.images = images(*first);
        }
        limits.model = none();
        limits
    }

    /// 池这一次的候选：从钉着的（钉住，`seat` 交的）、指针指的（轮换，或者还没钉上的）成员起，照写的先后绕一圈，每个
    /// 用得了的成员的 key 排下来。一个都用不了的交第一个的原话。
    pub(super) fn members(
        &self,
        config: &TurnConfig,
        values: &Values,
        pool: &Pool,
        seat: &Seat<'_>,
    ) -> Result<Vec<Choice>, NoModel> {
        let data = &self.data;
        for text in &pool.skipped {
            tracing::warn!(target: TARGET, pool = pool.name.as_str(), member = text.as_str(), "pool member skipped");
        }
        let held = seat
            .held
            .filter(|_| pool.strategy == Strategy::Pin)
            .and_then(|member| pool.find(&member.provider, &member.model));
        let first = match pool.strategy {
            Strategy::Ordered => 0,
            Strategy::Pin | Strategy::Rotate => held.unwrap_or_else(|| take(data, pool)),
        };
        let mut choices = Vec::new();
        let mut problem = None;
        for at in pool.order(first) {
            let member = &pool.members[at];
            let tried = target(data, values, member)
                .and_then(|target| self.choices(config, &target, Some(member), seat));
            match tried {
                Ok(more) => choices.extend(more),
                Err(error) => {
                    problem.get_or_insert(error);
                }
            }
        }
        match (choices.is_empty(), problem) {
            (false, _) => Ok(choices),
            (true, Some(problem)) => Err(problem),
            (true, None) => Err(NoModel(format!("pool {:?} has no models", pool.name))),
        }
    }
}

impl Routes {
    /// 剔掉窗口放不下这份请求的候选（施工 Q-4 上，见模块的说明）：`used` 是内核算的用量，窗口说得出、不比它大的不要。一个都
    /// 不剩的 [`Unsent::TooLong`]，原话写用量和每个候选的窗口。
    pub(super) fn fitting(
        &self,
        config: &TurnConfig,
        choices: Vec<Choice>,
        used: u64,
    ) -> Result<Vec<Choice>, Unsent> {
        let window = |choice: &Choice| {
            let (facts, _) = self.data.with(|knowledge| {
                facts(
                    &config.resolved,
                    knowledge,
                    &choice.target.provider,
                    &choice.target.model,
                )
            });
            facts.window.value
        };
        let windows: Vec<Option<u64>> = choices.iter().map(window).collect();
        let named: Vec<String> = choices
            .iter()
            .zip(&windows)
            .map(|(choice, window)| match window {
                Some(window) => format!("{} {window}", super::choice::name(&choice.who)),
                None => super::choice::name(&choice.who),
            })
            .collect();
        let kept: Vec<Choice> = choices
            .into_iter()
            .zip(windows)
            .filter(|(_, window)| window.is_none_or(|window| used < window))
            .map(|(choice, _)| choice)
            .collect();
        if kept.is_empty() {
            return Err(Unsent::TooLong(format!(
                "request of about {used} tokens fits no candidate's window: {}",
                named.join(", ")
            )));
        }
        Ok(kept)
    }
}

/// 池里一个成员这一轮发给谁：那一家这一轮的样子、模型名。
fn target(data: &ModelData, values: &Values, member: &Member) -> Result<Target, NoModel> {
    let provider =
        data.with(|knowledge| provider::provider(values, knowledge, &member.provider))?;
    Ok(Target {
        provider,
        model: member.model.clone(),
    })
}

/// 取指针指的那个成员，指针往前走一个，在阻塞线程里写盘。
fn take(data: &Arc<ModelData>, pool: &Pool) -> usize {
    let at = data.take(&pool.name, pool.members.len());
    let saving = Arc::clone(data);
    drop(tokio::task::spawn_blocking(move || saving.save_pointers()));
    at
}

/// 两个上限里小的；没有的不算。
fn smaller(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}
