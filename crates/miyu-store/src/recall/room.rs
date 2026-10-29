//! 一处记忆（「房间」，施工 R-3 下，`docs/blueprint/memory.md`「范围」）：一个范围的记忆日志、回合库、记忆库放在哪。
//!
//! - 跟着人格的：`modules/memory/<人格>/`、`index/recall/turns-<人格>.db`、`index/recall/memory-<人格>.db`。
//!   无人格的会话（施工 P-12，2026-10-17 项目主人定）也跟着「人格」走，编号写 [`NO_PERSONA`]：单独一间，无人格的会话
//!   共用它。无人格不是人格，人格列表里没有它，也改不了。
//! - 只在这个会话里的：会话自己的目录 `sessions/<会话>/memory/` 下，`log/`、`turns.db`、`memory.db`。删会话、进回收处、恢复
//!   都跟着目录走，不另写一行。

use std::fmt;
use std::path::PathBuf;

use miyu_kernel::id::SessionId;

use crate::root::DataRoot;

/// 无人格的那一间的编号（施工 P-12）：人格的编号以小写字母开头，`_` 开头的撞不上任何人格。
pub const NO_PERSONA: &str = "_none";

/// 回合库文件名的前后：`turns-<人格>.db`。
pub(crate) const TURNS_PREFIX: &str = "turns-";
/// 库文件的后缀。
pub(crate) const DB: &str = ".db";

/// 一处记忆。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Room {
    /// 跟着人格：这个人格的。
    Persona {
        /// 人格的编号。
        persona: String,
    },
    /// 只在这个会话里。
    Session {
        /// 会话编号。
        session: SessionId,
    },
}

/// 整串的样子：`persona <人格>`、`session <会话>`。带人格名，不写进运行日志（施工 LG-1 下：日志照
/// [`Room::kind`] 和 [`super::LogSalt::room_id`]）；短号照它算。
impl fmt::Display for Room {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Room::Persona { persona } => write!(f, "persona {persona}"),
            Room::Session { session } => write!(f, "session {session}"),
        }
    }
}

impl Room {
    /// 人格 `persona` 的那一间。
    pub fn persona(persona: &str) -> Room {
        Room::Persona {
            persona: persona.to_string(),
        }
    }

    /// 会话 `session` 自己的那一间。
    pub fn session(session: &SessionId) -> Room {
        Room::Session {
            session: session.clone(),
        }
    }

    /// 运行日志里写的种类（施工 LG-1 下）：`persona`、`session`。人格名、会话编号不写，照 [`super::LogSalt::room_id`]
    /// 写短号。
    pub fn kind(&self) -> &'static str {
        match self {
            Room::Persona { .. } => "persona",
            Room::Session { .. } => "session",
        }
    }

    /// 回合库在哪。
    pub(crate) fn turns(&self, root: &DataRoot) -> PathBuf {
        match self {
            Room::Persona { persona } => {
                recall_dir(root).join(format!("{TURNS_PREFIX}{persona}{DB}"))
            }
            Room::Session { .. } => self.session_dir(root).join(format!("turns{DB}")),
        }
    }

    /// 记忆库在哪。
    pub(crate) fn memories(&self, root: &DataRoot) -> PathBuf {
        match self {
            Room::Persona { persona } => recall_dir(root).join(format!("memory-{persona}{DB}")),
            Room::Session { .. } => self.session_dir(root).join(format!("memory{DB}")),
        }
    }

    /// 记忆日志在哪。
    pub(crate) fn log(&self, root: &DataRoot) -> PathBuf {
        match self {
            Room::Persona { persona } => root.path().join("modules").join("memory").join(persona),
            Room::Session { .. } => self.session_dir(root).join("log"),
        }
    }

    /// 会话那一间的目录。
    fn session_dir(&self, root: &DataRoot) -> PathBuf {
        match self {
            Room::Session { session } => root.session_dir(session).join("memory"),
            Room::Persona { .. } => root.path().to_path_buf(),
        }
    }
}

/// 放跟着人格的那几间检索库的目录。
pub(crate) fn recall_dir(root: &DataRoot) -> PathBuf {
    root.index().join("recall")
}
