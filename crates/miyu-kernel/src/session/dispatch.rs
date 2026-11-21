//! 一条输入照种类分派、一个命令照种类分派（从 `session.rs` 挪来，施工 LW-8：那个文件到了行数上限）。每一支经
//! [`crate::apart::sync`] 单独成一个函数：原来全并进 `handle`，走到一种输入也算执行过整块。

use super::*;

impl Session {
    /// 照输入的种类分派。嵌套调用的收尾、派发在 [`Self::handle`] 里统一做（施工 M-1，`nested.rs`）。
    // 每一支经 `apart::sync` 单独成一个函数（施工 LW-8）：原来全并进 `handle`（52 kB），走到一种输入也算执行过整块。
    pub(super) fn react(&mut self, input: Input) -> Vec<Action> {
        match input {
            Input::Command(received) => apart::sync(|| self.receive(received)),
            Input::Stored { at, upto } => apart::sync(|| self.stored(at, upto)),
            Input::Environment(environment) => {
                self.environment = environment;
                Vec::new()
            }
            Input::Limits(limits) => apart::sync(|| self.take_limits(limits)),
            Input::Reread { at, seen, files } => apart::sync(|| self.reread_done(at, seen, files)),
            Input::Recalled { texts } => {
                self.history.recall(texts);
                Vec::new()
            }
            hooked @ Input::TurnStartHooksDone { .. } => {
                apart::sync(|| self.turn_start_hooked(hooked))
            }
            Input::RequestSent {
                at,
                seen,
                model,
                request,
            } => apart::sync(|| self.request_sent(at, seen, model, request)),
            Input::ModelDelta { at, seen, delta } => {
                apart::sync(|| self.model_delta(at, seen, delta))
            }
            ended @ Input::ModelEnded { .. } => apart::sync(|| self.model_ended_input(ended)),
            Input::Woke { at, seen } => apart::sync(|| self.woke(at, seen)),
            expired @ Input::ApprovalExpired { .. } => {
                apart::sync(|| self.approval_expired(expired))
            }
            Input::ShellRan { at, ran } => apart::sync(|| self.shell_ran(at, ran)),
            Input::ToolDone {
                at,
                call_id,
                error,
                blocks,
                duration_ms,
                human,
                effects,
                stopped,
                end_turn,
                sent,
                display_name,
                detail,
            } => apart::sync(|| {
                self.tool_done(
                    at,
                    ToolResult {
                        call_id,
                        status: if stopped {
                            ToolStatus::Cancelled
                        } else if error {
                            ToolStatus::Error
                        } else {
                            ToolStatus::Ok
                        },
                        blocks,
                        duration_ms,
                        human,
                        effects,
                        end_turn: end_turn && !stopped && !error,
                        sent,
                        display_name,
                        detail,
                    },
                )
            }),
            Input::ToolProgress { at, call_id, text } => {
                apart::sync(|| self.tool_progress(at, call_id, text))
            }
            Input::ToolGuarded {
                at,
                call_id,
                verdict,
            } => apart::sync(|| self.tool_guarded(at, call_id, verdict)),
            Input::ToolAsks {
                at,
                call_id,
                questions,
            } => apart::sync(|| self.tool_asks(at, call_id, questions)),
            Input::ReadBack { at, from, events } => {
                apart::sync(|| self.read_back(at, from, events))
            }
            Input::Restarting { at } => apart::sync(|| self.restart(at)),
            Input::JobEnded {
                at,
                by,
                cause,
                reported,
            } => apart::sync(|| self.job_ended(at, by, cause, reported)),
            Input::JobLines {
                at,
                by,
                cause,
                lines,
            } => apart::sync(|| self.job_lines(at, by, cause, lines)),
            Input::AsideSent {
                at,
                purpose,
                upto,
                model,
                request,
                fell_back,
            } => apart::sync(|| self.aside_sent(at, &purpose, upto, (model, request), fell_back)),
            Input::AsideDelta {
                at,
                purpose,
                upto,
                delta,
            } => apart::sync(|| self.aside_delta(at, &purpose, upto, delta)),
            Input::AsideEnded {
                at,
                purpose,
                upto,
                usage,
                cost,
                error,
                sent,
            } => apart::sync(|| self.aside_ended(at, &purpose, upto, (usage, cost), error, sent)),
            Input::Watched { watched } => {
                self.watched = watched;
                Vec::new()
            }
            Input::WatchEnded {
                at,
                session,
                reason,
            } => apart::sync(|| self.watch_ended(at, session, reason)),
            Input::Described { at, blob, seen } => apart::sync(|| self.described(at, blob, seen)),
            Input::Nested(nested) => apart::sync(|| self.nested(nested)),
        }
    }

    /// 收到一个命令。接受过的编号照上一次回应，换了参数的拒绝，`id_reused`（施工 YC-7，`recent.rs`）；新的照命令判。回顾不记
    /// 编号（施工 3-8 四补）：再来一次就是再要一次，没有新内容的照样交回上一句。
    // 照命令分派的每一支同 `react`，各自一个函数（施工 LW-8）。
    fn receive(&mut self, received: Received) -> Vec<Action> {
        let Received {
            id,
            by,
            at,
            command,
        } = received;
        let digest = recent::digest(&command);
        if !matches!(command, Command::Recap)
            && let Some(events) = self.recent.get(&id)
        {
            if self.recent.reused(&id, digest) {
                return vec![rejected(id, Reason::IdReused)];
            }
            let events = events.to_vec();
            return self.reply_when_stored(id, events);
        }
        // 读回日志的时候不接命令：会话 actor 做完才接下一个，这是兜底（施工 6-9）。
        if self.reading.is_some() {
            return vec![rejected(id, Reason::Restoring)];
        }
        if self.ledger.sealed() && !seal::taken_when_sealed(&command) {
            return vec![rejected(id, Reason::Sealed)];
        }
        self.recent.expect(&id, digest);
        match command {
            Command::Send {
                blocks,
                urgent,
                venue,
            } => apart::sync(|| self.send(id, by, at, (blocks, venue, None), urgent)),
            Command::Typed { blocks, command } => {
                apart::sync(|| self.send(id, by, at, (blocks, None, Some(command)), false))
            }
            Command::Kickoff { blocks } => apart::sync(|| self.kickoff(id, by, at, blocks)),
            Command::Shell { text, urgent } => {
                apart::sync(|| self.send_shell(id, by, at, text, urgent))
            }
            Command::Append { event } => apart::sync(|| self.append(id, by, at, event)),
            Command::Respond { to, facts, tail } => {
                apart::sync(|| self.respond(id, by, at, to, (facts, tail)))
            }
            Command::Note { facts } => apart::sync(|| self.note(id, by, at, facts)),
            Command::Interrupt { queued } => apart::sync(|| self.interrupt(id, by, at, queued)),
            Command::Withdraw { message } => apart::sync(|| self.withdraw(id, by, at, message)),
            Command::SetMeta {
                title,
                pinned,
                project,
            } => apart::sync(|| self.set_meta(id, by, at, (title, pinned, project))),
            Command::Retitle => apart::sync(|| self.retitle(id, by, at)),
            Command::SetWorkspace { cwd, dirs } => {
                apart::sync(|| self.set_workspace(id, by, at, cwd, dirs))
            }
            Command::Ran { text, command } => apart::sync(|| self.ran(id, by, at, text, command)),
            Command::Configure { model } => apart::sync(|| self.configure(id, by, at, model)),
            Command::SetEffort { level } => apart::sync(|| self.set_effort(id, by, at, level)),
            Command::SetContext { tokens } => apart::sync(|| self.set_context(id, by, at, tokens)),
            Command::SetVision { vision } => apart::sync(|| self.set_vision(id, by, at, vision)),
            Command::SetCompactionLine { percent } => {
                apart::sync(|| self.set_line(id, by, at, percent))
            }
            Command::SetVoiceLimit { ms } => apart::sync(|| self.set_voice_limit(id, by, at, ms)),
            Command::Recast { persona, preset } => {
                apart::sync(|| self.recast(id, by, at, (persona, preset)))
            }
            Command::SetPermission { level, read_only } => {
                apart::sync(|| self.set_permission(id, by, at, level, read_only))
            }
            Command::Answer {
                call_id,
                answer: answer @ Answer::Approval { .. },
            } => apart::sync(|| self.answer(id, by, at, call_id, answer)),
            Command::Answer {
                call_id,
                answer: Answer::Questions(answers),
            } => apart::sync(|| self.answer_question(id, by, at, call_id, answers)),
            Command::Revert { turn } => apart::sync(|| self.revert(id, by, at, turn)),
            Command::Unrevert => apart::sync(|| self.unrevert(id, by, at)),
            Command::Redo { text, attachments } => {
                apart::sync(|| self.redo(id, by, at, text, attachments))
            }
            Command::Compact { instructions } => apart::sync(|| self.compact(id, at, instructions)),
            Command::Clear => apart::sync(|| self.clear(id, at)),
            Command::ClearTodos => apart::sync(|| self.clear_todos(id, by, at)),
            Command::SetState(written) => {
                apart::sync(|| self.set_package_state(id, by, at, written))
            }
            Command::Recap => apart::sync(|| self.recap(id)),
            Command::Report(reported) => apart::sync(|| self.report(id, by, at, reported)),
            Command::PeerIdle { status } => apart::sync(|| self.peer_idle(id, by, at, status)),
            Command::Goal(change) => apart::sync(|| self.goal(id, by, at, change)),
            Command::EndDream(ended) => apart::sync(|| self.end_dream(id, by, at, ended)),
        }
    }
}
