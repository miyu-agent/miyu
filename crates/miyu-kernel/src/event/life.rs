//! 人生会话的身份（施工 LF-5 中，设计 `70-推演人生.md` 第三节第 1 条，`docs/blueprint/life.md`）：`life.start` 在新开的会话里记
//! 一条，说「这是人生会话」。真相只在日志里：定时表那一行只管下次几点醒，表坏了换成空的以后照这一条接上；会话索引照它派生
//! 那一格。人格写在 `session.created` 里，这里不另记。不进她的上下文。

use serde::{Deserialize, Serialize};

/// `life.started`：这个会话是它的人格的人生会话。`by` 是开它的人。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LifeStarted {}
