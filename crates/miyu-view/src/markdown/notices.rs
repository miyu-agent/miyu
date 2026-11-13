//! 旁白和一轮的收尾：照时间线放一行。压缩的摘要折在下面；子代理的回报起一个 `Subagent` 小标题写全文；撤销说明不进（撤掉的
//! 当没发生过）；没出错的收尾不写。
//!
//! 施工 SE-1 中下中：后台命令下面脚本里调的工具照写法写（`markdown/extras.rs`），`tools: full` 再接 blob 里的输出全文；答了
//! 提问的 `chat` 一行写完。

use crate::entry::{Answered, End, Entry, JobDone, Mark};
use crate::notice::{CompactionState, Notice};

use miyu_kernel::id::Parent;

use super::extras;
use super::options::{Purpose, Tools};
use super::steps::folded;
use super::text::{Out, code_block, details, one_line, prose, quote};
use super::{Document, Say, Speaker, short};

/// 一轮的收尾：出错、打断、步数到顶、核心崩了的写一行；交回写了没有。
pub(super) fn end(out: &mut Out, end: &End, say: &Say<'_>) -> Option<Speaker> {
    let line = match end.reason.as_str() {
        "error" => {
            let why = end
                .explain
                .clone()
                .or_else(|| end.error.as_ref().map(|error| error.message.clone()))
                .unwrap_or_default();
            say.say("end/error", &[("why", why)])
        }
        "interrupted" => say.say("end/interrupted", &[]),
        "step_limit" => say.say("end/step-limit", &[]),
        "aborted" => say.say("end/aborted", &[]),
        _ => return None,
    };
    out.block(&quote(&line));
    Some(Speaker::Nobody)
}

/// 一条旁白；交回写了没有、写完以后算谁的。
pub(super) fn notice(
    out: &mut Out,
    entry: &Entry,
    notice: &Notice,
    doc: &Document<'_>,
    say: &Say<'_>,
) -> Option<Speaker> {
    let line = match notice {
        Notice::Compaction(compaction) => match compaction.state {
            CompactionState::Done => {
                out.block(&quote(&say.say("compacted", &[])));
                if let Some(summary) = doc.sources.summaries.get(&entry.id) {
                    out.block(&details(&say.say("summary", &[]), &prose(summary, true)));
                }
                return Some(Speaker::Nobody);
            }
            CompactionState::Failed => {
                let why = compaction
                    .explain
                    .clone()
                    .or_else(|| compaction.error.as_ref().map(|e| e.message.clone()))
                    .unwrap_or_default();
                say.say("compaction-failed", &[("why", why)])
            }
            CompactionState::Running => return None,
        },
        Notice::Cleared {} => say.say("cleared", &[]),
        Notice::Paused { .. } => say.say("paused", &[]),
        // 只在视图流里有（施工 TT-3），翻页、导出见不到；见到了也不写：标题没变，不是对话里的事。
        Notice::Reverted { .. } | Notice::RetitleFailed { .. } => return None,
        Notice::Job(done) if done.report.is_some() => return Some(report(out, done, say)),
        Notice::Job(done) => return Some(job(out, done, doc, say)),
        Notice::Lines(seen) => {
            let line = say.count("lines", seen.lines, &[("title", one_line(&seen.title))]);
            out.block(&quote(&line));
            out.block(&folded(&seen.text, say));
            return Some(Speaker::Nobody);
        }
        Notice::Answered(answered) if doc.options.purpose == Purpose::Chat => {
            let answers = answers(answered).replace("\n- ", "; ");
            let answers = answers.trim_start_matches("- ").to_string();
            say.say("answered/line", &[("answers", answers)])
        }
        Notice::Answered(answered) => {
            out.block(&quote(&say.say("answered", &[])));
            out.block(&answers(answered));
            return Some(Speaker::Nobody);
        }
        Notice::Recap { text, .. } => {
            out.block(&quote(&say.say("recap", &[])));
            out.block(&prose(text, false));
            return Some(Speaker::Nobody);
        }
        Notice::Model(swap) => say.say("model", &[("model", swap.model.clone())]),
        Notice::Persona(recast) => {
            let mut lines = Vec::new();
            let to = |to: &Option<String>| to.clone().unwrap_or_else(|| "—".to_string());
            if let Some(persona) = &recast.persona {
                lines.push(quote(&say.say("persona", &[("to", to(&persona.to))])));
            }
            if let Some(preset) = &recast.preset {
                lines.push(quote(&say.say("preset", &[("to", to(&preset.to))])));
            }
            out.block(&lines.join("\n>\n"));
            return Some(Speaker::Nobody);
        }
        Notice::Workspace { cwd, .. } => say.say("workspace", &[("cwd", cwd.clone())]),
        Notice::Peer(peer) => say.say(
            "peer",
            &[
                ("session", short(peer.session.as_str()).to_string()),
                ("reason", peer.reason.as_str().to_string()),
            ],
        ),
    };
    out.block(&quote(&line));
    Some(Speaker::Nobody)
}

/// 一件后台命令了结了：一行；下面脚本里调的工具照写法写，`full` 再接 blob 里的输出全文。
fn job(out: &mut Out, done: &JobDone, doc: &Document<'_>, say: &Say<'_>) -> Speaker {
    let key = match done.mark {
        Mark::Done => "job/done",
        Mark::Failed => "job/failed",
        Mark::Stopped => "job/stopped",
    };
    let title = match done.title.trim().is_empty() {
        true => done.command.clone().unwrap_or_default(),
        false => done.title.clone(),
    };
    let mut line = say.say(key, &[("title", one_line(&title))]);
    let parent = Some(Parent::Job(done.job.clone()));
    if doc.options.purpose == Purpose::Chat
        && let Some(nested) = extras::nested_line(parent.clone(), doc)
    {
        line.push_str(&format!(" · {nested}"));
    }
    out.block(&quote(&line));
    match doc.options.purpose {
        Purpose::Read => extras::nested(out, parent, doc, say),
        Purpose::Agent => extras::nested_raw(out, parent, doc),
        Purpose::Chat => {}
    }
    if doc.options.tools == Tools::Full
        && let Some(text) = done
            .output
            .as_ref()
            .and_then(|output| doc.sources.blobs.get(&output.blob))
            .filter(|text| !text.trim().is_empty())
    {
        match doc.options.purpose {
            Purpose::Read => out.block(&folded(text, say)),
            Purpose::Agent | Purpose::Chat => out.block(&code_block(text, "text")),
        }
    }
    Speaker::Nobody
}

/// 子代理的回报：`Subagent` 小标题带标题，没做完的写一行，再是回报全文。
fn report(out: &mut Out, done: &JobDone, say: &Say<'_>) -> Speaker {
    let title = one_line(&done.title);
    match title.is_empty() {
        true => out.block("## Subagent"),
        false => out.block(&format!("## Subagent: {title}")),
    }
    if done.mark != Mark::Done {
        out.block(&quote(&say.say("agent/stopped", &[])));
    }
    if let Some(report) = done.report.as_deref().filter(|r| !r.trim().is_empty()) {
        out.block(&prose(report, false));
    }
    Speaker::Subagent
}

/// 一组题答了：一题一行，问的话、选的和写的。
fn answers(answered: &Answered) -> String {
    answered
        .questions
        .iter()
        .zip(&answered.answers)
        .map(|(question, response)| {
            let mut said: Vec<String> = response.picked.clone();
            said.extend(response.text.iter().cloned());
            said.extend(response.notes.iter().cloned());
            format!(
                "- {}: {}",
                one_line(&question.question),
                one_line(&said.join(" — "))
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
