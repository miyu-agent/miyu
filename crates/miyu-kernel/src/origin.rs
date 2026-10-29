//! 事件的 `by`：这件事由谁引起（`docs/designs/03-事件模型.md` 第二节）。
//!
//! 内核从连接取，不从正文取：正文里自称是谁一概不作数（`01-架构.md` 第六节）。
//! 权限按「这一步是谁要求的」来判，看的就是这一格（`06-多用户与身份.md` 第五节）。

use serde::{Deserialize, Deserializer, Serialize};

use crate::id::{
    CallId, ExternalId, HarnessName, ModelName, ModuleId, ProviderId, SessionId, VenueId,
};
use crate::raw::{self, RawJson};

/// 这件事由谁引起。JSON 里用 `kind` 分开九种；读到不认识的，整块原样留着。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum By {
    /// 主人（Miyu 只给一个人用，施工 SU-6 上）。
    Person(Person),
    /// 通讯平台上的人，由桥担保（`06-多用户与身份.md` 第二节）。
    External(External),
    /// 模型：它的回复 `message.assistant`，连同里面的工具调用。
    Model(Model),
    /// 一次工具调用：这件事是它在执行时引起的。
    Tool(Tool),
    /// 模块，包括扩展，例如记忆模块注入的召回结果。
    Module(Module),
    /// 另一个会话，例如父会话给子代理留言。
    Session(Session),
    /// 内核自己，例如崩溃重启后给没走完的回合补上的「中断」（`02-内核.md` 不变量 8）。
    Kernel,
    /// 别的 harness：经 `miyu ask --from` 发来的话（施工 7-1，`agents.md` 第十一条）。
    Harness(Harness),
    /// 到点的定时任务、循环交进来的话（施工 J-1 上，设计 `45-定时和循环.md` 第三节第 2 条）。
    Schedule(Schedule),
    /// 不认识的种类，新版本才有的：整块原样留着，写出去还是原样。
    #[serde(untagged)]
    Unknown(RawJson),
}

/// 主人（施工 SU-6 上：只有一个人，不再记账号）。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Person {
    /// 经哪个平台身份认出来的主人（施工 O-3，`venues.md`）：主人在通讯平台的私聊里说的话，核心查主人对应表认出是主人。
    /// 本机的头上说的没有。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub via: Option<ExternalId>,
}

/// 通讯平台上的人。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct External {
    /// 这个人是在哪个场所说的话，例如哪个群。
    pub venue: VenueId,
    /// 平台上的身份编号，由桥担保，例如 `qq:10086`。
    pub id: ExternalId,
    /// 在主人对应表里，是主人（施工 O-3，SU-6 上起不再记账号）：群里的主人仍是外部身份，写的时候就记下，以后对应表改了不跟着
    /// 变。不是的不写。
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub owner: bool,
    /// 桥报的这个人在场所里的身份（施工 O-3）：斜杠命令照它判谁能用（O-6）。没有 `owner`：主人照对应表认。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}

/// 桥报的场所里的身份（施工 O-3，`venues.md`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// 场所里管理的人：场所规则的 `managers`。
    Manager,
    /// 别的人。
    Member,
}

/// 一个模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    /// 经哪个供应商调用的。
    pub endpoint: ProviderId,
    /// 模型的名字。和供应商的名字一样，内核只记不解读。
    pub model: ModelName,
}

/// 一次工具调用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tool {
    /// 那一次调用的编号。
    pub call_id: CallId,
}

/// 一个模块，包括扩展。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Module {
    /// 模块的编号，就是它清单里的 `id`。
    pub id: ModuleId,
}

/// 另一个会话。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    /// 那个会话的编号。
    pub id: SessionId,
}

/// 别的 harness。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Harness {
    /// 它自己报的名字，不可信：写法照短名字查，给模型看之前照不可信的文本处理（`kernel/ids.md`）。
    pub name: HarnessName,
}

/// 到点的一个任务。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    /// 任务的编号（`t<n>`），在唤醒表里。
    pub schedule: String,
    /// 是循环的话，哪一种：组装照它挑标签（施工 J-2）。定时任务没有。
    #[serde(rename = "loop", default, skip_serializing_if = "Option::is_none")]
    pub looping: Option<ScheduleLoop>,
}

/// 循环的种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScheduleLoop {
    /// 固定间隔。
    Fixed,
    /// 她每轮结束自己定下一次。
    Dynamic,
}

impl By {
    /// 是人（施工 R-8 从回合索引挪进内核，两处共用）：有账号的人（`Person`）、通讯平台上的人（`External`）。别的会话、子代理、
    /// 别的 harness、工具、模块、内核都不是。回合索引收哪几轮（`memory.md` 第一条第 1 款）、联想拿哪句话（第四条第 1 款）
    /// 照它判。
    #[must_use]
    pub fn is_person(&self) -> bool {
        matches!(self, By::Person(_) | By::External(_))
    }

    /// 是主人本人（施工 O-2 下，原来在斜杠命令里）：本机的头上的人、私聊里对应表认出的本人（`Person`）、群里对应表里有的
    /// 外部身份。斜杠命令照它判谁能换工作区，提供者的工具照它挡（`tool.call` 的 `owner`）。
    #[must_use]
    pub fn is_owner(&self) -> bool {
        match self {
            By::Person(_) => true,
            By::External(external) => external.owner,
            _ => false,
        }
    }
}

impl<'de> Deserialize<'de> for By {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        raw::read_tagged(
            d,
            "kind",
            |kind, json| {
                Some(match kind {
                    "person" => raw::parse(json).map(By::Person),
                    "external" => raw::parse(json).map(By::External),
                    "model" => raw::parse(json).map(By::Model),
                    "tool" => raw::parse(json).map(By::Tool),
                    "module" => raw::parse(json).map(By::Module),
                    "session" => raw::parse(json).map(By::Session),
                    "kernel" => Ok(By::Kernel),
                    "harness" => raw::parse(json).map(By::Harness),
                    "schedule" => raw::parse(json).map(By::Schedule),
                    _ => return None,
                })
            },
            By::Unknown,
        )
    }
}

#[cfg(test)]
mod tests;
