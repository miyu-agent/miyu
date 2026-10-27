//! 索引里的一行（施工 3-8 七补，`docs/blueprint/store/index.md`「一行记什么」）：一个会话照日志算出来的几样，和它照到日志的
//! 哪里。会话写日志时一批批往上盖（[`super::SessionIndex::advance`]），列会话时照日志补（`miyu-endpoint` 的 `list.rs`）：两处
//! 照同一个 [`Row::see`] 算，结果才和从头读整份日志的一字不差。

use rusqlite::Row as SqlRow;

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Dream, DreamEnded, Event};
use miyu_kernel::id::{AccountId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::time::Timestamp;

use super::IndexError;
use crate::log::Mark;

/// 一个会话在索引里的一行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// 会话编号。
    pub id: SessionId,
    /// 属主：`session.created` 的 `owner`。
    pub owner: AccountId,
    /// 父会话：子会话才有。
    pub parent: Option<SessionId>,
    /// 一次性的：`miyu ask` 开的。
    pub oneshot: bool,
    /// 标题：空的是没有。
    pub title: String,
    /// 标题是人起的（施工 3-8 八补，终端要的）：最后一条写了标题的 `session.meta_changed` 的 `by` 是人。内核起的、没有标题的是
    /// `false`。
    pub named: bool,
    /// 置顶。
    pub pinned: bool,
    /// 工作目录：日志里最后一条带 `cwd` 的（[`cwd`]），很早以前的日志一条都没有。
    pub cwd: Option<String>,
    /// 造的时刻：`session.created` 的 `at`。
    pub created: Timestamp,
    /// 最近一次动静：照到的最后一条事件的 `at`。
    pub last_active: Timestamp,
    /// 照到日志的哪里。
    pub mark: Mark,
    /// 第一句话的第一行（施工 9-5）：第一条 `message.user` 的文字，去掉前后空白，最多 50 个字（`PREVIEW_CHARS`）；还没人说过话、
    /// 第一句没有字的是空的。没有标题的会话，头照它显示。
    pub preview: String,
    /// 场所（施工 O-3）：`session.created` 的 `venue`。本机的是 `local`；通讯平台的场所会话照它找回，列会话时跳过。
    pub venue: String,
    /// 人格（施工 P-1 下）：`session.created` 的 `persona`。以前的日志没有的是空的。
    pub persona: Option<String>,
    /// 预设（施工 P-2 上）：`session.created` 的 `preset`。以前的日志没有的是空的。
    pub preset: Option<String>,
    /// 在哪个项目（施工 H-2）：最后一条写了 `project` 的 `session.meta_changed`，移出来了、没进过的是空的。
    pub project: Option<String>,
    /// 这个会话自己的模型调用一共用了多少 token：每条带用量的 `model.called`（主请求、起标题、压缩这些都算）的输入（没命中、
    /// 缓存读、缓存写）加输出。终端底栏的 `Σ` 同一个算法；子代理的另算在它们自己的行里，列会话时加上去。
    pub tokens: u64,
    /// 上下文用了多少 token（施工 9-5 再补，网页的会话列表要的）：最近一次带用量的主请求（不是压缩、起标题这些）的输入，同
    /// 终端底栏、会话状态的 `used`；压缩以后、还没有请求的没有。
    pub context: Option<u64>,
    /// 那一次请求的模型：`<供应商>/<模型>`。列会话时照它查窗口。
    pub context_model: Option<String>,
    /// 整理会话的（施工 D-1 下，设计 41）：`session.created` 的 `dream`，哪一间、怎么来的。别的会话没有。表里存成它的 JSON。
    pub dream: Option<Dream>,
    /// 从哪开的（施工 8-33 四补）：`session.created` 的 `origin`，终端集成开的是 `terminal`。别的会话没有。
    pub origin: Option<String>,
    /// fork 子代理（施工 A-F 补）：`session.created` 的 `forked` 写了 `within` 的。头照它换图标。
    pub forked: bool,
    /// 整理会话这一趟做了什么（施工 DR-1）：最后一条 `dream.ended`。没跑完、崩了没记上的、别的会话没有。表里存成它的 JSON。
    pub dream_result: Option<DreamEnded>,
}

/// `preview` 最多几个字（按 Unicode 的字数）。
const PREVIEW_CHARS: usize = 50;

impl Row {
    /// 照日志的第一条起一行：`session.created` 的属主、父会话、是不是一次性的、工作目录，时刻既是造的时刻也是最近一次
    /// 动静。照到哪里先记成第一段的开头，调的一方读完再换。第一条不是 `session.created` 的没有。
    pub fn new(id: SessionId, first: &Event) -> Option<Row> {
        let Body::SessionCreated(created) = &first.body else {
            return None;
        };
        Some(Row {
            id,
            owner: created.owner.clone(),
            parent: created.parent.clone(),
            oneshot: created.oneshot,
            title: String::new(),
            named: false,
            pinned: false,
            cwd: created.cwd.clone(),
            created: first.at,
            last_active: first.at,
            mark: Mark {
                segment: 1,
                bytes: 0,
                next: first.seq,
            },
            preview: String::new(),
            venue: created.venue.as_str().to_string(),
            persona: created.persona.clone(),
            // 明着无工具的记成空的编号，同换预设时内核记的（施工 P-16 下）。
            preset: match created.no_tools {
                true => Some(String::new()),
                false => created.preset.clone(),
            },
            project: None,
            tokens: 0,
            context: None,
            context_model: None,
            dream: created.dream.clone(),
            origin: created.origin.clone(),
            forked: created
                .forked
                .as_ref()
                .is_some_and(|forked| forked.within.is_some()),
            dream_result: None,
        })
    }

    /// 照先后看一条（`protocol.md` 的 `session.list` 第 3、4 条）：它的时刻是最近一次动静；带 `cwd` 的盖上工作目录；
    /// `session.meta_changed` 写了 `title` 的换成它（空的是去掉），`by` 是人、又不是去掉的算人起的；写了 `pinned` 的换成它；写了 `project` 的换成它（空的是移出来，施工 H-2）。
    /// 撤掉的回合里改的也算：改名不是对话的一部分。`policy.changed` 写了 `persona`、`preset` 的换成它（中途换人格、预设，施工
    /// P-13；空的人格是无人格）：人换了就算，不等下一个回合真换上。
    pub fn see(&mut self, event: &Event) {
        // 脚本里调的工具（施工 M-1）不算动静：不是人和她的对话，后台任务的会在回合外落盘，会把终端会话的闲置往后推。
        if !matches!(event.body, Body::ToolNested(_)) {
            self.last_active = event.at;
        }
        if let Some(cwd) = cwd(event) {
            self.cwd = Some(cwd.to_string());
        }
        // 只在空会话里 `/goal` 开的（施工 X-8 补修）：第一句是续轮消息，开头是 `<goal_round>` 标签，照目标原文。
        if let Body::GoalSet(set) = &event.body
            && self.preview.is_empty()
        {
            self.preview = first_line(&set.text);
        }
        if let Body::MessageUser(message) = &event.body
            && self.preview.is_empty()
        {
            // 斜杠命令模板展开成的照人敲的那一行（施工 X-14 上）。
            self.preview = match message.typed() {
                Some(typed) => first_line(typed),
                None => preview(&message.blocks),
            };
        }
        if let Body::ModelCalled(called) = &event.body
            && let Some(usage) = &called.usage
        {
            self.tokens = self.tokens.saturating_add(usage.total());
            if !called.aside() && called.compaction.is_none() {
                self.context = Some(usage.input());
                self.context_model = match (&called.endpoint, &called.model) {
                    (Some(endpoint), Some(model)) => Some(format!("{endpoint}/{model}")),
                    _ => None,
                };
            }
        }
        if let Body::DreamEnded(ended) = &event.body {
            self.dream_result = Some(*ended);
        }
        if let Body::ContextCompacted(_) = &event.body {
            self.context = None;
        }
        if let Body::PolicyChanged(changed) = &event.body {
            if let Some(persona) = &changed.persona {
                self.persona = Some(persona.clone()).filter(|persona| !persona.is_empty());
            }
            if let Some(preset) = &changed.preset {
                self.preset = Some(preset.clone());
            }
        }
        if let Body::MetaChanged(changed) = &event.body {
            if let Some(title) = &changed.title {
                self.title.clone_from(title);
                self.named = !title.is_empty() && matches!(event.by, By::Person(_));
            }
            self.pinned = changed.pinned.unwrap_or(self.pinned);
            if let Some(project) = &changed.project {
                self.project = Some(project.clone()).filter(|project| !project.is_empty());
            }
        }
    }

    /// 从表里读出来的一行，列的先后照 [`super::COLUMNS`]。
    pub(super) fn from_sql(row: &SqlRow<'_>) -> Result<Row, IndexError> {
        let id: String = row.get(0)?;
        let bad = |what: &str| IndexError::Bad(format!("row {id}: {what}"));
        let owner: String = row.get(1)?;
        let parent: Option<String> = row.get(2)?;
        let parent = match parent {
            Some(parent) => Some(SessionId::parse(&parent).map_err(|_| bad("parent"))?),
            None => None,
        };
        let millis = |at: i64| Timestamp::from_unix_millis(at).ok_or_else(|| bad("time"));
        let number = |n: i64| u64::try_from(n).map_err(|_| bad("mark"));
        let next: i64 = row.get(11)?;
        Ok(Row {
            id: SessionId::parse(&id).map_err(|_| bad("id"))?,
            owner: AccountId::parse(&owner).map_err(|_| bad("owner"))?,
            parent,
            oneshot: row.get(3)?,
            title: row.get(4)?,
            pinned: row.get(5)?,
            cwd: row.get(6)?,
            created: millis(row.get(7)?)?,
            last_active: millis(row.get(8)?)?,
            mark: Mark {
                segment: number(row.get(9)?)?,
                bytes: number(row.get(10)?)?,
                next: miyu_kernel::id::Seq::new(number(next)?).ok_or_else(|| bad("mark"))?,
            },
            preview: row.get(12)?,
            venue: row.get(13)?,
            persona: row.get(14)?,
            preset: row.get(15)?,
            named: row.get(16)?,
            project: row.get(17)?,
            tokens: u64::try_from(row.get::<_, i64>(18)?).map_err(|_| bad("tokens"))?,
            context: row
                .get::<_, Option<i64>>(19)?
                .map(|n| u64::try_from(n).map_err(|_| bad("context")))
                .transpose()?,
            context_model: row.get(20)?,
            dream: row
                .get::<_, Option<String>>(21)?
                .map(|text| serde_json::from_str(&text).map_err(|_| bad("dream")))
                .transpose()?,
            origin: row.get(22)?,
            forked: row.get::<_, i64>(23)? != 0,
            dream_result: row
                .get::<_, Option<String>>(24)?
                .map(|text| serde_json::from_str(&text).map_err(|_| bad("dream_result")))
                .transpose()?,
        })
    }

    /// 写进表里的几格，先后照 [`super::COLUMNS`]。数字超过 SQLite 的整数（`i64`）的报错：一段不会有那么长。
    pub(super) fn to_sql(&self) -> Result<[rusqlite::types::Value; 25], IndexError> {
        use rusqlite::types::Value;
        let number = |n: u64| {
            i64::try_from(n)
                .map(Value::Integer)
                .map_err(|_| IndexError::Bad(format!("row {}: mark too large", self.id.as_str())))
        };
        let text = |s: &str| Value::Text(s.to_string());
        let optional = |s: Option<&str>| s.map_or(Value::Null, text);
        Ok([
            text(self.id.as_str()),
            text(self.owner.as_str()),
            optional(self.parent.as_ref().map(SessionId::as_str)),
            Value::Integer(i64::from(self.oneshot)),
            text(&self.title),
            Value::Integer(i64::from(self.pinned)),
            optional(self.cwd.as_deref()),
            Value::Integer(self.created.unix_millis()),
            Value::Integer(self.last_active.unix_millis()),
            number(self.mark.segment)?,
            number(self.mark.bytes)?,
            number(self.mark.next.get())?,
            text(&self.preview),
            text(&self.venue),
            optional(self.persona.as_deref()),
            optional(self.preset.as_deref()),
            Value::Integer(i64::from(self.named)),
            optional(self.project.as_deref()),
            number(self.tokens)?,
            self.context.map_or(Ok(Value::Null), number)?,
            optional(self.context_model.as_deref()),
            match &self.dream {
                Some(dream) => text(&serde_json::to_string(dream).map_err(|error| {
                    IndexError::Bad(format!("row {}: dream: {error}", self.id.as_str()))
                })?),
                None => Value::Null,
            },
            optional(self.origin.as_deref()),
            Value::Integer(i64::from(self.forked)),
            match &self.dream_result {
                Some(result) => text(&serde_json::to_string(result).map_err(|error| {
                    IndexError::Bad(format!("row {}: dream_result: {error}", self.id.as_str()))
                })?),
                None => Value::Null,
            },
        ])
    }
}

/// 这一条记下的工作目录（`protocol.md`「会话表」第 5 条）：带 `cwd` 的 `turn.started`、`session.created`，换工作区的
/// `session.workspace_changed`（施工 9-7 上）。会话表载入时、列会话时
/// 都照日志里最后一条带它的算，同一个认法。
pub fn cwd(event: &Event) -> Option<&str> {
    match &event.body {
        Body::TurnStarted(started) => started.cwd.as_deref(),
        Body::SessionCreated(created) => created.cwd.as_deref(),
        Body::WorkspaceChanged(changed) => Some(changed.cwd.as_str()),
        _ => None,
    }
}

/// 一句话的预览（施工 9-5）：头一块文字的第一行，去掉前后空白，最多 [`PREVIEW_CHARS`] 个字。没有文字的是空的。
fn preview(blocks: &[Block]) -> String {
    blocks
        .iter()
        .find_map(|block| match block {
            Block::Text(text) => Some(first_line(&text.text)),
            _ => None,
        })
        .unwrap_or_default()
}

/// `text` 第一行不是空白的，截到 [`PREVIEW_CHARS`] 个字；都是空白的是空的。
fn first_line(text: &str) -> String {
    text.lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| line.chars().take(PREVIEW_CHARS).collect())
        .unwrap_or_default()
}
