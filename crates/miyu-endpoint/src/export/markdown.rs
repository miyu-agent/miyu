//! `session.export {format: "markdown"}`（施工 SE-1 中下上、中下中，设计 `58-会话导出和导入.md` 第一节，调研第五节第 2、3、
//! 6、7 条，`docs/blueprint/protocol.md`「`session.export`」）：整个会话喂投影（`view/whole.rs`），照预设和开关用
//! `miyu_view::markdown` 渲染，写到人给的文件或者放进回应。
//!
//! - 参数（`export/params.rs`）：`for` 定预设，`range`、`thinking`、`tools` 写了的盖过预设；`children` 只收 `reports`、`assets`
//!   只收 `placeholder`，写 `append`、`files` 的 `bad_params` 带 `data.later`（下一步才有）。
//! - `range: context`（`agent` 的默认）：每一条另交给内核的有效历史（`Sources::keeping_history`），喂完落到检查点上，和请求
//!   组装看到的是同一份。
//! - `tools: full`：后台命令的输出、脚本里调的工具交回的结果去 blob 库取全文（`Sources::wanted`）；取不到的照没有。
//! - `to: {"path"}` 同原始包写文件的规矩（绝对路径、数据根里面不写、不覆盖、先写临时文件再挂上去）；`to: "inline"` 放进回应的
//!   `text`，超过 [`INLINE_MOST`] 字节的拒 `export_too_big`（协议一行最长 1 MiB，给转义留余量）。
//! - 配了的密钥（`secrets.toml` 里的值）原样出现的换成 `[redacted: <名字>]`，回应的 `redacted` 报哪几个、各几处；原始包不改。
//! - 回应 `{path | text, bytes, tokens, upto, redacted}`：`tokens` 照底栏上下文用量的本地估算（UTF-8 字节数除以 4），`upto`
//!   是导到的最后一条。
//! - 原始包里的 `transcript.md` 也照这里渲染（[`transcript`]，`read` 预设）。
//! - 不写进会话日志；运行日志记一行 `INFO session exported`（会话、格式、多大、隐去几处，不记密钥的名字和值）。

use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::estimate;
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::time::UtcOffset;
use miyu_store::blob::Blobs;
use miyu_store::bundle;
use miyu_store::index::Row;
use miyu_store::root::DataRoot;
use miyu_view::Texts;
use miyu_view::markdown::{About, Document, Options, Range, Rendered, Sources, Tools, render};

use super::params::{Target, wanted};
use super::{ExportParams, TARGET, refuse_inside, refused};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;
use crate::view::whole::whole;
use crate::view::{Titles, texts};

/// `to: "inline"` 最多放多少字节：512 KiB。
const INLINE_MOST: usize = 512 * 1024;

/// 渲染一个会话要的、和会话无关的几样：给人看的字从哪来、时区、配了的密钥。
#[derive(Clone)]
pub(super) struct Kit {
    /// 资源目录。
    resources: miyu_store::resources::ResourceRoot,
    /// 投影兜底用的名字。
    titles: Titles,
    /// 时区。
    offset: UtcOffset,
    /// 配了的密钥：名字、值。
    secrets: Vec<(String, String)>,
}

impl Kit {
    /// 照核心这一刻的。
    pub(super) fn of(core: &Core) -> Kit {
        Kit {
            resources: core.resources.clone(),
            titles: Titles::of(core),
            offset: crate::sessions::offset(),
            secrets: core
                .config()
                .secrets
                .stored
                .entries
                .iter()
                .map(|(name, secret)| (name.clone(), secret.expose().to_string()))
                .collect(),
        }
    }
}

/// Markdown 的那一支：见模块的说明。
pub(super) async fn export(
    core: &Core,
    peer: Peer,
    session: SessionId,
    params: ExportParams,
) -> Result<Value, Refusal> {
    let (target, language, options) = wanted(params, peer.language)?;
    if !core.sessions.exists(core, &session).await {
        return Err(Refusal::NOT_FOUND);
    }
    let root = core.root.clone();
    let kit = Kit::of(core);
    let id = session.clone();
    let (rendered, upto, target) = tokio::task::spawn_blocking(move || {
        if let Target::File(out) = &target {
            refuse_inside(&root, out)?;
            if out.symlink_metadata().is_ok() {
                return Err(Refusal::EXPORT_EXISTS);
            }
        }
        let (rendered, upto) = transcript(&root, &id, &kit, language, options)?;
        match &target {
            Target::File(out) => bundle::write_file(out, rendered.text.as_bytes())
                .map_err(|error| refused(&id, error))?,
            Target::Inline if rendered.text.len() > INLINE_MOST => {
                return Err(Refusal::EXPORT_TOO_BIG);
            }
            Target::Inline => {}
        }
        Ok((rendered, upto, target))
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

/// 把会话 `session` 照 `options` 渲染成 Markdown（字照 `language`）：整份喂投影、记下条目里没有的原文，要的再取 blob。
/// 交回渲染好的和导到的最后一条。碰磁盘：在阻塞线程里调。
///
/// # Errors
///
/// 没有这个会话的 `session_not_found`；日志读不下去的 `session_broken`；给人看的字读不出来的 `internal`。
pub(super) fn transcript(
    root: &DataRoot,
    session: &SessionId,
    kit: &Kit,
    language: &'static str,
    options: Options,
) -> Result<(Rendered, Seq), Refusal> {
    let texts: Arc<Texts> = Arc::new(texts(&kit.resources, language, &kit.titles)?);
    let mut sources = match options.range {
        Range::Context => Sources::keeping_history(),
        Range::All | Range::Turns(_) => Sources::default(),
    };
    let mut row: Option<Row> = None;
    let blobs = Blobs::new(root.blobs());
    let fed = whole(
        &root.session_dir(session),
        Arc::clone(&texts),
        blobs.clone(),
        |event| {
            sources.see(event);
            // 开头的几样同会话列表那一行的算法：照第一条起一行，再一条条看（同 `list.rs`）。
            if row.is_none() {
                row = Row::new(session.clone(), event);
            }
            if let Some(row) = &mut row {
                row.see(event);
            }
        },
    )
    .map_err(|error| crate::view::unread(session, error))?;
    sources.settle();
    let entries = fed.projector.entries();
    if options.tools == Tools::Full {
        for hash in sources.wanted(entries) {
            if let Ok(bytes) = blobs.get(&hash) {
                sources
                    .blobs
                    .insert(hash, String::from_utf8_lossy(&bytes).into_owned());
            }
        }
    }
    let row = row.ok_or(Refusal::BROKEN)?;
    let about = About {
        title: match row.title.is_empty() {
            true => row.preview.clone(),
            false => row.title.clone(),
        },
        from: row.created,
        to: row.last_active,
        tokens: row.tokens,
        cwd: row.cwd.clone(),
    };
    let secrets: Vec<(&str, &str)> = kit
        .secrets
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let rendered = render(&Document {
        entries,
        about: &about,
        sources: &sources,
        texts: &texts,
        options,
        offset: kit.offset,
        secrets: &secrets,
    });
    Ok((rendered, fed.upto))
}
