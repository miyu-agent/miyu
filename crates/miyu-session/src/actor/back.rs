//! 执行器送回 actor 的（`docs/blueprint/session/actor.md` 第 3 条第 3 点）：请求的回报、到点了、工具和后台命令的结果、辅助请求的
//! 回报、等不到的「空了告诉我」（施工 C-6）、替它看图的转述（施工 8-17），照 actor 的时钟记下到的时刻，写成内核的输入。施工 C-6 从 `actor.rs` 挪出来
//! （那个文件到了行数上限）；到点叫醒的定时任务施工 T-1 上也挪过来（同样的原因）。等人确认的截止时刻（施工 CD-1）同一种定时任务。

use std::time::Duration;

use tokio::sync::mpsc;

use miyu_kernel::event::Said;
use miyu_kernel::id::CallId;
use miyu_kernel::session::{Action, Input};
use miyu_kernel::time::Timestamp;

use super::Actor;
use crate::TARGET;
use crate::port::{Back, Report};

/// 等确认到时没人确认、内核替人拒绝时写给她的那一句（施工 CD-1）：编进程序，不进策略快照（同 `miyu-endpoint` 守卫的
/// `core/guard/*.txt`）：进了快照，以前造的会话的快照就缺这一格。登记在 `26-提示词.md` 第十节。
const EXPIRED: &str =
    include_str!("../../../../resources/core/tool-results/approval-timed-out.txt");

/// 那一句给人看的说法。
const EXPIRED_SAID: &str = "core/tool-results/approval-timed-out";

impl Actor {
    /// 到点叫醒（[`Action::Wake`]）、等人确认的截止时刻（[`Action::ExpireApproval`]，施工 CD-1）：起一个定时的任务，到 `at`
    /// 这一刻送回「到点了」「到时了」。别的动作不会交到这里。人先定了的不收回：到时送回来内核不理。
    pub(super) fn wake(&mut self, alarm: Action) {
        let (at, back) = match alarm {
            Action::Wake { at, seen } => (at, Back::Woke { seen }),
            Action::ExpireApproval {
                at,
                call_id,
                nested,
            } => (at, Back::ApprovalExpired { call_id, nested }),
            _ => return,
        };
        let wait = at
            .unix_millis()
            .saturating_sub(self.clock.now().unix_millis());
        let wait = Duration::from_millis(u64::try_from(wait).unwrap_or(0));
        let backs = self.backs.clone();
        tokio::spawn(async move {
            tokio::time::sleep(wait).await;
            answer_back(&backs, back);
        });
    }

    /// 执行器送回来的，照 actor 的时钟记下到的时刻，写成内核的输入；已经不要了的工具回报，不理。
    pub(super) fn back(&mut self, back: Back) -> Option<Input> {
        // 一次抽取走完了（施工 V-51）：只叫醒 actor 看一眼退不退得下（`life.rs`），不进内核，也不算一次动静（闲多久照旧）。
        if matches!(back, Back::Extracted) {
            return None;
        }
        // 辅助请求到了总时限（施工 TT-3，`model/overdue.rs`）：已经说完了的不进内核，也不算一次动静。
        if let Back::AsideOverdue {
            purpose,
            upto,
            asked,
        } = back
        {
            let at = self.clock.now();
            return self.aside_overdue(at, purpose, upto, asked);
        }
        self.quiet = std::time::Instant::now();
        let at = self.clock.now();
        Some(match back {
            Back::Woke { seen } => Input::Woke { at, seen },
            Back::ApprovalExpired { call_id, nested } => expired(at, call_id, nested),
            Back::Tool(back) => return self.tools.back(at, back),
            Back::Guarded { waiting, verdict } => self.guards_answered(at, waiting, verdict),
            Back::Nested(back) => return self.tools.nested_back(at, back),
            // `goal` 工具交来的（施工 X-8 上，`goal/asked.rs`）。
            Back::Goal {
                call_id,
                ask,
                reply,
            } => return self.goal_asked(at, call_id, ask, reply),
            Back::Job(ended) => self.jobs.arrived(at, ended),
            Back::JobLines(seen) => crate::jobs::SessionJobs::lines(at, seen),
            Back::Shell(ran) => {
                self.tools.person_done(ran.message);
                Input::ShellRan { at, ran }
            }
            // 抽取（施工 R-6 上，`extract.rs`）：不进内核。
            Back::ExtractDue { generation, forced } => {
                self.extract_due(generation, forced);
                return None;
            }
            Back::DreamRead {
                generation,
                read,
                reply,
                hush,
            } => {
                self.dream_read(generation, read, reply, hush);
                return None;
            }
            Back::ExtractRead {
                generation,
                forced,
                read,
            } => {
                self.extract_read(generation, forced, read);
                return None;
            }
            Back::Extracted => return None,
            // 等的会话等不到了（施工 C-6，`crate::peers`）：内核照账本还在等、确实到了点的才记。
            Back::WatchEnded { session, reason } => Input::WatchEnded {
                at,
                session,
                reason,
            },
            Back::Aside {
                purpose,
                upto,
                report,
            } => self.aside_back(at, purpose, upto, report),
            // 上面已经收了。
            Back::AsideOverdue { .. } => return None,
            // 替它看图回来了（施工 8-17）：没成的记一行，这张图这一轮写占位。
            Back::Described { blob, seen } => {
                let seen = seen
                    .inspect_err(|why| {
                        tracing::info!(target: TARGET, blob = %blob, why = why.as_str(), "image not described");
                    })
                    .ok();
                Input::Described { at, blob, seen }
            }
            Back::Report { seen, report } => match report {
                Report::Sent { model, request, .. } => Input::RequestSent {
                    at,
                    seen,
                    model,
                    request,
                },
                Report::Delta(delta) => Input::ModelDelta { at, seen, delta },
                Report::Ended {
                    usage,
                    cost,
                    error,
                    wait_ms,
                    excess,
                    failover,
                    sent,
                } => {
                    self.ended(seen, usage.as_ref(), error.as_ref());
                    Input::ModelEnded {
                        at,
                        seen,
                        usage,
                        cost: cost.map(|cost| *cost),
                        error,
                        wait_ms,
                        excess,
                        failover,
                        sent,
                    }
                }
            },
        })
    }
}

/// 第 `call_id` 次调用（脚本里调的第 `nested` 件）等确认到时了：交给内核的那一条，带上编进程序的那一句。
fn expired(at: Timestamp, call_id: CallId, nested: Option<u32>) -> Input {
    Input::ApprovalExpired {
        at,
        call_id,
        nested,
        text: EXPIRED.trim_end_matches(['\n', '\r']).to_string(),
        human: Some(Said::new(EXPIRED_SAID)),
    }
}

/// 送回 actor。会话停了就送不进去，丢掉。
#[expect(
    clippy::let_underscore_must_use,
    reason = "会话停了：到点了也没人要，丢掉"
)]
pub(super) fn answer_back(backs: &mpsc::UnboundedSender<Back>, back: Back) {
    let _ = backs.send(back);
}
