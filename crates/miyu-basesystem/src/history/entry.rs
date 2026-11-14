//! 哪些算一条、一条的原文怎么写（`docs/blueprint/tools/history.md`「哪些算一条」「一条的原文」，施工 6-4）。
//!
//! 撤掉的、撤回的交给内核的 `History::whole()` 算过（边读边算，`collect.rs`），这里一条条看剩下的：人说的话、她的回复、工具
//! 结果、以前的摘要。她自己翻记录的那几步（`history` 的调用和结果）不算：找的时候会找到自己这一次调用，翻出来的旧结果又和原文
//! 重复。
//!
//! 人切权限级别另算（[`switched`]，施工 2-7 补）：撤销不改现在的权限（`docs/blueprint/kernel/session.md`「载入和崩溃」
//! 第 2 条），所以撤掉的回合里切的也列，不经 `History::whole()`。

use miyu_kernel::block::Block;
use miyu_kernel::event::{Body, Event, PolicyChanged, VenueMessage};
use miyu_kernel::facts::effective_level;
use miyu_kernel::id::SessionId;
use miyu_kernel::origin::{By, Role};
use miyu_kernel::template::{Template, escape};
use miyu_kernel::time::Timestamp;

use crate::load::say;

/// 一条是谁说的：参数 `by` 的三种写法。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Who {
    /// 人说的话。
    User,
    /// 她的回复、以前的摘要。
    Assistant,
    /// 工具结果。
    Tool,
}

impl Who {
    /// 参数里、输出里的写法。
    pub(super) fn name(self) -> &'static str {
        match self {
            Who::User => "user",
            Who::Assistant => "assistant",
            Who::Tool => "tool",
        }
    }

    /// 读参数 `by`；不是这三种的没有。
    pub(super) fn parse(text: &str) -> Option<Who> {
        [Who::User, Who::Assistant, Who::Tool]
            .into_iter()
            .find(|who| who.name() == text)
    }
}

/// 一条：序号、时刻、谁说的、原文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Entry {
    /// 日志的序号。
    pub(super) seq: u64,
    /// 写下的时刻。
    pub(super) at: Timestamp,
    /// 谁说的：筛的时候照它。
    pub(super) who: Who,
    /// 「谁」那一格另写的：别的 harness 发来的话写 `agent "<名字>"`（施工 7-10），别的会话发来的话写 `session <短编号>`
    /// （施工 C-2），场所会话里人说的话写发的人（施工 O-42）；没有的照 `who` 写。
    pub(super) from: Option<String>,
    /// 「读」时这一条下面的原文；「找」也比它。
    pub(super) text: String,
}

impl Entry {
    /// 「谁」那一格：别的 harness 发来的写它的名字，别的会话发来的写它的短编号，别的写 `by` 的那三种写法。
    pub(super) fn said_by(&self) -> &str {
        self.from.as_deref().unwrap_or(self.who.name())
    }
}

/// 一条里代码写的几样：图片、文件的占位（`history/image.txt`、`history/file.txt`），别的 harness 发来的那一条的「谁」
/// （`history/agent.txt`，施工 7-10），别的会话发来的那一条的「谁」（`history/session.txt`，施工 C-2），人切了权限级别的
/// 那一条的原文（`history/permission.txt`，施工 2-7 补）。
#[derive(Clone)]
pub(super) struct Placeholders {
    pub(super) image: Template,
    pub(super) file: Template,
    pub(super) agent: Template,
    pub(super) session: Template,
    pub(super) permission: Template,
}

/// 一条还算数的事件要是算一条的种类（人说的话、她的回复、工具结果、以前的摘要）：谁说的、显示的时刻（平台给了时刻的照它，补拉
/// 回来的比记下的早，施工 O-40）。别的种类没有。算不算还要看原文有没有字（[`text_of`]），工具结果是她自己翻记录的那几步的
/// 不算（照调用编号认，`collect.rs`）。
pub(super) fn kind_of(event: &Event) -> Option<(Who, Timestamp)> {
    let who = match &event.body {
        Body::MessageUser(message) => {
            let at = message.venue.as_ref().and_then(|venue| venue.sent);
            return Some((Who::User, at.unwrap_or(event.at)));
        }
        Body::MessageAssistant(_) | Body::ContextCompacted(_) => Who::Assistant,
        Body::ToolResult(_) => Who::Tool,
        _ => return None,
    };
    Some((who, event.at))
}

/// 一条的原文，末尾的空白去掉；一个字都没有的（例如只想了没说就被打断的回复）是空的，不算一条。工具名是 `own` 的调用不写：
/// 她自己翻记录的那几步。不是 [`kind_of`] 认的种类的是空的。
pub(super) fn text_of(event: &Event, placeholders: &Placeholders, own: &str) -> String {
    let mut text = match &event.body {
        Body::MessageUser(message) => blocks(&message.blocks, placeholders, own),
        Body::MessageAssistant(reply) => blocks(&reply.blocks, placeholders, own),
        Body::ToolResult(result) => blocks(&result.blocks, placeholders, own),
        Body::ContextCompacted(compacted) => compacted.summary.clone(),
        _ => String::new(),
    };
    text.truncate(text.trim_end().len());
    text
}

/// 「谁」那一格要另写的：别的会话发来的话要照整份日志记着的派出去的任务认是不是别的会话（`History::is_peer`），读完才认得出，
/// 先记下是哪一个会话发的（[`Sender::Session`]）；别的读到这一条就定了（[`from`]）。
pub(super) enum Sender {
    /// 定了：另写的，或者照 `who` 写的没有。
    Known(Option<String>),
    /// 这个会话发来的：是别的会话的写它的短编号（[`peer`]），父会话、派的子代理照 `who` 写。
    Session(SessionId),
}

/// 一条的「谁」那一格（[`Sender`]）。
pub(super) fn sender_of(event: &Event, placeholders: &Placeholders) -> Sender {
    match (&event.body, &event.by) {
        (Body::MessageUser(message), _) if message.venue.is_some() => {
            Sender::Known(from(event, placeholders))
        }
        (Body::MessageUser(_), By::Session(session)) => Sender::Session(session.id.clone()),
        _ => Sender::Known(from(event, placeholders)),
    }
}

/// 别的会话发来的那一条的「谁」：照 `history/session.txt` 写它的短编号，和请求里那块标签是同一个编号（施工 C-2）。
pub(super) fn peer(session: &SessionId, placeholders: &Placeholders) -> String {
    inline(say(&placeholders.session, &[("id", session.short())]))
}

/// 人切了权限级别（带 `permission` 的 `session.policy_changed`）算一条，「谁」是 `user`，原文写切成了哪一级，写法和
/// 事实里的一样（施工 2-7 补）。别的事件、只换了策略快照的没有。照日志原样读来的事件交给它，不管撤没撤掉：现在的权限就是
/// 这样算的，撤销不改它。
pub(super) fn switched(event: &Event, placeholders: &Placeholders) -> Option<Entry> {
    let Body::PolicyChanged(PolicyChanged {
        permission: Some(permission),
        ..
    }) = &event.body
    else {
        return None;
    };
    Some(Entry {
        seq: event.seq.get(),
        at: event.at,
        who: Who::User,
        from: None,
        text: inline(say(
            &placeholders.permission,
            &[("level", effective_level(permission))],
        )),
    })
}

/// 几块写成原文，一块一段：正文照原样；工具调用写成 `→ <工具名> <参数原文>`，工具名是 `own` 的不写；图片、文件、只有
/// 路径的附件（照文件的）写占位；思考不给。直接写进一份字里，不先一块块拷出来再接（施工 V-45）。
fn blocks(blocks: &[Block], placeholders: &Placeholders, own: &str) -> String {
    let mut text = String::new();
    let shown = blocks.iter().filter(|block| match block {
        Block::Text(_) | Block::Image(_) | Block::File(_) | Block::Path(_) => true,
        Block::ToolCall(call) => call.name != own,
        // 供应商侧工具（施工 HT-1）照不认识的块不给。
        Block::Reasoning(_) | Block::HostedTool(_) | Block::Unknown(_) => false,
    });
    for (k, block) in shown.enumerate() {
        if k > 0 {
            text.push('\n');
        }
        match block {
            Block::Text(part) => text.push_str(&part.text),
            Block::ToolCall(call) => {
                text.push_str("→ ");
                text.push_str(&call.name);
                text.push(' ');
                text.push_str(&call.args);
            }
            Block::Image(_) => text.push_str(&inline(say(&placeholders.image, &[]))),
            Block::File(file) => text.push_str(&inline(say(
                &placeholders.file,
                &[("name", file.name.as_str())],
            ))),
            // 只有路径的附件（施工 AT-1）照文件的占位写，带名字：不为它多一句字。
            Block::Path(attached) => text.push_str(&inline(say(
                &placeholders.file,
                &[("name", attached.name.as_str())],
            ))),
            Block::Reasoning(_) | Block::HostedTool(_) | Block::Unknown(_) => {}
        }
    }
    text
}

/// 别的 harness 发来的话（`message.user`，`by` 是 `harness`，施工 7-10）：「谁」那一格照 `history/agent.txt` 写它的名字，
/// 名字照模板的规矩转义，和请求里那块标签是同一个名字。场所会话里人说的话（施工 O-42，通讯平台的会话 2026-10-21 实测查出：她
/// 查「刚才谁推荐了 bitwarden」只看到 `user`）：照群里那一行的写法写发的人（[`sender`]）。别的会话发来的话另认（[`Sender`]）；
/// 别的没有。
fn from(event: &Event, placeholders: &Placeholders) -> Option<String> {
    match (&event.body, &event.by) {
        (Body::MessageUser(message), by) if message.venue.is_some() => {
            message.venue.as_ref().and_then(|venue| sender(by, venue))
        }
        (Body::MessageUser(_), By::Harness(harness)) => Some(inline(say(
            &placeholders.agent,
            &[("name", harness.name.as_str())],
        ))),
        _ => None,
    }
}

/// 场所里发的人，同群里那一行（`miyu-assemble` 的 `group::sender`，`docs/blueprint/chat.md`）：名字（没有的写平台身份），后面
/// 括号里是 `id=`（这一条 `show_ids`、又有名字的才写）和身份（`owner` 对应表里有的、`manager` 桥报的）。名字、平台身份是
/// 不可信的，照模板的规矩转义成一行。不是外部身份、不是认出来的本人的没有。
fn sender(by: &By, venue: &VenueMessage) -> Option<String> {
    let (id, role) = match by {
        By::External(external) => {
            let role = match (external.owner, external.role) {
                (true, _) => Some("owner"),
                (false, Some(Role::Manager)) => Some("manager"),
                (false, _) => None,
            };
            (&external.id, role)
        }
        By::Person(person) => (person.via.as_ref()?, Some("owner")),
        _ => return None,
    };
    let name = venue.name.as_deref().unwrap_or(id.as_str());
    let mut notes = Vec::new();
    if venue.show_ids && venue.name.is_some() {
        notes.push(format!("id={}", escape(id.as_str())));
    }
    notes.extend(role.map(str::to_string));
    Some(match notes.is_empty() {
        true => escape(name),
        false => format!("{} ({})", escape(name), notes.join(", ")),
    })
}

/// 占位写在一行里：去掉字的文件末尾那个换行。
fn inline(text: String) -> String {
    text.trim_end().to_string()
}
