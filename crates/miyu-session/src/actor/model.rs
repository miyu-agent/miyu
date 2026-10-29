//! 请求模型的那几样（施工 3-7 下；施工 4-7 上从 `actor.rs` 挪出来，那边放不下了）：交给端口、叫停、说完了记一行
//! 收场（`28-运行日志.md` 第三节）。辅助请求（施工 3-8 四补的回顾、五补的起标题）也在这里：同一个端口，回报另走一路。
//! 说完了跟着端口的限额（施工 8-9，[`Actor::follow_limits`]）：变了交给内核，模型变了推 `model.changed`。回合开始时叫端口照
//! 这一轮的配置重新解析会话的引用（施工 8-10，[`Actor::turn_start`]）：限额变了交给内核，头看得到的变了推 `model.changed`。
//! 思考强度（施工 8-18）给头看的那一档也算头看得到的一格。回合开始还问挂接点（施工 R-4 上，`hooks`）。替看不了图的模型看图也在这里交给端口（施工 8-17）。

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::oneshot;

use miyu_kernel::event::{
    CallError, ChangeWhy, ModelChanged, Purpose, Transient, TransientBody, Usage,
};
use miyu_kernel::facts::{Present, Speaker};
use miyu_kernel::id::{ContentHash, Seq, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::request::{Difference, Request};
use miyu_kernel::session::{Injection, Input, Limits};
use miyu_kernel::time::{Timestamp, UtcOffset};

use super::{Actor, answer};
use crate::TARGET;
use crate::config::TurnConfig;
use crate::handle::Pushed;
use crate::knowledge::Shelves;
use crate::lines::{millis, where_};
use crate::memory::Calls;
use crate::port::{Cancel, Report, Reports, Sight};
use crate::route::NONE;
use crate::settings::CompactionSettings;
use crate::shown::{Next, Shown};

/// 回合开始问挂接点最多等多久（施工 R-4 上定的起点）：过了当这一轮没交。读的是本机的小库，平常几毫秒。
const HOOKS: Duration = Duration::from_millis(300);

impl Actor {
    /// 请求模型：交给端口，记下叫停它的那一头和这一刻。前缀和上一次比变了的，运行日志里写上第一处不同在哪
    /// （施工 3-9 下）：缓存没命中时，一看就知道是不是我们的前缀变了。
    pub(super) fn call(&mut self, seen: Seq, request: Request, changed: Option<Difference>) {
        let model = self.model.model();
        tracing::info!(
            target: TARGET,
            seen = seen.get(),
            endpoint = model.endpoint.as_str(),
            model = model.model.as_str(),
            changed = changed.map(|changed| where_(&changed)),
            "request"
        );
        let (stop, cancel) = oneshot::channel();
        self.calls.insert(seen, (stop, Instant::now()));
        let reports = Reports::new(seen, self.backs.clone());
        let config = self.config.current();
        self.model
            .call(seen, request, config, reports, Cancel::new(cancel));
    }

    /// 发一次辅助请求（施工 3-8 四补的回顾、五补的起标题）：交给同一个端口，回报走辅助请求那一路，名字是用途和它照到的
    /// 那一条。叫停它的那一头拿着不用：内核不叫停辅助请求，actor 停了放下它，请求跟着停。运行日志那一行前面带用途
    /// （`recap request`、`title request`）。
    pub(super) fn aside(&mut self, purpose: Purpose, upto: Seq, request: Request) {
        let model = self.model.model();
        tracing::info!(
            target: TARGET,
            seen = upto.get(),
            endpoint = model.endpoint.as_str(),
            model = model.model.as_str(),
            "{} request",
            purpose.as_str()
        );
        let (stop, cancel) = oneshot::channel();
        self.asides.retain(|(asking, ..)| *asking != purpose);
        self.asides
            .push((purpose.clone(), upto, stop, Instant::now()));
        let reports = Reports::aside(purpose, upto, self.backs.clone());
        let config = self.config.current();
        self.model
            .call(upto, request, config, reports, Cancel::new(cancel));
    }

    /// 替看不了图的模型看图（施工 8-17）：交给端口照这一轮的配置发，结果送回 `Back::Described`（`back.rs`）。叫不停：会话停了，
    /// 回来的没人收。
    pub(super) fn describe(&mut self, blob: ContentHash, request: Request) {
        let sight = Sight::new(blob, self.backs.clone());
        let config = self.config.current();
        self.model.describe(request, config, sight);
    }

    /// 不要请求 `seen` 了：叫端口停下。
    pub(super) fn cancel(&mut self, seen: Seq) {
        let Some((stop, asked)) = self.calls.remove(&seen) else {
            return;
        };
        answer(stop, ());
        tracing::info!(
            target: TARGET,
            seen = seen.get(),
            took_ms = millis(asked.elapsed()),
            "cancelled"
        );
    }

    /// 请求 `seen` 说完了：不用再叫停它了；记一行收场（`28-运行日志.md` 第三节）；跟着端口的限额（施工 8-9）。
    pub(super) fn ended(&mut self, seen: Seq, usage: Option<&Usage>, error: Option<&CallError>) {
        self.follow_limits();
        let Some((_, asked)) = self.calls.remove(&seen) else {
            return;
        };
        finished(seen, asked, usage, error, "");
    }

    /// 一次请求说完了（主请求、辅助请求都算）：端口的限额和上一次交给内核的比（施工 8-9，`models.md`「怎么走」第五条第 7
    /// 条）。变了的当场交 `Input::Limits`（不出动作），给头看的跟着换；限额里的模型变了、不是 `none` 的（轮换的池总是
    /// `none`），推一条 `model.changed`，`why` 是 `failover`。在送进说完了之前做：回合还开着，说完了的用量也照新的模型算锚。
    pub(super) fn follow_limits(&mut self) {
        let limits = self.model.limits();
        if limits == self.handed {
            return;
        }
        let moved = limits.model != self.handed.model && limits.model.endpoint.as_str() != NONE;
        self.hand(limits);
        if moved {
            self.announce(ChangeWhy::Failover);
        }
    }

    /// 回合开始（`turn.started` 已经落了盘）：冻结这一轮的配置，带上会话这时的目录的项目配置（施工 8-4）；叫端口照它重新
    /// 解析内核交来的引用 `reference`（施工 8-10，`models.md`「怎么走」第六条第 3 条）。限额变了交给内核；头看得到的（引用、
    /// 接下来发给谁、思考强度、窗口、压缩线）变了推一条 `model.changed`，`why` 是 `turn`（「施工时定的」8-10）。给头看的那一档（照新的
    /// 配置算）变了也推 `model.changed`。人格的文件改了的，带上新快照的哈希（施工 P-1 再补，`persona.rs`）。问挂接点（施工
    /// R-4 上，`hooks`），交回跑完了，带着退回了默认的那一次和挂接点交的。
    pub(super) async fn turn_start(
        &mut self,
        turn: TurnId,
        reference: Option<String>,
        present: Vec<Present>,
        said: Option<String>,
        speakers: Vec<Speaker>,
    ) -> Input {
        self.config.turn(self.session.cwd().to_string()).await;
        let before = self.shown_now();
        let replaced = self.model.turn(self.config.current(), reference.as_deref());
        self.hand(self.model.limits());
        if self.shown_now() != before {
            self.announce(ChangeWhy::Turn);
        }
        let values = self.config.current().resolved.values();
        // 私密目录照这一轮的配置（施工 5-15 下）：权限策略、沙盒都换上。
        if let Some(private) = self.guard.private_of(&values) {
            self.guard.set_private(private.clone());
            self.tools.set_private(private);
        }
        // 只禁写的名单照这一轮的配置（施工 5-17 下）：权限策略换上，文件工具写名单上的问人。
        self.guard.set_readonly(self.guard.readonly_of(&values));
        // 沙盒强度照这一轮的配置（施工 5-16 下）。
        let strength = crate::settings::SandboxSettings::from(&values).strength;
        self.tools.set_weak(strength == crate::settings::WEAK);
        let prepare = CompactionSettings::from(&values).prepare;
        // 技能照这一刻的快照先找起来，和刷新人格并着等（施工 PERF-2）：两样都在阻塞线程里读盘，原来技能排在人格后面干等。
        // 人格、预设真换了的（交回新快照的哈希，很少见），工具面跟着变，照新的再找一次。
        let speculated = self.skills_turn(&present, &values);
        let (policy, skills) = tokio::join!(self.refresh_persona(values.clone()), speculated);
        let skills = match policy {
            Some(_) => self.skills_turn(&present, &values).await,
            None => skills,
        };
        // 项目指令在阻塞线程里读盘，和挂接点、扩展一起等，不排在前面干等（施工 PERF-1）。
        let docs = self.project_docs_turn(&present);
        // 子目录的项目指令照同一个口径（施工 P-14 下）：这一轮读、改工作目录下面的文件时补。
        self.project_docs_look();
        // 主人放行出沙盒的照这一轮的配置换（施工 X-36）：权限策略、执行器各拿一份。
        let unsandboxed =
            crate::unsandboxed::Unsandboxed::of(&values).within(self.guard.confines().clone());
        self.guard.set_unsandboxed(unsandboxed.clone());
        self.tools.set_unsandboxed(unsandboxed);
        let config = Arc::clone(self.config.current());
        let knowledge = self.recalling();
        // 扩展和记忆、知识库一起问：各有各的限时，这一轮只等慢的那一个（施工 X-16）。
        let extensions = self.extension_injections(said.clone());
        let (mut injected, extended, docs) = tokio::join!(
            hooks(
                self.tools.memory(),
                knowledge,
                (present, speakers),
                said,
                config
            ),
            extensions,
            docs
        );
        let skills = self.skills_take(skills);
        injected.extend(extended);
        injected.extend(skills);
        injected.extend(docs);
        Input::TurnStartHooksDone {
            policy,
            prepare,
            at: self.clock.now(),
            turn,
            injected,
            replaced,
        }
    }

    /// 自动想起问谁（施工 K-6 下，`knowledge.md` 第九条）：会话这一刻的工具面上有 `kb_search` 的（知识库这个包装着、预设开着、
    /// 查得到至少一个库），它的知识库。
    fn recalling(&self) -> Option<Arc<Shelves>> {
        let refresh = self.persona.as_ref()?;
        let on = refresh
            .snapshot
            .tools
            .iter()
            .any(|tool| tool.name == miyu_tool::KB_SEARCH);
        refresh.knowledge.clone().filter(|_| on)
    }

    /// 限额 `limits` 变了的交给内核；给头看的照内核算的限额、端口的引用和模型写一次。
    fn hand(&mut self, mut limits: Limits) {
        // 预设路由不知道，这里补上（施工 E-M）：不用预设的 `none`。
        let pin = self
            .persona
            .as_ref()
            .and_then(|refresh| refresh.snapshot.preset.as_ref());
        crate::snapshot::stamp_preset(&mut limits, pin);
        // 命令、脚本拿到的 `MIYU_MODEL`、`MIYU_EFFORT` 照同一份（施工 E-M）。
        self.tools
            .model_in_use(limits.identity.as_ref().map(|identity| miyu_tool::InUse {
                model: identity.model.clone(),
                effort: identity.effort.clone(),
            }));
        if limits != self.handed {
            self.session.handle(Input::Limits(limits.clone()));
            self.handed = limits;
        }
        // 只换模型这两格：待办另有人换（施工 D-3）。
        let mut shown = self
            .shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        shown.limits = self.session.context_limits();
        shown.next = Next::of(&*self.model);
    }

    /// 内核推了 `todos.changed`（施工 D-3）：给头看的那一份跟着换，`subscribe` 照它答。别的瞬时事件不管。
    pub(super) fn show_todos(&self, transient: &Transient) {
        if let TransientBody::TodosChanged(changed) = &transient.body {
            let mut shown = self
                .shown
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            shown.todos = changed.todos.clone();
            shown.todos_done = changed.done.clone();
        }
    }

    /// 落了盘推出去的事件动了包的状态（施工 TW-1）：照内核的换，`session.state`、界面块照它答。
    pub(super) fn show_states(&self, events: &[miyu_kernel::event::Event]) {
        if events.iter().any(miyu_kernel::history::touches_states) {
            let states = self.session.package_states();
            self.shown
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .states = states;
        }
    }

    /// 给头看的那一份，这一刻的。
    fn shown_now(&self) -> Shown {
        self.shown
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    /// 推一条 `model.changed`，照给头看的那一份写：`by` 是内核，`turn`、`cause` 照内核这时的回合。
    fn announce(&mut self, why: ChangeWhy) {
        let Shown { limits, next, .. } = self.shown_now();
        let (turn, cause) = self
            .session
            .turn_cause()
            .map_or((None, None), |(turn, cause)| (Some(turn), cause));
        let (endpoint, model) = next.model.map_or((None, None), |model| {
            (Some(model.endpoint), Some(model.model))
        });
        let effort = next.effort;
        let at = self.clock.now();
        self.push(Pushed::Transient(Transient {
            at,
            turn,
            by: By::Kernel,
            cause,
            body: TransientBody::ModelChanged(Box::new(ModelChanged {
                reference: next.reference,
                endpoint,
                model,
                effort,
                limits,
                why,
            })),
        }));
    }

    /// 辅助请求的一样回报，写成内核的输入；说完了的先记一行收场（施工 3-8 五补从 `actor.rs` 挪来，那边放不下了）。
    pub(super) fn aside_back(
        &mut self,
        at: Timestamp,
        purpose: Purpose,
        upto: Seq,
        report: Report,
    ) -> Input {
        match report {
            Report::Sent {
                model,
                request,
                fell_back,
            } => Input::AsideSent {
                at,
                purpose,
                upto,
                model,
                request,
                fell_back,
            },
            Report::Delta(delta) => Input::AsideDelta {
                at,
                purpose,
                upto,
                delta,
            },
            Report::Ended {
                usage, cost, error, ..
            } => {
                self.follow_limits();
                self.aside_ended(&purpose, upto, usage.as_ref(), error.as_ref());
                Input::AsideEnded {
                    at,
                    purpose,
                    upto,
                    usage,
                    cost: cost.map(|cost| *cost),
                    error,
                }
            }
        }
    }

    /// 辅助请求说完了（施工 3-8 四补的回顾、五补的起标题）：记一行收场，和主请求的一样，前面带用途（`recap failed`、
    /// `title failed`……）。起标题两次都没起成就不再试，第二行 `title failed` 就是那一行。
    fn aside_ended(
        &mut self,
        purpose: &Purpose,
        upto: Seq,
        usage: Option<&Usage>,
        error: Option<&CallError>,
    ) {
        let Some(k) = self
            .asides
            .iter()
            .position(|(asking, seen, ..)| asking == purpose && *seen == upto)
        else {
            return;
        };
        let (_, _, _, asked) = self.asides.remove(k);
        finished(upto, asked, usage, error, &format!("{} ", purpose.as_str()));
    }
}

/// 一次请求收场的那一行：出错的写分类，说完了的写输入、命中、写进缓存的、输出。`what` 是主请求的空着，辅助请求的是用途加一个
/// 空格（`recap `、`title `、`compaction `）。提前压好的出错记 `WARN`（施工 6-11 上）：到线时照当场压，没人看得到它。
fn finished(
    seen: Seq,
    asked: Instant,
    usage: Option<&Usage>,
    error: Option<&CallError>,
    what: &str,
) {
    let took_ms = millis(asked.elapsed());
    match error {
        Some(error) if what == "compaction " => tracing::warn!(
            target: TARGET,
            seen = seen.get(),
            took_ms,
            class = error.class.as_str(),
            "{what}failed"
        ),
        Some(error) => tracing::info!(
            target: TARGET,
            seen = seen.get(),
            took_ms,
            class = error.class.as_str(),
            "{what}failed"
        ),
        None => tracing::info!(
            target: TARGET,
            seen = seen.get(),
            took_ms,
            "in" = usage.map(|usage| usage
                .uncached
                .saturating_add(usage.cache_read)
                .saturating_add(usage.cache_write)),
            hit = usage.map(|usage| usage.cache_read),
            write = usage.map(|usage| usage.cache_write).filter(|written| *written > 0),
            out = usage.map(|usage| usage.output),
            "{what}ended"
        ),
    }
}

/// 问回合开始的挂接点：记忆（施工 R-4 上，`memory.md` 第三条；`memory` 是它的端口和会话的时区）交常驻的摘要和联想（施工
/// R-8，第四条），知识库（施工 K-6 下，`knowledge.md` 第九条；`knowledge` 是会话的知识库）交自动想起。`said` 是这一轮人说的话，
/// 不是人开的一轮没有；照意思找照这一轮的配置 `config`；`present` 是内核交来的这一段上下文里模块注入过的。两个挂接点并行问，
/// 照模块编号排（知识库在记忆前面）；各自限时 [`HOOKS`]：过了的那一件当这一轮没交，记一行 `DEBUG`（那次读在阻塞线程里照样
/// 读完，交回的丢掉）。
async fn hooks(
    memory: Option<(Calls, UtcOffset)>,
    knowledge: Option<Arc<Shelves>>,
    (present, speakers): (Vec<Present>, Vec<Speaker>),
    said: Option<String>,
    config: TurnConfig,
) -> Vec<Injection> {
    let known = async {
        let (shelves, said) = (knowledge?, said.as_deref()?);
        let using = shelves.using(&config);
        timed(
            "knowledge",
            "recall",
            shelves.recall(&using, said, &present),
        )
        .await
    };
    let remembered = remember(
        memory,
        (present.clone(), speakers),
        said.clone(),
        Arc::clone(&config),
    );
    let (known, remembered) = tokio::join!(known, remembered);
    known.into_iter().chain(remembered).collect()
}

/// 记忆那一个挂接点：先问摘要、和她说话的那个人的卡（施工 E-1 补，照这一轮开口的人 `speakers`），再问联想（联想要知道这一轮
/// 摘要列了哪几条，不重复带）。照这个先后排：摘要、卡、联想。
async fn remember(
    memory: Option<(Calls, UtcOffset)>,
    (present, speakers): (Vec<Present>, Vec<Speaker>),
    said: Option<String>,
    config: TurnConfig,
) -> Vec<Injection> {
    let Some((memory, offset)) = memory else {
        return Vec::new();
    };
    let (summary, card) = tokio::join!(
        timed("memory", "summary", memory.summary(offset, present.clone())),
        timed("memory", "card", memory.card(speakers, present.clone())),
    );
    let summary: Vec<Injection> = summary.into_iter().chain(card).collect();
    let Some(said) = said else {
        return summary;
    };
    // 这一轮摘要列的几条，联想当已经交过的。
    let mut seen = present;
    seen.extend(summary.iter().map(|injection| Present {
        module: injection.module.clone(),
        kind: injection.fact.kind.clone(),
        refs: injection.fact.refs.clone(),
    }));
    let recall = timed(
        "memory",
        "recall",
        memory.recall(said, offset, seen, &config),
    )
    .await;
    summary.into_iter().chain(recall).collect()
}

/// 限时 [`HOOKS`] 等挂接点的一件（`module` 是哪个挂接点，`part`：`summary`、`card`、`recall`）；过了的没有，记一行 `DEBUG`。
async fn timed(
    module: &'static str,
    part: &'static str,
    asked: impl Future<Output = Option<Injection>>,
) -> Option<Injection> {
    match tokio::time::timeout(HOOKS, asked).await {
        Ok(injected) => injected,
        Err(_) => {
            tracing::debug!(target: TARGET, module, part, "turn start hook timed out");
            None
        }
    }
}
