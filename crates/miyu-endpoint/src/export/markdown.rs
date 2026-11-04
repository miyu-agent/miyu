//! `session.export {format: "markdown"}`（施工 SE-1 中下上，设计 `58-会话导出和导入.md` 第一节，调研第五节第 2、3、7 条，
//! `docs/blueprint/protocol.md`「`session.export`」）：整个会话喂投影（`view/whole.rs`），用 `miyu_view::markdown` 的 `read`
//! 预设渲染，写到人给的文件或者放进回应。
//!
//! - 这一步只收 `for: "read"`（不写的也是）、`range: "all"`（不写的也是）、`locale`（不写的照连接握手时的语言）。`for` 写
//!   `agent`、`chat`，`range` 写 `context`、`{"turns": N}`，写了 `thinking`、`tools`、`children`、`assets` 的，`bad_params`，
//!   `data.later` 说是哪一个：下一步才有。别的写法照旧 `bad_params`。
//! - `to: {"path"}` 同原始包写文件的规矩（绝对路径、数据根里面不写、不覆盖、先写临时文件再挂上去）；`to: "inline"` 放进回应的
//!   `text`，超过 [`INLINE_MOST`] 字节的拒 `export_too_big`（协议一行最长 1 MiB，给转义留余量）。
//! - 配了的密钥（`secrets.toml` 里的值）原样出现的换成 `[redacted: <名字>]`，回应的 `redacted` 报哪几个、各几处；原始包不改。
//! - 回应 `{path | text, bytes, tokens, upto, redacted}`：`tokens` 照底栏上下文用量的本地估算（UTF-8 字节数除以 4），`upto`
//!   是导到的最后一条。
//! - 不写进会话日志；运行日志记一行 `INFO session exported`（会话、格式、多大、隐去几处，不记密钥的名字和值）。

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::estimate;
use miyu_kernel::id::SessionId;
use miyu_store::blob::Blobs;
use miyu_store::bundle;
use miyu_store::index::Row;
use miyu_view::markdown::{About, Document, Options, Sources, render};

use super::{ExportParams, Inline, TARGET, To, checked, refuse_inside, refused};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::view::whole::whole;
use crate::view::{Titles, texts};

/// `to: "inline"` 最多放多少字节：512 KiB。
const INLINE_MOST: usize = 512 * 1024;

/// 写到哪。
enum Target {
    /// 本机的一个文件。
    File(PathBuf),
    /// 放进回应。
    Inline,
}

/// Markdown 的那一支：见模块的说明。
pub(super) async fn export(
    core: &Core,
    peer: Peer,
    session: SessionId,
    params: ExportParams,
) -> Result<Value, Refusal> {
    let (target, language) = wanted(params, peer.language)?;
    if !core.sessions.exists(core, &session).await {
        return Err(Refusal::NOT_FOUND);
    }
    let root = core.root.clone();
    let dir = root.session_dir(&session);
    let resources = core.resources.clone();
    let titles = Titles::of(core);
    let secrets: Vec<(String, String)> = core
        .config()
        .secrets
        .stored
        .entries
        .iter()
        .map(|(name, secret)| (name.clone(), secret.expose().to_string()))
        .collect();
    let offset = crate::sessions::offset();
    let id = session.clone();
    let (rendered, upto, target) = tokio::task::spawn_blocking(move || {
        if let Target::File(out) = &target {
            refuse_inside(&root, out)?;
            if out.symlink_metadata().is_ok() {
                return Err(Refusal::EXPORT_EXISTS);
            }
        }
        let texts = Arc::new(texts(&resources, language, &titles)?);
        let mut sources = Sources::default();
        let mut row: Option<Row> = None;
        let fed = whole(
            &dir,
            Arc::clone(&texts),
            Blobs::new(root.blobs()),
            |event| {
                sources.see(event);
                // 开头的几样同会话列表那一行的算法：照第一条起一行，再一条条看（同 `list.rs`）。
                if row.is_none() {
                    row = Row::new(id.clone(), event);
                }
                if let Some(row) = &mut row {
                    row.see(event);
                }
            },
        )
        .map_err(|error| crate::view::unread(&id, error))?;
        let row = row.ok_or(Refusal::BROKEN)?;
        let about = About {
            title: match row.title.is_empty() {
                true => row.preview.clone(),
                false => row.title.clone(),
            },
            from: row.created,
            to: row.last_active,
            tokens: row.tokens,
        };
        let secrets: Vec<(&str, &str)> = secrets
            .iter()
            .map(|(name, value)| (name.as_str(), value.as_str()))
            .collect();
        let rendered = render(&Document {
            entries: fed.projector.entries(),
            about: &about,
            sources: &sources,
            texts: &texts,
            options: Options::READ,
            offset,
            secrets: &secrets,
        });
        match &target {
            Target::File(out) => bundle::write_file(out, rendered.text.as_bytes())
                .map_err(|error| refused(&id, error))?,
            Target::Inline if rendered.text.len() > INLINE_MOST => {
                return Err(Refusal::EXPORT_TOO_BIG);
            }
            Target::Inline => {}
        }
        Ok((rendered, fed.upto, target))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)??;
    let bytes = rendered.text.len();
    tracing::info!(
        target: TARGET,
        session = session.as_str(),
        format = "markdown",
        bytes,
        redacted = rendered.redacted.values().sum::<usize>(),
        "session exported"
    );
    let mut reply = json!({
        "bytes": bytes,
        "tokens": estimate::text(&rendered.text),
        "upto": upto,
        "redacted": rendered.redacted,
    });
    match target {
        Target::File(out) => reply["path"] = json!(out),
        Target::Inline => reply["text"] = json!(rendered.text),
    }
    Ok(reply)
}

/// 照参数定写到哪、用哪种语言（`language` 是连接握手时的）；下一步才有的写法拒 `bad_params` 带 `data.later`。
fn wanted(params: ExportParams, language: &'static str) -> Result<(Target, &'static str), Refusal> {
    match params.purpose.as_deref() {
        None | Some("read") => {}
        Some("agent" | "chat") => return Err(Refusal::later("for")),
        Some(_) => return Err(Refusal::BAD_PARAMS),
    }
    match &params.range {
        None => {}
        Some(Value::String(range)) if range == "all" => {}
        Some(Value::String(range)) if range == "context" => return Err(Refusal::later("range")),
        Some(Value::Object(range))
            if range.len() == 1
                && range
                    .get("turns")
                    .and_then(Value::as_u64)
                    .is_some_and(|n| n > 0) =>
        {
            return Err(Refusal::later("range"));
        }
        Some(_) => return Err(Refusal::BAD_PARAMS),
    }
    for (name, switch) in [
        ("thinking", &params.thinking),
        ("tools", &params.tools),
        ("children", &params.children),
        ("assets", &params.assets),
    ] {
        if switch.is_some() {
            return Err(Refusal::later(name));
        }
    }
    let language = match params.locale.as_deref() {
        None => language,
        Some(locale) if locale.starts_with("zh") => "zh",
        Some(locale) if locale.starts_with("ja") => "ja",
        Some(_) => "en",
    };
    let target = match params.to {
        To::File(to) => Target::File(checked(to.path)?),
        To::Inline(Inline::Inline) => Target::Inline,
    };
    Ok((target, language))
}
