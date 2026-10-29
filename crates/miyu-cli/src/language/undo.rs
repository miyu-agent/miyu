//! `miyu undo`、`miyu restore` 给人看的字（施工 4-7 下，样子是项目主人 2026-09-28 定的）；`miyu redo` 的第一行（施工 4-7 再补）。

use super::Language;
use crate::undo::Direction;

impl Language {
    /// 第一行：撤的（恢复的）是哪一轮（设计 47：结果行，动词打头、不带「·」）。`said` 是那一轮人说的那句话，没有的写「最后一轮」；
    /// 几轮的写几轮。
    pub(crate) fn undo_header(
        &self,
        direction: Direction,
        said: Option<&str>,
        turns: usize,
    ) -> String {
        match (self, direction, said, turns) {
            (Language::Chinese, Direction::Undo, Some(said), 0 | 1) => {
                format!("已撤销「{said}」这一轮")
            }
            (Language::Chinese, Direction::Undo, Some(said), turns) => {
                format!("已撤销「{said}」起的 {turns} 轮")
            }
            (Language::Chinese, Direction::Undo, None, _) => "已撤销最后一轮".to_string(),
            (Language::Chinese, Direction::Restore, Some(said), 0 | 1) => {
                format!("已恢复「{said}」这一轮")
            }
            (Language::Chinese, Direction::Restore, Some(said), turns) => {
                format!("已恢复「{said}」起的 {turns} 轮")
            }
            (Language::Chinese, Direction::Restore, None, _) => "已恢复撤销的那一轮".to_string(),
            (Language::English, Direction::Undo, Some(said), 0 | 1) => {
                format!("undid the turn \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Undo, Some(said), turns) => {
                format!("undid {turns} turns from \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Undo, None, _) => "undid the last turn".to_string(),
            (Language::English, Direction::Restore, Some(said), 0 | 1) => {
                format!("restored the turn \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Restore, Some(said), turns) => {
                format!("restored {turns} turns from \u{201c}{said}\u{201d}")
            }
            (Language::English, Direction::Restore, None, _) => {
                "restored the undone turn".to_string()
            }
        }
    }

    /// `miyu redo` 的第一行（施工 4-7 再补，`docs/blueprint/cli/redo.md`）：照撤销一轮的那一行，接「，重新做」。`said` 是撤掉的
    /// 那一轮原来那一句，没有的写「最后一轮」。
    pub(crate) fn redo_header(&self, said: Option<&str>) -> String {
        match (self, said) {
            (Language::Chinese, Some(said)) => format!("· 已撤销「{said}」这一轮，重新做"),
            (Language::Chinese, None) => "· 已撤销最后一轮，重新做".to_string(),
            (Language::English, Some(said)) => {
                format!("· Undid the turn \u{201c}{said}\u{201d}, redoing it")
            }
            (Language::English, None) => "· Undid the last turn, redoing it".to_string(),
        }
    }

    /// 撤掉了压缩的那一句（施工 6-9，`compaction.md`「给人看的字」）：撤掉一次、几次都是这一句（2026-09-29 项目主人定）。
    pub(crate) fn compaction_undone(&self) -> &'static str {
        match self {
            Language::Chinese => "已撤销压缩，上下文恢复到压缩前",
            Language::English => "undid the compaction; the context is as it was before",
        }
    }

    /// 撤掉了清空的那一句（施工 6-8 补，`compaction.md` 第十四条）：撤掉一次、几次都是这一句（2026-09-30 项目主人定）。
    pub(crate) fn clear_undone(&self) -> &'static str {
        match self {
            Language::Chinese => "已撤销清空，上下文恢复到清空前",
            Language::English => "undid the clear; the context is as it was before",
        }
    }

    /// 撤销停掉了那几轮派出去的任务的那一句（施工 7-8，`agents.md` 第七条第 1 条）：`count` 是停掉了几个。
    pub(crate) fn jobs_stopped(&self, count: usize) -> String {
        match (self, count) {
            (Language::Chinese, count) => format!("已停止 {count} 个任务"),
            (Language::English, 1) => "stopped 1 job".to_string(),
            (Language::English, count) => format!("stopped {count} jobs"),
        }
    }

    /// 撤销的最后一行：怎么恢复（设计 47：建议另起一行）。
    pub(crate) fn restore_hint(&self) -> &'static str {
        match self {
            Language::Chinese => "提示：发送下一句之前，可以运行 'miyu restore' 恢复",
            Language::English => {
                "hint: run 'miyu restore' before sending anything else to bring it back"
            }
        }
    }
}
