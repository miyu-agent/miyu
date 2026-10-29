//! 发送前问一声「前缀要变了」（施工 OV-1 下，项目主人 10-26 定的交互，`docs/construction/OV-1-同名覆盖工具说明和提示词.md`
//! 第十节）：你的覆盖、人格、预设那几棵树改过、会话还没照它拼过的，开新一轮之前先问人，免得他不知道这一轮会因为缓存
//! 失效多付输入的钱。
//!
//! - 只问握手声明了 `caps.prefix_ask` 的头；本机会话、这一刻空着（不在跑一轮）的才问。
//! - 不带 `prefix` 的拒 `prefix_changing`，`data.files` 是改了的文件（最多 [`MOST`] 个，多的另带 `more`）。
//! - `"prefix": "keep"` 放行这一批照常说；`"prefix": "compact"` 放行、先压缩再说，压不了的回压缩自己的拒绝、这句不发。
//! - 放行照会话记（[`miyu_session::prefix::Prefix::allow`]）；之后有新的改动再问。

use serde::Deserialize;
use serde_json::json;

use miyu_kernel::id::CommandId;
use miyu_kernel::origin::By;
use miyu_kernel::session::{Command, Outcome};
use miyu_session::Handle;

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
            let (files, more) = core.edits.since(settled, MOST);
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
