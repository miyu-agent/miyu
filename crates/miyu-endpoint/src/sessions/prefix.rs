//! 发送前问一声「前缀要变了」（施工 OV-1 下，项目主人 10-26 定的交互，`docs/construction/OV-1-同名覆盖工具说明和提示词.md`
//! 第十节）：你的覆盖、人格、预设那几棵树改过、会话还没照它拼过的，开新一轮之前先问人，免得他不知道这一轮会因为缓存
//! 失效多付输入的钱。
//!
//! - 只问握手声明了 `caps.prefix_ask` 的头；本机会话、这一刻空着（不在跑一轮）的才问。
//! - 不带 `prefix` 的拒 `prefix_changing`，`data.files` 是改了的文件（最多 [`MOST`] 个，多的另带 `more`）。
//! - `"prefix": "keep"` 放行这一批照常说；`"prefix": "compact"` 放行、先压缩再说，压不了的回压缩自己的拒绝、这句不发。
//! - 放行照会话记（[`miyu_session::prefix::Prefix::allow`]）；之后有新的改动再问。
//! - 照这个会话自己判（施工 PX-1）：号变了，先叫会话照回合开始那一套干算一遍（[`Handle::check_prefix`]），它的前缀不变的不问
//!   （改的是别的人格、头像、背景、它用不到的覆盖），变了才问，`data.files` 只列真进它前缀的那几个（[`touches`]）。

use serde::Deserialize;
use serde_json::json;

use miyu_kernel::id::CommandId;
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};
use miyu_session::Handle;
use miyu_session::prefix::{PrefixCheck, Touches};

use crate::Core;
use crate::hello::Peer;
use crate::list::LOCAL;
use crate::refusal::Refusal;

/// 改了的文件最多列几个。
const MOST: usize = 20;

/// 人选的：直接发送，或者压缩上下文再发送。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Choice {
    /// 直接发送。
    Keep,
    /// 压缩上下文并发送。
    Compact,
}

/// 开这一轮之前问过了：照常说，或者人选了先压缩（已经压完、交出去了）。
///
/// # Errors
///
/// 要问人的（`prefix_changing`）；先压缩压不了的（压缩自己的原因）；会话停了的。
pub(crate) async fn ask(
    core: &Core,
    (peer, by): (Peer, &By),
    (handle, command): (&Handle, &CommandId),
    choice: Option<Choice>,
) -> Result<(), Refusal> {
    if !peer.prefix_ask || handle.venue().as_str() != LOCAL || handle.busy() {
        return Ok(());
    }
    let version = core.edits.version();
    let prefix = handle.prefix();
    let settled = prefix.seen().max(prefix.allowed());
    if version <= settled {
        return Ok(());
    }
    match choice {
        None => {
            let touched = match handle.check_prefix(version).await {
                Ok(PrefixCheck::Unchanged) => return Ok(()),
                Ok(PrefixCheck::Changed(touched)) => touched,
                Err(_) => return Err(Refusal::STOPPED),
            };
            let (files, more) = relevant(core.edits.since(settled, usize::MAX), &touched);
            let mut data = serde_json::Map::new();
            data.insert("files".to_string(), json!(files));
            if more > 0 {
                data.insert("more".to_string(), json!(more));
            }
            Err(Refusal {
                data: Some(data),
                ..Refusal::PREFIX_CHANGING
            })
        }
        Some(Choice::Keep) => {
            prefix.allow(version);
            Ok(())
        }
        Some(Choice::Compact) => {
            let id = CommandId::parse(&format!("{}/prefix", command.as_str()))
                .map_err(|_| Refusal::BAD_PARAMS)?;
            let compact = Command::Compact { instructions: None };
            match handle.command(id, by.clone(), compact).await {
                Ok(Outcome::Rejected { reason, about }) => {
                    Err(Refusal::kernel_about(reason, &about))
                }
                Ok(_) => {
                    prefix.allow(version);
                    Ok(())
                }
                Err(_) => Err(Refusal::STOPPED),
            }
        }
    }
}

/// 改了的文件里真进这个会话前缀的那几个，最多 [`MOST`] 个，多的算进「还有」。一个都挑不出来的（会话说要变、改动却对不上它用的
/// 那几样）照全部列，免得问人时说不出改了什么。
fn relevant((all, lost): (Vec<String>, usize), touched: &Touches) -> (Vec<String>, usize) {
    let picked: Vec<String> = all
        .iter()
        .filter(|path| touches(path, touched))
        .cloned()
        .collect();
    let mut files = if picked.is_empty() { all } else { picked };
    let more = files.len().saturating_sub(MOST) + lost;
    files.truncate(MOST);
    (files, more)
}

/// 图片：人格文件夹里的头像、背景这些，不进前缀。
const IMAGES: [&str; 9] = [
    "png", "jpg", "jpeg", "webp", "gif", "avif", "bmp", "svg", "ico",
];

/// 改了的 `path`（照你那一层写的相对路径，`personas/<编号>/…`、`presets/<编号>.toml`、`tools/<名字>.…`、`prompts/…`）进不进用着
/// `touched` 的会话的前缀：它用的那个人格的文件（图片不算）、它的预设、它工具面上那几件工具的覆盖、提示词的覆盖（哪个会话都用）。
fn touches(path: &str, touched: &Touches) -> bool {
    let mut parts = path.split('/');
    let stem = |name: &str| name.split('.').next().unwrap_or(name).to_string();
    match (parts.next(), parts.next()) {
        (Some("personas"), Some(id)) => {
            let image = path.rsplit_once('.').is_some_and(|(_, extension)| {
                IMAGES.contains(&extension.to_ascii_lowercase().as_str())
            });
            touched.persona.as_deref() == Some(id) && !image
        }
        (Some("presets"), Some(file)) => touched.preset.as_deref() == Some(stem(file).as_str()),
        (Some("tools"), Some(file)) => touched.tools.contains(&stem(file)),
        (Some("prompts"), _) => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests;
