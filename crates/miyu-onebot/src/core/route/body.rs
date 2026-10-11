//! `ext.onebot.chat.decided` 的 `body`（施工 O-23，`onebot.md` 第一条「群里怎么叫她」第 7 条那张表；「施工时定的」第 76、97
//! 条）：判的几条、发的人是谁、进站链的结果和原因、旗、线路规程、条件和加分、顶替、走的路、判官那一次、算分的每一项、结论。
//! 名字照群聊内核的类型写成蛇形的字。纯逻辑。O-23 下从 `decide.rs` 挪出来，多 `discipline`、`supersede`、`judge`、`score`。
//!
//! 条件的写法读回来（[`kind_of`]）：投影照判断认判过要回的那几条（`projection.rs`，第 11 条）。
//!
//! 判不了照回的（O-37，[`finale`]）不另加格：`outcome` 是 `reply`、`judge` 里是 `unjudged`、没有 `score` 就是它（「施工时定的」
//! 第 220 条）。

use miyu_chat::{Flag, Kind, Mode, Outcome, Route, Score, Standing, Supersede, Unreadable, Why};
use serde_json::{Value, json};

use super::ask::{Answer, Unjudged};
use super::decide::{Conclusion, Decision};
use super::projection::RATE_LIMITED;

/// 判官回来了的那一段：问的那一次和算的分（读出来了才有分）。
#[derive(Debug, Clone, Copy)]
pub(super) struct Judged<'a> {
    /// 问的那一次。
    pub(super) answer: &'a Answer,
    /// 算的分。
    pub(super) score: Option<&'a Score>,
}

/// 最后怎么做：回、只记下、回一句提示。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Finale {
    /// 开一轮。
    Reply,
    /// 只记下。
    Record,
    /// 回一句提示：为什么。
    Notice(Why),
}

impl Finale {
    /// `body` 的 `outcome` 的写法，运行日志也照它。
    pub(super) fn name(self) -> &'static str {
        match self {
            Finale::Reply => "reply",
            Finale::Record => "record",
            Finale::Notice(_) => "notice",
        }
    }
}

/// 最后怎么做：问判官的照分，分够的回；判不了的（「群里怎么叫她」第 13 条）主触发是冲她来的照回，别的只记下（18 第七节；
/// 冲她来的照回 O-37 改，2026-10-11 项目主人定，照旧版）。主触发照合起来的条件取（顶替重判的几条一起）。读出来了、可没算分
/// 的（本机的钟读不出）不是判不了，只记下。
pub(super) fn finale(decision: &Decision, judged: Option<Judged<'_>>) -> Finale {
    match decision.conclusion {
        Conclusion::Reply => Finale::Reply,
        Conclusion::Record => Finale::Record,
        Conclusion::Notice(why) => Finale::Notice(why),
        Conclusion::Judge(_) => match judged {
            Some(Judged {
                score: Some(score), ..
            }) if score.reply => Finale::Reply,
            Some(Judged { answer, .. }) if answer.result.is_err() && called(decision) => {
                Finale::Reply
            }
            _ => Finale::Record,
        },
    }
}

/// 主触发是冲她来（`chat.md` 第三条第 7 条）。
fn called(decision: &Decision) -> bool {
    decision
        .passed
        .as_ref()
        .is_some_and(|passed| passed.conditions.primary() == Some(Kind::Direct))
}

/// `body`（第 7 条那张表）：发的人是 `standing`；问了判官的另有 `judged`。
pub(super) fn body(decision: &Decision, standing: Standing, judged: Option<Judged<'_>>) -> Value {
    let mut body = json!({
        "msgs": decision.msgs,
        "standing": standing_name(standing),
        "outcome": finale(decision, judged).name(),
    });
    let (inbound, why) = match decision.verdict.outcome {
        Outcome::Pass => ("pass", None),
        Outcome::RecordOnly(why) => ("record_only", Some(why)),
        Outcome::Notice(why) => ("notice", Some(why)),
    };
    body["inbound"] = json!(inbound);
    if let Some(why) = why {
        body["why"] = json!(why_name(why));
    }
    if !decision.verdict.flags.is_empty() {
        let flags: Vec<&str> = decision
            .verdict
            .flags
            .iter()
            .map(|flag| flag_name(*flag))
            .collect();
        body["flags"] = json!(flags);
    }
    let Some(passed) = &decision.passed else {
        return body;
    };
    body["discipline"] = json!(passed.discipline.name());
    let hits: Vec<Value> = passed
        .conditions
        .hits
        .iter()
        .map(|hit| json!({"kind": kind_name(hit.kind), "bonus": hit.bonus}))
        .collect();
    body["conditions"] = json!(hits);
    match &passed.supersede {
        Supersede::None => {}
        Supersede::Inherit { msg, .. } => body["supersede"] = json!({"inherit": msg.get()}),
        Supersede::Rejudge { cancel, .. } => body["supersede"] = json!({"rejudge": cancel.get()}),
    }
    body["route"] = json!(route_name(passed.route));
    let mode = match passed.route {
        Route::Judge => Mode::Reply,
        Route::ModerationOnly => Mode::ModerationOnly,
        Route::Record | Route::Commit => return body,
    };
    let mut asked = json!({"mode": mode_name(mode)});
    if passed.rate_full {
        asked["unjudged"] = json!("rate_full");
    }
    if let Some(Judged { answer, score }) = judged {
        asked["tries"] = json!(answer.tries);
        asked["millis"] = json!(answer.millis);
        if let Some(model) = &answer.model {
            asked["model"] = json!(model);
        }
        match &answer.result {
            Ok(judgement) => {
                let mut said = json!({
                    "scores": judgement.scores,
                    "should_reply": judgement.should_reply,
                    "to_bot": judgement.to_bot,
                    "reason": judgement.reason,
                });
                if let Some(severity) = judgement.severity {
                    said["severity"] = json!(severity);
                }
                asked["answer"] = said;
            }
            Err(unjudged) => {
                let (why, detail) = unjudged_name(unjudged);
                asked["unjudged"] = json!(why);
                if let Some(detail) = detail {
                    asked["detail"] = json!(detail);
                }
            }
        }
        if let Some(score) = score {
            body["score"] = json!({
                "raw": score.raw, "adjust": score.adjust, "bonus": score.bonus, "lift": score.lift,
                "threshold": score.threshold, "total": score.total, "reply": score.reply,
            });
        }
    }
    body["judge"] = asked;
    body
}

/// 没放行的原因的写法：`body` 的 `why`，提示的 `reason`。
pub(super) fn why_name(why: Why) -> &'static str {
    match why {
        Why::Asleep => "asleep",
        Why::Muted => "muted",
        Why::NotAllowed => "not_allowed",
        Why::RateLimited => RATE_LIMITED,
    }
}

/// 判不了的为什么的写法和细节：核心拒了的细节是原因码，读不出的是哪一种（少了哪一维写成 `dimension:<名字>`）。
pub(super) fn unjudged_name(unjudged: &Unjudged) -> (&'static str, Option<String>) {
    match unjudged {
        Unjudged::Queue => ("queue", None),
        Unjudged::Timeout => ("timeout", None),
        Unjudged::Refused(reason) => ("refused", Some(reason.clone())),
        Unjudged::Unreadable(unreadable) => {
            let detail = match unreadable {
                Unreadable::NoObject => "no_object".to_string(),
                Unreadable::NoSeverity => "no_severity".to_string(),
                Unreadable::Dimension(name) => format!("dimension:{name}"),
            };
            ("unreadable", Some(detail))
        }
    }
}

/// 条件的种类的写法读回来：认不出的是空的。
pub(super) fn kind_of(name: &str) -> Option<Kind> {
    KINDS
        .iter()
        .find(|(_, written)| *written == name)
        .map(|(kind, _)| *kind)
}

/// 条件的种类和写法。
const KINDS: [(Kind, &str); 5] = [
    (Kind::Direct, "direct"),
    (Kind::Continuation, "continuation"),
    (Kind::AfterSpeaking, "after_speaking"),
    (Kind::Probability, "probability"),
    (Kind::Moderation, "moderation"),
];

/// 条件的种类的写法。
fn kind_name(kind: Kind) -> &'static str {
    KINDS
        .iter()
        .find(|(known, _)| *known == kind)
        .map_or("", |(_, written)| written)
}

/// 发的人是谁的写法（施工 O-27 随叫法改：原来写 `owner`、`trusted`）。
fn standing_name(standing: Standing) -> &'static str {
    match standing {
        Standing::Admin => "admin",
        Standing::Whitelisted => "whitelisted",
        Standing::Member => "member",
    }
}

/// 旗的写法。
fn flag_name(flag: Flag) -> &'static str {
    match flag {
        Flag::Moderation => "moderation",
    }
}

/// 走的路的写法。
fn route_name(way: Route) -> &'static str {
    match way {
        Route::Record => "record",
        Route::Commit => "commit",
        Route::ModerationOnly => "moderation_only",
        Route::Judge => "judge",
    }
}

/// 问判官的哪一种的写法。
fn mode_name(mode: Mode) -> &'static str {
    match mode {
        Mode::Reply => "reply",
        Mode::ModerationOnly => "moderation_only",
    }
}

#[cfg(test)]
mod tests;
