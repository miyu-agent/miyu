//! 一个编辑器里开着的会话（`internal/blueprint/cli/acp.md`「一个会话记着什么」）：编辑器里的会话就是 Miyu 的会话，编号同一个
//! （设计 73 第五节）。这里只记翻译要的：报过的每一条、等着回的那一句、问过编辑器的确认、上一次报的待办和用量。

use std::collections::{HashMap, HashSet};

use serde_json::Value;

use miyu_view::{End, Tool};

/// 一个会话。
pub(crate) struct Session {
    /// 编辑器给的工作目录（绝对路径）：工具参数里的相对路径照它补全了再报（规范要绝对路径）。
    pub(crate) cwd: String,
    /// 见过的条目：编号到报到哪了。
    pub(crate) known: HashMap<String, Known>,
    /// 等着回的 `session/prompt`。
    pub(crate) prompt: Option<Prompt>,
    /// 编辑器自己发来的那几句（`m<序号>`）：编辑器自己画了，不报回去。
    pub(crate) own: HashSet<String>,
    /// 问过编辑器、还没了结的确认和提问（[`waiting_key`]）。
    pub(crate) asked: HashSet<String>,
    /// 上一次报给编辑器的几样：一样的不再报。
    pub(crate) reported: Reported,
    /// 会话这时的权限、模型、思考档位：会话配置项照它写（`options.rs`）。
    pub(crate) current: Current,
    /// 这个会话认的斜杠命令（`command.catalog` 的 `commands`）。
    pub(crate) commands: Vec<Value>,
    /// 编辑器里已经有开头的几轮（`turn` 写成字）：人说的话、编辑器自己发的那一句开的。别的一轮（后台任务报回来叫醒的、
    /// 循环到点的）头一次报内容以前先报一句开头，编辑器才另起一条消息。
    pub(crate) introduced: HashSet<String>,
    /// 最近一件结束了、还没叫醒哪一轮的后台任务：标题、怎么结束的（`done`、`failed`、`stopped`）。
    pub(crate) finished_job: Option<(String, String)>,
}

/// 一条条目报到哪了。
pub(crate) enum Known {
    /// 正文、思考：报的是哪一种（`agent_message_chunk`、`agent_thought_chunk`）、报过的字。
    Text {
        /// 哪一种。
        kind: &'static str,
        /// 报过的字。
        sent: String,
    },
    /// 一步工具：报过的最后一份。
    Tool(Box<Tool>),
    /// 别的（人说的话、时间线的一段、收尾、旁白）：只记见过。
    Other,
}

/// 等着回的 `session/prompt`。
pub(crate) struct Prompt {
    /// 编辑器那条请求的编号。
    pub(crate) id: Value,
    /// 等什么结束。
    pub(crate) waiting: Waiting,
    /// 收到过 `session/cancel`：结束了一律回 `cancelled`（规范的要求）。
    pub(crate) cancelled: bool,
}

/// 等什么结束才回 `session/prompt`。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Waiting {
    /// 说的那一句（`m<序号>`）：她听到了就知道是哪一轮。
    Message(String),
    /// 这一轮（`turn.started` 的序号）。
    Turn(Value),
    /// 一条斜杠命令追加的这几条事件：引出的那一句、开的那一轮里有一个结束了就回；都没有的，命令一回就回。
    Command(Vec<Value>),
}

/// 上一次报给编辑器的几样。
#[derive(Default)]
pub(crate) struct Reported {
    /// 待办（`plan` 的 `entries`）。
    pub(crate) plan: Option<Value>,
    /// 用量（`usage_update`）。
    pub(crate) usage: Option<Value>,
    /// 标题。
    pub(crate) title: Option<String>,
}

/// 会话这时的权限、模型、思考档位，照会话状态读。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Current {
    /// 模式的编号：`full`、`read-only`（`options.rs`）。
    pub(crate) mode: String,
    /// 模型或 `@池`；跟着配置、还不知道的没有。
    pub(crate) model: Option<String>,
    /// 思考的一档；不带思考的没有。
    pub(crate) effort: Option<String>,
}

/// 这一轮结束了，回等着的 `session/prompt` 的那一句。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Stop {
    /// 正常回：`stopReason`。
    Reason(&'static str),
    /// 出错：照 JSON-RPC 的错误回，话照连接的语言。
    Failed(String),
}

impl Session {
    /// 一个刚开、刚接上的会话：什么都还没报过。会话编号是 [`crate::acp::server::Server`] 里那张表的键。
    pub(crate) fn new(cwd: &str) -> Session {
        Session {
            cwd: cwd.to_string(),
            known: HashMap::new(),
            prompt: None,
            own: HashSet::new(),
            asked: HashSet::new(),
            reported: Reported::default(),
            current: Current {
                mode: crate::acp::options::FULL.to_string(),
                ..Current::default()
            },
            commands: Vec::new(),
            introduced: HashSet::new(),
            finished_job: None,
        }
    }

    /// 编辑器发的一句话（`id`）出现了，`turn` 是它所属的一轮（还排着的没有）：等的是引出它的那条斜杠命令的，换成等这一句；
    /// 等的是这一句、它有了所属的一轮的，换成等这一轮。
    pub(crate) fn heard(&mut self, id: &str, turn: Option<&Value>) {
        let Some(prompt) = &mut self.prompt else {
            return;
        };
        if let Waiting::Command(events) = &prompt.waiting
            && events.iter().any(|seq| format!("m{seq}") == id)
        {
            prompt.waiting = Waiting::Message(id.to_string());
        }
        if let (Waiting::Message(waited), Some(turn)) = (&prompt.waiting, turn)
            && waited == id
        {
            prompt.waiting = Waiting::Turn(turn.clone());
        }
    }

    /// 一轮结束了（`end` 那一条，属于 `turn`）：等的是这一轮的，交回编辑器那条请求的编号和怎么回。
    pub(crate) fn ended(&mut self, end: &End, turn: Option<&Value>) -> Option<(Value, Stop)> {
        let prompt = self.prompt.as_ref()?;
        let ours = match (&prompt.waiting, turn) {
            (Waiting::Turn(waited), Some(turn)) => waited == turn,
            (Waiting::Command(events), Some(turn)) => events.contains(turn),
            _ => false,
        };
        if !ours {
            return None;
        }
        let prompt = self.prompt.take()?;
        Some((prompt.id, stop(end, prompt.cancelled)))
    }
}

/// 一轮怎么结束的写成 `stopReason`（设计 73 第五节第 3 条）：正常结束 `end_turn`，到了轮数上限 `max_turn_requests`，被拒
/// `refusal`，撞到输出上限 `max_tokens`，打断 `cancelled`；出错的照 JSON-RPC 的错误回。收到过 `session/cancel` 的一律
/// `cancelled`：打断那一下也可能让这一轮以出错收尾，规范要求不当错误回。
pub(crate) fn stop(end: &End, cancelled: bool) -> Stop {
    use miyu_kernel::event::{EndReason, ErrorClass};
    if cancelled {
        return Stop::Reason("cancelled");
    }
    match &end.reason {
        EndReason::Completed => Stop::Reason("end_turn"),
        EndReason::Interrupted => Stop::Reason("cancelled"),
        EndReason::StepLimit => Stop::Reason("max_turn_requests"),
        _ => match end.error.as_ref().map(|error| &error.class) {
            Some(ErrorClass::ContentPolicy) => Stop::Reason("refusal"),
            Some(ErrorClass::OutputLimit) => Stop::Reason("max_tokens"),
            _ => Stop::Failed(
                end.explain
                    .clone()
                    .or_else(|| end.error.as_ref().map(|error| error.message.clone()))
                    .unwrap_or_else(|| end.reason.as_str().to_string()),
            ),
        },
    }
}

/// 一件在等人的事认作哪一件：调用编号，脚本里调的那一件再带序号。同一件问过编辑器的不再问。
pub(crate) fn waiting_key(waiting: &miyu_view::Waiting) -> String {
    match waiting.nested {
        Some(nested) => format!("{}#{nested}", waiting.call),
        None => waiting.call.to_string(),
    }
}
