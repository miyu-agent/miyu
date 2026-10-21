//! 服务的提示词（施工 X-39 之二，`docs/blueprint/mcp.md`「怎么走」第六条）：列完工具接着列（服务声明了 `prompts` 的才列），
//! 名字清理成斜杠命令名 `<服务名>:<提示词名>`，记在看管那一格；撞名的不登记，进 `skipped`。菜单、`command.run` 照它
//! （`commands/prompts.rs`）。

use std::sync::Arc;
use std::time::Duration;

use miyu_kernel::block::Block;
use miyu_mcp::{Called, Client, Message, Prompt};
use miyu_tool::Picture;

use super::tool::Texts;
use super::{Shared, Skipped, TARGET, content};
use crate::Core;

/// 命令名最多几个字符（同工具名的上限）。
const NAME_LIMIT: usize = 64;

/// 登记成斜杠命令的一个提示词。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Offered {
    /// 命令名：`<服务名>:<清理过的提示词名>`，恰好一个 `:`（两个头的斜杠菜单照这个形状认）。
    pub(crate) command: String,
    /// 服务列出来的那一项：取的时候照它的原名问。
    pub(crate) prompt: Prompt,
    /// 取一次最多等多久：服务的 `timeout_ms`。
    pub(crate) wait: Duration,
}

/// 一个在跑的服务登记的提示词（施工 X-39，`Servers::prompts`）。
pub(crate) struct Prompts {
    /// 包的编号：个人设置里的是 `~home`。
    pub(crate) package: String,
    /// 服务名。
    pub(crate) service: String,
    /// 它的客户端：取提示词照它问。
    pub(crate) client: Arc<Client>,
    /// 登记的几个。
    pub(crate) offered: Vec<Offered>,
}

/// 服务 `service` 的提示词 `prompt` 的命令名：提示词名里不合 `[A-Za-z0-9_-]` 的字（空白、斜杠、`:` 都算）换成 `_`，整个超过
/// [`NAME_LIMIT`] 个字符的截到那么多（同工具名，`spec::name`）。
pub(crate) fn command_name(service: &str, prompt: &str) -> String {
    let cleaned: String = prompt
        .chars()
        .map(
            |c| match c.is_ascii_alphanumeric() || c == '_' || c == '-' {
                true => c,
                false => '_',
            },
        )
        .collect();
    let mut name = format!("{service}:{cleaned}");
    name.truncate(NAME_LIMIT);
    name
}

/// 列出来的 `prompts` 换成登记的几个：清理以后撞了同一个服务前面那个的不登记，交回进 `skipped` 的那几个。
/// 等 `prompts/get` 多久（施工 X-39 补）：照服务的 `timeout_ms`，最多 [`crate::commands::ANSWER_WAIT`]（同扩展的命令）。
/// `timeout_ms` 是给调工具的，可以长到一小时；人敲了命令在等。
pub(super) fn wait(timeout_ms: u64) -> Duration {
    Duration::from_millis(timeout_ms).min(crate::commands::ANSWER_WAIT)
}

pub(super) fn offered(
    service: &str,
    prompts: Vec<Prompt>,
    wait: Duration,
) -> (Vec<Offered>, Vec<Skipped>) {
    let mut offered: Vec<Offered> = Vec::new();
    let mut skipped = Vec::new();
    for prompt in prompts {
        let command = command_name(service, &prompt.name);
        if offered.iter().any(|one| one.command == command) {
            tracing::warn!(target: TARGET, service, prompt = prompt.name.as_str(), taken_by = command.as_str(), "mcp prompt skipped");
            skipped.push(Skipped::prompt(prompt.name, "name_taken"));
            continue;
        }
        offered.push(Offered {
            command,
            prompt,
            wait,
        });
    }
    (offered, skipped)
}

/// 列提示词（服务声明了的才列），换成登记的几个记进看管那一格，交回进 `skipped` 的那几个。列不出来的记一行，照没有算：
/// 提示词列不出来不该连工具一起拖垮。
pub(super) async fn list(shared: &Shared, client: &Client, wait: Duration) -> Vec<Skipped> {
    if !client.offers_prompts() {
        shared.set_prompts(Vec::new());
        return Vec::new();
    }
    let service = shared.service.as_str();
    let listed = match client.prompts().await {
        Ok(listed) => listed,
        Err(error) => {
            tracing::warn!(target: TARGET, service, error = %error, "mcp prompts not listed");
            shared.set_prompts(Vec::new());
            return Vec::new();
        }
    };
    for problem in &listed.skipped {
        tracing::warn!(target: TARGET, service, problem = problem.as_str(), "mcp prompt skipped");
    }
    let (offered, skipped) = offered(service, listed.prompts, wait);
    shared.set_prompts(offered);
    skipped
}

/// 取回来的几条消息换成块（施工 X-39）：照工具结果的规矩（`content::done`：文字照原文、太长的截、四种图、声音和二进制的资源
/// 写一行），`assistant` 的照原文并进来，前面不另加标记（会丢掉谁说的，Claude Code 也是这么并的）。空的文字块不要。交回文字块
/// 和图（调的一方存成 blob、接在后面）；字读不出来的（安装坏了）没有。
pub(crate) fn said(core: &Core, messages: Vec<Message>) -> Option<(Vec<Block>, Vec<Picture>)> {
    let texts = Texts::load(core)?;
    let called = Called {
        content: messages
            .into_iter()
            .map(|message| message.content)
            .collect(),
        structured: None,
        is_error: false,
    };
    let done = content::done(called, &texts);
    let blocks = done
        .blocks
        .into_iter()
        .filter(|block| !matches!(block, Block::Text(text) if text.text.is_empty()))
        .collect();
    Some((blocks, done.images))
}

#[cfg(test)]
mod tests;
