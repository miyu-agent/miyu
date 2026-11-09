//! 发一句话、等由它开的那一轮结束、读回她说的正文：整理会话一批一轮（施工 D-1 上，`dream.rs`）和问顾问（施工 ADV-1，设计
//! `69-旗舰顾问.md`，`advisor.rs`）共用。施工 ADV-1 从 `dream.rs` 挪出来。

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::{Body, EndReason, ModelDelta, TransientBody};
use miyu_kernel::id::{CommandId, SessionId, TurnId};
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};
use miyu_session::{Handle, Pushed};

/// 那一轮怎么结束的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Turned {
    /// 答完了：`message.assistant` 的文字块照先后接起来，思考不要。
    Answered(String),
    /// 被打断了。
    Stopped,
    /// 出错结束了（`turn.ended` 的原因不是答完、不是打断）：英文的一句为什么，原因的写法加上请求的错。
    Failed(String),
}

/// 以 `by` 的名义、编号 `id` 给会话 `session`（把手 `handle`）发 `text`，等由它开的那一轮结束。这一轮里模型的增量照先后交给
/// `delta`（整理会话边流边交，施工 D-1 下；顾问不要）。
///
/// # Errors
///
/// 会话停了、拒了这一句：英文的一句为什么。
pub(super) async fn ask(
    (handle, session): (&Handle, &SessionId),
    id: CommandId,
    by: By,
    text: String,
    delta: &mut (dyn FnMut(&ModelDelta) + Send),
) -> Result<Turned, String> {
    let gone = || format!("session {session} stopped");
    // 先订阅：这一句的事件一定先于发话的回应到（`04-核心协议.md` 第六节第 2 条）。
    let mut pushes = handle.subscribe().await.map_err(|_| gone())?;
    let send = Command::Send {
        blocks: vec![Block::Text(Text { text })],
        urgent: false,
        venue: None,
    };
    let outcome = handle.command(id, by, send).await.map_err(|_| gone())?;
    let Outcome::Accepted { events } = outcome else {
        return Err(format!("session {session} refused it: {outcome:?}"));
    };
    let said = events
        .first()
        .copied()
        .ok_or_else(|| "nothing was recorded".to_string())?;
    let mut turn: Option<TurnId> = None;
    let (mut answer, mut error) = (String::new(), None);
    loop {
        let pushed = pushes.next().await.map_err(|_| gone())?;
        let events = match &*pushed {
            Pushed::Events(events) => events,
            Pushed::Transient(transient) => {
                if let TransientBody::ModelDelta(piece) = &transient.body
                    && turn.is_some()
                    && transient.turn == turn
                {
                    delta(piece);
                }
                continue;
            }
            Pushed::Goal { .. } => continue,
        };
        for event in events {
            match &event.body {
                Body::TurnStarted(started) if started.trigger == Some(said) => {
                    turn = event.turn;
                }
                _ if turn.is_none() || event.turn != turn => {}
                Body::MessageAssistant(reply) => {
                    for block in &reply.blocks {
                        if let Block::Text(text) = block {
                            answer.push_str(&text.text);
                        }
                    }
                }
                Body::ModelCalled(called) => {
                    if let Some(failed) = &called.error {
                        error = Some(failed.message.clone());
                    }
                }
                Body::TurnEnded(ended) => {
                    return Ok(match &ended.reason {
                        EndReason::Completed => Turned::Answered(answer),
                        EndReason::Interrupted => Turned::Stopped,
                        reason => Turned::Failed(match error {
                            Some(error) => format!("{}: {error}", reason.as_str()),
                            None => reason.as_str().to_string(),
                        }),
                    });
                }
                _ => {}
            }
        }
    }
}
