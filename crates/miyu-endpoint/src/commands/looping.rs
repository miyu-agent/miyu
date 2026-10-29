//! `/loop`（施工 J-2，设计 `45-定时和循环.md` 第三节第 4 条，照 Claude Code）：让一句话在这个会话里反复跑。
//!
//! - `/loop <间隔> <话>`、`/loop <话> every <间隔>`、`/loop every <间隔> <话>`：固定间隔。间隔写 `30s`、`5m`、`2h`、`1d`，不到
//!   [`miyu_tool::WAKEUP_MIN`] 秒的照它；建这个会话上的一行固定循环，下一次就是现在：唤醒表马上交进来跑第一次。
//! - `/loop <话>`：动态。同样马上跑一次，之后她每轮自己调 `schedule_wakeup` 定下一次，不调就停。
//! - `/loop off`：删掉这个会话的循环，固定的、动态的都删。
//! - 一个会话同时只有一个循环：再敲一次 `/loop` 换掉原来的（主会话 2026-10-27 定，技术题）。人敲的记成属主建的
//!   （`by: person`），跑在这个会话上；到点交进会话、她每轮定下一次都是唤醒表的事（施工 J-1 上、`schedules/port.rs`）。
//!
//! 只在工具面上有 `schedule_wakeup` 的会话里认（循环这个包装了、预设开着）；别的会话照「没有这条命令」答，命令菜单里也不列。

use std::sync::Arc;

use miyu_kernel::id::SessionId;
use miyu_kernel::origin::By;
use miyu_session::Handle;

use super::Said;
use super::registry::{Call, Commands, Ran, Running, Slash};
use crate::Core;
use crate::refusal::Refusal;
use crate::schedules::{Creator, Loop, Target, Wanted, When};

/// 循环这个包的编号（同 `miyu_schedule::LOOP`；端点不依赖软件包的 crate）。
pub(super) const LOOP_PACKAGE: &str = "loop";

/// 登记 `/loop`。
pub(super) fn register(commands: &mut Commands) {
    commands.register(Arc::new(LoopCommand));
}

/// `/loop`。
struct LoopCommand;

impl Slash for LoopCommand {
    fn name(&self) -> &'static str {
        "loop"
    }

    fn package(&self) -> Option<&'static str> {
        Some(LOOP_PACKAGE)
    }

    fn summary(&self) -> &'static str {
        "commands/summary/loop"
    }

    fn argument(&self) -> Option<&'static str> {
        Some("commands/argument/loop")
    }

    fn usage(&self) -> &'static [(&'static str, &'static str)] {
        &[
            (
                "commands/placeholder/loop-fixed",
                "commands/usage/loop/fixed",
            ),
            (
                "commands/placeholder/loop-dynamic",
                "commands/usage/loop/dynamic",
            ),
            ("off", "commands/usage/loop/off"),
        ]
    }

    fn allowed(&self, _core: &Core, handle: &Handle, _by: &By) -> Result<(), Refusal> {
        match handle.loops() {
            true => Ok(()),
            false => Err(Refusal::UNKNOWN_COMMAND),
        }
    }

    fn run<'a>(&'a self, call: Call<'a>) -> Running<'a> {
        Box::pin(async move {
            let asked = parse(call.rest)?;
            let stopped = stop(call.core, call.session)?;
            let said = match asked {
                Asked::Off if stopped => Said::plain("commands/loop-off"),
                Asked::Off => Said::plain("commands/loop-none"),
                Asked::Fixed { every, prompt } => {
                    start(
                        call.core,
                        call.session,
                        When::Every { every },
                        Loop::Fixed,
                        prompt,
                    )?;
                    Said::with(
                        "commands/loop-fixed",
                        vec![("every", interval(every)), ("text", one_line(prompt))],
                    )
                }
                Asked::Dynamic { prompt } => {
                    let at = call.core.schedules().now();
                    start(
                        call.core,
                        call.session,
                        When::At { at },
                        Loop::Dynamic,
                        prompt,
                    )?;
                    Said::with("commands/loop-dynamic", vec![("text", one_line(prompt))])
                }
            };
            Ok(Ran {
                events: Vec::new(),
                said,
            })
        })
    }
}

/// 删掉跑在这个会话上的循环（固定的、动态的）：删了的交回真，一个都没有的交回假。
fn stop(core: &Core, session: &SessionId) -> Result<bool, Refusal> {
    let schedules = core.schedules();
    let rows = schedules.list().map_err(|_| Refusal::INTERNAL)?;
    let mut stopped = false;
    for row in rows.iter().filter(|row| {
        row.looping.is_some()
            && matches!(&row.target, Target::Session { session: on } if on == session)
    }) {
        schedules.cancel(&row.id).map_err(|_| Refusal::INTERNAL)?;
        stopped = true;
    }
    Ok(stopped)
}

/// 建这个会话上的一行循环：第一次就是现在，唤醒表马上交进来。
fn start(
    core: &Core,
    session: &SessionId,
    when: When,
    looping: Loop,
    prompt: &str,
) -> Result<(), Refusal> {
    let schedules = core.schedules();
    let wanted = Wanted {
        by: Creator::Person,
        when,
        prompt: prompt.to_string(),
        target: Target::Session {
            session: session.clone(),
        },
        looping: Some(looping),
        first: Some(schedules.now()),
    };
    schedules
        .add(wanted)
        .map(|_| ())
        .map_err(|_| Refusal::INTERNAL)
}

/// 间隔写成给人看的：整天、整小时、整分钟的照那个单位，别的照秒（`1d`、`2h`、`5m`、`90s`）。
fn interval(seconds: u64) -> String {
    match seconds {
        0 => "0s".to_string(),
        s if s % 86_400 == 0 => format!("{}d", s / 86_400),
        s if s % 3_600 == 0 => format!("{}h", s / 3_600),
        s if s % 60 == 0 => format!("{}m", s / 60),
        s => format!("{s}s"),
    }
}

/// 回执里的原话：换行写成空格（回执是一行）。
fn one_line(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 人要做的。
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Asked<'a> {
    /// 固定间隔：每多少秒一次，说什么。
    Fixed { every: u64, prompt: &'a str },
    /// 动态：她自己定节奏，说什么。
    Dynamic { prompt: &'a str },
    /// 停掉这个会话的循环。
    Off,
}

/// 读 `/loop` 后面的字（去掉了前后空白）。什么都没写的、只写了间隔没写话的参数不对。
pub(super) fn parse(asked: &str) -> Result<Asked<'_>, Refusal> {
    if asked == "off" {
        return Ok(Asked::Off);
    }
    let words: Vec<&str> = asked.split_whitespace().collect();
    let fixed = match words.as_slice() {
        [] => return Err(Refusal::BAD_PARAMS),
        ["every", interval, ..] => seconds(interval).map(|every| (every, after(asked, 2))),
        [interval, ..] if seconds(interval).is_some() => {
            seconds(interval).map(|every| (every, after(asked, 1)))
        }
        [.., "every", interval] => seconds(interval).map(|every| (every, before(asked, 2))),
        _ => None,
    };
    match fixed {
        Some((_, "")) => Err(Refusal::BAD_PARAMS),
        Some((every, prompt)) => Ok(Asked::Fixed {
            every: every.max(miyu_tool::WAKEUP_MIN),
            prompt,
        }),
        None => Ok(Asked::Dynamic { prompt: asked }),
    }
}

/// `30s`、`5m`、`2h`、`1d` 是多少秒；别的、溢出的没有。
fn seconds(word: &str) -> Option<u64> {
    let unit = match word.chars().last()? {
        's' => 1,
        'm' => 60,
        'h' => 3_600,
        'd' => 86_400,
        _ => return None,
    };
    let number = word.get(..word.len() - 1)?;
    if number.is_empty() || !number.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    number.parse::<u64>().ok()?.checked_mul(unit)
}

/// 跳过开头 `count` 个词以后剩下的原文，去掉前后空白。
fn after(text: &str, count: usize) -> &str {
    let mut rest = text.trim_start();
    for _ in 0..count {
        rest = rest
            .split_once(char::is_whitespace)
            .map_or("", |(_, rest)| rest.trim_start());
    }
    rest.trim_end()
}

/// 去掉末尾 `count` 个词以后剩下的原文，去掉前后空白。
fn before(text: &str, count: usize) -> &str {
    let mut rest = text.trim_end();
    for _ in 0..count {
        rest = rest
            .rsplit_once(char::is_whitespace)
            .map_or("", |(rest, _)| rest.trim_end());
    }
    rest.trim_start()
}

#[cfg(test)]
mod tests;
