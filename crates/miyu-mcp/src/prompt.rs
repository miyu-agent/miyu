//! 服务的提示词（施工 X-39，`docs/blueprint/mcp.md`「怎么走」第六条）：`prompts/list` 列出来的一项、`prompts/get` 回来的
//! 几条消息。照 MCP 的写法读，消息的内容同工具结果（[`Content`]）。变成斜杠命令、并成人说的一句是核心那一边的事。

use std::time::Duration;

use serde_json::Value;

use crate::Content;

/// 一个提示词。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompt {
    /// 服务给的名字，原样。
    pub name: String,
    /// 给人看的名字，没写的没有。
    pub title: Option<String>,
    /// 说明，没写的没有。
    pub description: Option<String>,
    /// 参数，照服务写的先后。
    pub arguments: Vec<Argument>,
}

/// 提示词的一个参数。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    /// 名字。
    pub name: String,
    /// 说明，没写的没有。
    pub description: Option<String>,
    /// 必填。
    pub required: bool,
}

/// `prompts/list` 列全了的。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Prompts {
    /// 认得出的，照服务给的先后。
    pub prompts: Vec<Prompt>,
    /// 认不出来、没收的几项：说哪一项哪里不对。
    pub skipped: Vec<String>,
    /// 新时代的服务说这份多久内不用再问（`ttlMs`）；没说的、旧时代的没有。
    pub fresh_for: Option<Duration>,
}

/// 谁说的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// 人。
    User,
    /// 模型。
    Assistant,
}

/// `prompts/get` 回来的一条消息。
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    /// 谁说的。
    pub role: Role,
    /// 内容。
    pub content: Content,
}

impl Prompt {
    /// 读 `prompts/list` 里的一项：要有名字，参数要有名字。名字照原样（取的时候照它问）；变成斜杠命令名时的清理是核心那一边的事。
    ///
    /// # Errors
    ///
    /// 说哪里不对。
    pub fn read(raw: &Value) -> Result<Prompt, String> {
        let text =
            |value: &Value, key: &str| value.get(key).and_then(Value::as_str).map(str::to_string);
        let Some(name) = text(raw, "name").filter(|name| !name.is_empty()) else {
            return Err(format!("a prompt without a name: {raw}"));
        };
        let mut arguments = Vec::new();
        for argument in raw
            .get("arguments")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            let Some(argument_name) = text(argument, "name").filter(|name| !name.is_empty()) else {
                return Err(format!("prompt {name}: an argument without a name"));
            };
            arguments.push(Argument {
                name: argument_name,
                description: text(argument, "description"),
                required: argument.get("required").and_then(Value::as_bool) == Some(true),
            });
        }
        Ok(Prompt {
            title: text(raw, "title").filter(|title| !title.trim().is_empty()),
            description: text(raw, "description"),
            name,
            arguments,
        })
    }
}

impl Message {
    /// 读 `prompts/get` 回来的 `messages`：`role` 不是 `user`、`assistant` 的、没有内容的不收。
    pub(crate) fn read_all(result: &Value) -> Vec<Message> {
        result
            .get("messages")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|raw| {
                let role = match raw.get("role").and_then(Value::as_str) {
                    Some("user") => Role::User,
                    Some("assistant") => Role::Assistant,
                    _ => return None,
                };
                Some(Message {
                    role,
                    content: Content::read(raw.get("content")?),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests;
