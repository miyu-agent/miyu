//! `session.export {session, format, to}`（施工 SE-1 上，设计 `58-会话导出和导入.md` 第一节，`docs/blueprint/protocol.md`
//! 「`session.export`」）：把一个会话打成原始包（`format: "bundle"`）写到人给的路径。Markdown（`format: "markdown"`，施工
//! SE-1 中下上、中下中）在 `export/markdown.rs`，写文件照同样的规矩。
//!
//! - 包里有：这个会话、它的全部后代（子代理、子代理的子代理）、分叉来源到分叉点为止的那几段（一层层往上），和它们引用到的
//!   blob（[`miyu_store::bundle`]）。只读日志、不载入会话，在跑的会话照这一刻已经落盘的取。
//! - 包里另放一份 `transcript.md`（施工 SE-1 中下中，调研第五节第 8 条）：主会话照 `read` 预设渲染（语言照连接握手时的，
//!   配了的密钥照样换掉），解开就能看；包里的日志原样不改。
//! - 只给人的头（`connection/rights.rs`）：包里和事件流一个级别，还带着 blob。
//! - 核心照头给的路径写（同 `blob.put` 的 `path` 反过来）：要绝对路径；数据根里面不写（`export_in_data_root`）；已经有了的
//!   不覆盖（`export_exists`）；目录不在、写不进的 `export_unwritable`。
//! - 不写进会话日志：它不改会话。运行日志记一行，谁在什么时候导了哪个会话、多大。

use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::event::Body;
use miyu_kernel::id::{Seq, SessionId};
use miyu_kernel::time::Timestamp;
use miyu_store::blob::Blobs;
use miyu_store::bundle::{self, About, BundleError, Part};
use miyu_store::index::SessionIndex;
use miyu_store::log::first_event;
use miyu_store::root::DataRoot;
use miyu_view::markdown::Options;

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

mod markdown;
mod params;

/// 运行日志的来源。
const TARGET: &str = "miyu::endpoint";

/// `session.export` 的参数。`for` 以下只对 Markdown（`export/markdown.rs`），原始包写了的 `bad_params`。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExportParams {
    /// 会话编号。
    session: String,
    /// 导成什么。
    format: Format,
    /// 写到哪。
    to: To,
    /// 给谁用：`read`（不写的也是）、`agent`、`chat`（`export/params.rs`）。
    #[serde(default, rename = "for")]
    purpose: Option<String>,
    /// 导多少：`all`、`context`、`{"turns": N}`；不写的照预设。
    #[serde(default)]
    range: Option<Value>,
    /// 给人看的字用哪种语言；不写的照连接握手时的。
    #[serde(default)]
    locale: Option<String>,
    /// 盖过预设的开关：带不带思考。
    #[serde(default)]
    thinking: Option<Value>,
    /// 同上：工具写多少，`line`、`seen`、`full`。
    #[serde(default)]
    tools: Option<Value>,
    /// 同上：子代理，这一步只收 `reports`。
    #[serde(default)]
    children: Option<Value>,
    /// 同上：图片、附件，这一步只收 `placeholder`。
    #[serde(default)]
    assets: Option<Value>,
}

impl ExportParams {
    /// 写了只对 Markdown 的参数。
    fn for_markdown(&self) -> bool {
        self.purpose.is_some()
            || self.range.is_some()
            || self.locale.is_some()
            || self.thinking.is_some()
            || self.tools.is_some()
            || self.children.is_some()
            || self.assets.is_some()
    }
}

/// 导成什么。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Format {
    /// 原始包（ZIP）。
    Bundle,
    /// Markdown（施工 SE-1 中下上）。
    Markdown,
}

/// 写到哪：本机的一个文件，或者放进回应（只对 Markdown）。
#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum To {
    /// 本机的一个文件。
    File(ToFile),
    /// `"inline"`：放进回应的 `text`。
    Inline(Inline),
}

/// 本机的一个文件。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToFile {
    /// 绝对路径。
    path: PathBuf,
}

/// 写成字的 `"inline"`。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Inline {
    /// 放进回应。
    Inline,
}

/// `session.export`：见模块的说明。原始包回应 `{path, bytes, upto, sessions, blobs}`：写到哪、包多大、主会话导到第几条、包里
/// 有哪几个会话、几个 blob；Markdown 的见 `export/markdown.rs`。
pub(crate) async fn export(
    core: &Core,
    peer: Peer,
    params: ExportParams,
) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    if let Format::Markdown = params.format {
        return markdown::export(core, peer, session, params).await;
    }
    let (false, To::File(to)) = (params.for_markdown(), params.to) else {
        return Err(Refusal::BAD_PARAMS);
    };
    let out = checked(to.path)?;
    if !core.sessions.exists(core, &session).await {
        return Err(Refusal::NOT_FOUND);
    }
    let root = core.root.clone();
    let index = core.index();
    let main = session.clone();
    let target = out.clone();
    let kit = markdown::Kit::of(core);
    let language = peer.language;
    let done = tokio::task::spawn_blocking(move || {
        refuse_inside(&root, &target)?;
        let parts = gather(&root, Some(&index), &main).map_err(|error| {
            tracing::warn!(target: TARGET, session = main.as_str(), error = %error, "export not read");
            Refusal::BROKEN
        })?;
        let (transcript, _) =
            markdown::transcript(&root, &main, &kit, language, Options::READ)?;
        let about = About {
            main: main.clone(),
            miyu: env!("CARGO_PKG_VERSION").to_string(),
            exported: now(),
            transcript: Some(transcript.text),
        };
        bundle::write(&parts, &Blobs::new(root.blobs()), &about, &target).map_err(|error| refused(&main, error))
    })
    .await
    .map_err(|_| Refusal::INTERNAL)??;
    let manifest = &done.manifest;
    tracing::info!(
        target: TARGET,
        session = session.as_str(),
        sessions = manifest.sessions.len(),
        blobs = manifest.blobs.len(),
        bytes = done.bytes,
        "session exported"
    );
    let upto = manifest.sessions.first().and_then(|first| first.upto);
    let sessions: Vec<&str> = manifest
        .sessions
        .iter()
        .map(|one| one.id.as_str())
        .collect();
    Ok(json!({
        "path": out,
        "bytes": done.bytes,
        "upto": upto,
        "sessions": sessions,
        "blobs": manifest.blobs.len(),
    }))
}

/// 头给的路径：要绝对的、带文件名的。
fn checked(out: PathBuf) -> Result<PathBuf, Refusal> {
    match out.is_absolute() && out.file_name().is_some() {
        true => Ok(out),
        false => Err(Refusal::BAD_PARAMS),
    }
}

/// 打包失败照哪一种拒。
fn refused(session: &SessionId, error: BundleError) -> Refusal {
    match error {
        BundleError::Exists => Refusal::EXPORT_EXISTS,
        BundleError::BlobMismatch(hash) => {
            tracing::warn!(target: TARGET, session = session.as_str(), blob = hash.as_str(), "export blob mismatch");
            Refusal::BROKEN
        }
        BundleError::Io(error) => {
            tracing::warn!(target: TARGET, session = session.as_str(), error = %error, "export not written");
            Refusal::EXPORT_UNWRITABLE
        }
    }
}

/// 写到数据根里面的不收；放它的目录不在的 `export_unwritable`。比的是真实的位置，路径里经过链接的也认得出。
fn refuse_inside(root: &DataRoot, out: &Path) -> Result<(), Refusal> {
    let parent = out.parent().ok_or(Refusal::BAD_PARAMS)?;
    let parent = parent
        .canonicalize()
        .map_err(|_| Refusal::EXPORT_UNWRITABLE)?;
    let root = root.path().canonicalize().map_err(|_| Refusal::INTERNAL)?;
    if parent.starts_with(&root) {
        return Err(Refusal::EXPORT_IN_DATA_ROOT);
    }
    Ok(())
}

/// 这一刻，RFC 3339。
fn now() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            i64::try_from(since.as_millis()).unwrap_or(i64::MAX)
        });
    Timestamp::from_unix_millis(millis).map_or_else(String::new, |at| at.to_string())
}

/// 包里要放的会话：`main`、它的全部后代（照编号排），再是一层层往上的分叉来源（取到分叉点；同一个来源被要了几次的取最远的
/// 那一点；已经整个在包里的不再放）。父子关系先照会话列表的索引，索引里没有的读它的第一条。
pub(crate) fn gather(
    root: &DataRoot,
    index: Option<&SessionIndex>,
    main: &SessionId,
) -> io::Result<Vec<Part>> {
    let rows = index
        .and_then(|index| index.rows().ok())
        .unwrap_or_default();
    let mut parents: BTreeMap<SessionId, SessionId> = BTreeMap::new();
    for id in root.sessions()? {
        let parent = match rows.get(&id) {
            Some(row) => row.parent.clone(),
            None => created(root, &id).ok().and_then(|created| created.parent),
        };
        if let Some(parent) = parent {
            parents.insert(id, parent);
        }
    }
    let mut whole: Vec<SessionId> = vec![main.clone()];
    let mut seen: BTreeSet<SessionId> = BTreeSet::from([main.clone()]);
    let mut next = 0;
    while next < whole.len() {
        let current = whole[next].clone();
        next += 1;
        for (child, parent) in &parents {
            if *parent == current && seen.insert(child.clone()) {
                whole.push(child.clone());
            }
        }
    }
    let mut sources: BTreeMap<SessionId, Seq> = BTreeMap::new();
    let mut pending: Vec<SessionId> = whole.clone();
    while let Some(id) = pending.pop() {
        let Some(forked) = created(root, &id)?.forked else {
            continue;
        };
        if seen.contains(&forked.from) {
            continue;
        }
        let further = sources
            .get(&forked.from)
            .is_none_or(|upto| forked.upto > *upto);
        if further {
            sources.insert(forked.from.clone(), forked.upto);
            pending.push(forked.from);
        }
    }
    let mut parts: Vec<Part> = whole
        .into_iter()
        .map(|id| Part {
            dir: root.session_dir(&id),
            session: id,
            upto: None,
        })
        .collect();
    parts.extend(sources.into_iter().map(|(id, upto)| Part {
        dir: root.session_dir(&id),
        session: id,
        upto: Some(upto),
    }));
    Ok(parts)
}

/// 一个会话的第一条：`session.created`。
fn created(root: &DataRoot, id: &SessionId) -> io::Result<miyu_kernel::event::SessionCreated> {
    let first = first_event(&root.session_dir(id)).map_err(io::Error::other)?;
    match first.body {
        Body::SessionCreated(created) => Ok(created),
        _ => Err(io::Error::other(format!(
            "{id}: first event is not session.created"
        ))),
    }
}
