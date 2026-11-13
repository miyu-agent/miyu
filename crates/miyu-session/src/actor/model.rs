//! 请求模型的那几样（施工 3-7 下；施工 4-7 上从 `actor.rs` 挪出来，那边放不下了）：交给端口、叫停、说完了记一行
//! 收场（`28-运行日志.md` 第三节）。辅助请求（施工 3-8 四补的回顾、五补的起标题）也在这里：同一个端口，回报另走一路。
//! 说完了跟着端口的限额（施工 8-9，[`Actor::follow_limits`]）：变了交给内核，模型变了推 `model.changed`。回合开始时叫端口照
//! 这一轮的配置重新解析会话的引用（施工 8-10，[`Actor::turn_start`]）：限额变了交给内核，头看得到的变了推 `model.changed`。
//! 思考强度（施工 8-18）给头看的那一档也算头看得到的一格。回合开始还问挂接点（施工 R-4 上，`hooks`）。替看不了图的模型看图也在这里交给端口（施工 8-17）。

use std::sync::Arc;
use std::time::Instant;

use tokio::sync::oneshot;

use miyu_kernel::event::{
    CallError, ChangeWhy, ModelChanged, Purpose, Transient, TransientBody, Usage,
};
use miyu_kernel::facts::{Present, Speaker};
use miyu_kernel::id::{ContentHash, Seq, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::request::{Difference, Request};
use miyu_kernel::session::{Input, Limits};
use miyu_kernel::time::Timestamp;

use super::{Actor, answer};
use crate::TARGET;
use crate::handle::Pushed;
use crate::knowledge::Shelves;
use crate::lines::{millis, where_};
use crate::port::{Cancel, Report, Reports, Sight};
use crate::route::NONE;
use crate::settings::CompactionSettings;
use crate::shown::{Next, Shown};
use crate::timing::turn::TurnStart;

mod hooks;
mod line;
mod overdue;

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
        let reports = Reports::new(seen, self.backs.clone()).timed(self.asked.take());
        let config = self.config.current();
        self.model
            .call(seen, request, config, reports, Cancel::new(cancel));
    }

    /// 发一次辅助请求（施工 3-8 四补的回顾、五补的起标题）：交给同一个端口，回报走辅助请求那一路，名字是用途和它照到的
    /// 那一条。叫停它的那一头拿着不用：内核不叫停辅助请求，actor 停了放下它、到了总时限叫停它（施工 TT-3，`overdue.rs`），
    /// 请求跟着停。运行日志那一行前面带用途（`recap request`、`title request`）。
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
        let asked = Instant::now();
        self.asides.retain(|(asking, ..)| *asking != purpose);
        self.asides.push((purpose.clone(), upto, stop, asked));
        self.overdue(purpose.clone(), upto, asked);
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
    /// R-4 上，`hooks`），交回跑完了，带着退回了默认的那一次和挂接点交的。`DEBUG` 开着的，每一段等了多久记一行
    /// `turn start timed`（施工 PF-3，`timing/turn.rs`）。
    pub(super) async fn turn_start(
        &mut self,
        turn: TurnId,
        reference: Option<String>,
        present: Vec<Present>,
        (said, speakers, opener): (Option<String>, Vec<Speaker>, Option<By>),
    ) -> Input {
        // 这一轮开口的是不是主人记在这一轮上，派工具时交给脚本（施工 X-43）；记忆照说话的人带他的卡（施工 E-1 补）。
        self.tools.round_started(turn, &speakers);
        // 放到堆上（施工 PF-3）：分段计时每段多套一层 future，这一大块原样嵌在 actor 跑的 future 里，debug 编的测试在 2 MiB
        // 的线程栈上溢出（`memory_lanes` 几个测试）。一个回合多一次分配。
        Box::pin(self.begin(turn, reference, present, (said, speakers, opener))).await
    }

    /// [`Actor::turn_start`] 做的事。
    async fn begin(
        &mut self,
        turn: TurnId,
        reference: Option<String>,
        present: Vec<Present>,
        (said, speakers, opener): (Option<String>, Vec<Speaker>, Option<By>),
    ) -> Input {
        let mut laps = TurnStart::start();
        let cwd = self.session.cwd().to_string();
        laps.config.run(self.config.turn(cwd)).await;
        let before = self.shown_now();
        // 会话记的思考强度、钉的看图模型一起交（施工 SM-1、VM-1）：内核照日志算的。
        let (effort, vision) = (self.session.effort(), self.session.vision());
        let replaced = self
            .model
            .turn(self.config.current(), reference.as_deref(), effort, vision);
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
        // 受保护的命令哪几类不问、等确认等多久也照这一轮的配置（施工 HZ-1、CD-1）。
        let confirm = crate::settings::ConfirmSettings::from(&values);
        self.guard.set_waived(confirm.waived());
        let prepare = CompactionSettings::from(&values).prepare;
        // 技能照这一刻的快照先找起来，和刷新人格并着等（施工 PERF-2）：两样都在阻塞线程里读盘，原来技能排在人格后面干等。
        // 人格、预设真换了的（交回新快照的哈希，很少见），工具面跟着变，照新的再找一次。
        let speculated = self.skills_turn(&present, &values);
        let (policy, skills) = tokio::join!(
            laps.persona.run(self.refresh_persona(values.clone())),
            laps.skills.run(speculated)
        );
        let skills = match policy {
            Some(_) => laps.skills.run(self.skills_turn(&present, &values)).await,
            None => skills,
        };
        // 压缩触发线照会话钉的、这一轮的配置（施工 CT-1，`model/line.rs`）：只换快照里的数，不重找技能。
        let policy = laps.line.run(self.reline(&values, policy)).await;
        // 项目指令在阻塞线程里读盘，和挂接点、扩展一起等，不排在前面干等（施工 PERF-1）。
        let docs = self.project_docs_turn(&present);
        // 子目录的项目指令照同一个口径（施工 P-14 下）：这一轮读、改工作目录下面的文件时补。
        self.project_docs_look();
        let config = Arc::clone(self.config.current());
        let knowledge = self.recalling();
        // 扩展和记忆、知识库一起问：各有各的限时，这一轮只等慢的那一个（施工 X-16）。
        let extensions = self.extension_injections(said.clone());
        let (mut injected, extended, docs) = tokio::join!(
            hooks::hooks(
                self.tools.memory(),
                knowledge,
                (present, speakers, opener),
                said,
                config,
                &mut laps.hooks
            ),
            laps.extensions.run(extensions),
            laps.project.run(docs)
        );
        let skills = self.skills_take(skills);
        injected.extend(extended);
        injected.extend(skills);
        injected.extend(docs);
        laps.log(turn);
        Input::TurnStartHooksDone {
            policy,
            prepare,
            confirm_ms: confirm.wait_ms(),
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
    pub(super) fn hand(&mut self, mut limits: Limits) {
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
        // 扩展的工具调用带的人格照同一时刻的（施工 TC-1）。
        self.tools.persona_in_use(self.session.persona());
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
                usage,
                cost,
                error,
                sent,
                ..
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
                    sent,
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
