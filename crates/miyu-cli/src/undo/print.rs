//! 照核心交回的几样写成一行行给人看的（样子是项目主人 2026-09-28 定的，`docs/construction/4-7-miyu undo、miyu
//! redo（下）.md`）：第一行说撤的是哪一轮，撤掉了压缩的说一句上下文回到了压缩前（施工 6-9），撤掉了清空的说一句回到了
//! 清空以前（施工 6-8 补），停掉了那几轮派出去的任务的说一句停掉了几个（施工 7-8），撤销的最后说怎么恢复。只管写成什么样，
//! 不管往哪写。施工 U-1 起撤销不再改回文件，不提文件。
//!
//! `miyu redo` 也照这几行印撤掉了哪一轮（施工 4-7 再补，`docs/blueprint/cli/redo.md`）：第一行接「，重新做」，不说怎么恢复。

use serde_json::Value;

use miyu_store::human::clean;

use super::{Direction, UndoPlan};
use crate::shown::{Line, cut};

/// 那一轮人说的话最多印几个字。
const SAID_CHARS: usize = 40;

/// 核心交回的 `result` 写成的几行。
pub(super) fn lines(result: &Value, plan: &UndoPlan) -> Vec<Line> {
    let header = plan
        .language
        .undo_header(plan.direction, said(result).as_deref(), turns(result));
    let mut lines = vec![Line::gray(header)];
    lines.extend(body_texts(result, plan).into_iter().map(Line::gray));
    if plan.direction == Direction::Undo {
        lines.push(Line::gray(plan.language.restore_hint()));
    }
    lines
}

/// 重做的回应写成的几行（施工 4-7 再补）：第一行照撤销那一行接「，重新做」，接着照撤销印，不说怎么恢复：重做以后恢复不了。
/// `plan` 照撤销的写（[`Direction::Undo`]）：差异的头一行对照的是她改完的。
pub(crate) fn redo_lines(result: &Value, plan: &UndoPlan) -> Vec<Line> {
    let mut lines = vec![Line::gray(
        plan.language.redo_header(said(result).as_deref()),
    )];
    // 重做接着是这一轮的过程记录：中间那几句照过程行写，带「·」（设计 47，项目主人 10-29）。
    lines.extend(
        body_texts(result, plan)
            .into_iter()
            .map(|text| Line::gray(format!("· {text}"))),
    );
    lines
}

/// 那一轮人说的话：控制字符换掉，太长的截掉。
fn said(result: &Value) -> Option<String> {
    result["said"]
        .as_str()
        .map(|said| cut(&clean(said), SAID_CHARS))
}

/// 撤了几轮，回应里没有的当 1。
fn turns(result: &Value) -> usize {
    result["turns"]
        .as_u64()
        .and_then(|turns| usize::try_from(turns).ok())
        .unwrap_or(1)
}

/// 第一行和最后一行中间的：撤掉了压缩、清空的那两句，停掉了几个任务的那一句。
fn body_texts(result: &Value, plan: &UndoPlan) -> Vec<String> {
    let language = &plan.language;
    let mut lines = Vec::new();
    // 撤掉了几次压缩、几次清空，核心撤销时才交，是 0 的不交：几次都说同一句，两样都有的先压缩后清空；恢复不说（施工 6-9、
    // 6-8 补）。
    let undone = |field: &str| result[field].as_u64().is_some_and(|count| count > 0);
    if plan.direction == Direction::Undo {
        if undone("compactions") {
            lines.push(language.compaction_undone().to_string());
        }
        if undone("clears") {
            lines.push(language.clear_undone().to_string());
        }
        // 停掉了几个任务，核心撤销时才交，没有的不交（施工 7-8）：只说几个，是哪几个在回应里。
        let stopped = result["jobs"].as_array().map_or(0, Vec::len);
        if stopped > 0 {
            lines.push(language.jobs_stopped(stopped));
        }
    }
    lines
}
