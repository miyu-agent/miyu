//! `session.export {format: "markdown"}`（施工 SE-1 中下上、中下中、中下下，设计 `58-会话导出和导入.md` 第一节，调研第五节第
//! 2、3、6、7 条，`docs/blueprint/protocol.md`「`session.export`」）：整个会话喂投影（`view/whole.rs`），照预设和开关用
//! `miyu_view::markdown` 渲染，写到人给的文件或者放进回应。
//!
//! - 参数（`export/params.rs`）：`for` 定预设，`range`、`thinking`、`tools` 写了的盖过预设；`children`、`assets` 两种写法。
//! - `range: context`（`agent` 的默认）：每一条另交给内核的有效历史（`Sources::keeping_history`），喂完落到检查点上，和请求
//!   组装看到的是同一份。
//! - `tools: full`：后台命令的输出、脚本里调的工具交回的结果去 blob 库取全文（`Sources::wanted`）；取不到的照没有。
//! - `children: append`：渲染进去的那几步派的子会话（`Draft::children`）一个个整份喂投影、照同一套开关渲染（范围一律
//!   `all`），照任务编号接在主会话后面；子会话又派的照样接（`j2.1` 跟在 `j2` 后面）。日志已经没了的子会话跳过，读不下去的
//!   `session_broken`。密钥、家目录、token 估算照接起来的整份算。
//! - `assets: files`（`export/assets.rs`）：只配写文件；每个会话渲染前查好它写到的 blob 取不取得到，取得到的写成链接、拷进
//!   旁边的文件夹，取不到的照占位、回应里列出来。
//! - `to: {"path"}` 同原始包写文件的规矩（绝对路径、数据根里面不写、不覆盖、先写临时文件再挂上去）；`to: "inline"` 放进回应的
//!   `text`，超过 [`INLINE_MOST`] 字节的拒 `export_too_big`（协议一行最长 1 MiB，给转义留余量）。
//! - 配了的密钥（`secrets.toml` 里的值）原样出现的换成 `[redacted: <名字>]`，回应的 `redacted` 报哪几个、各几处；原始包不改。
//! - 回应 `{path | text, bytes, tokens, upto, redacted, children, assets?}`：`tokens` 照底栏上下文用量的本地估算（UTF-8 字节数
//!   除以 4），`upto` 是主会话导到的最后一条，`children` 是附在后面的子会话，`assets` 只有 `assets: files` 才有。
//! - 原始包里的 `transcript.md` 也照这里渲染（[`transcript`]，`read` 预设）。
//! - 不写进会话日志；运行日志记一行 `INFO session exported`（会话、格式、多大、隐去几处、附了几个子会话、拷了几个附件，不记
//!   密钥的名字和值）。

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::estimate;
use miyu_kernel::id::{ContentHash, JobId, Seq, SessionId};
use miyu_kernel::time::UtcOffset;
use miyu_store::blob::Blobs;
use miyu_store::bundle;
use miyu_store::index::Row;
use miyu_store::log::OpenError;
use miyu_store::root::DataRoot;
use miyu_view::Texts;
use miyu_view::markdown::{
    About, Child, Document, Draft, Files, Options, Range, Rendered, Sources, Tools, assets, draft,
    finish,
};

use super::assets::{discard, folder_of, present, taken};
use super::params::{Target, Wanted, wanted};
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
    let wanted = wanted(params, peer.language)?;
    if !core.sessions.exists(core, &session).await {
        return Err(Refusal::NOT_FOUND);
    }
    let root = core.root.clone();
    let kit = Kit::of(core);
    let id = session.clone();
    let done = tokio::task::spawn_blocking(move || written(&root, &id, &kit, wanted))
        .await
        .map_err(|_| Refusal::INTERNAL)??;
    let bytes = done.made.rendered.text.len();
    tracing::info!(
        target: TARGET,
        session = session.as_str(),
        format = "markdown",
        bytes,
        redacted = done.made.rendered.redacted.values().sum::<usize>(),
        children = done.made.children.len(),
        assets = done.copied,
        "session exported"
    );
    let children: Vec<Value> = done
        .made
        .children
        .iter()
        .map(|child| json!({"job": child.job, "session": child.session, "title": child.title}))
        .collect();
    let mut reply = json!({
        "bytes": bytes,
        "tokens": estimate::text(&done.made.rendered.text),
        "upto": done.made.upto,
        "redacted": done.made.rendered.redacted,
        "children": children,
    });
    if let Some(missing) = &done.missing {
        reply["assets"] = json!({"files": done.copied, "missing": missing});
        if done.copied > 0 {
            reply["assets"]["dir"] = json!(done.folder);
        }
    }
    match done.target {
        Target::File(out) => reply["path"] = json!(out),
        Target::Inline => reply["text"] = json!(done.made.rendered.text),
    }
    Ok(reply)
}

/// 渲染好、写下了的。
struct Written {
    /// 渲染好的。
    made: Made,
    /// 写到哪。
    target: Target,
    /// 附件文件夹（`assets: files` 的）。
    folder: Option<PathBuf>,
    /// 拷进文件夹的附件有几个。
    copied: usize,
    /// `assets: files` 的：要写、取不到的 blob；别的是 `None`。
    missing: Option<BTreeSet<ContentHash>>,
}

/// 查好写得了、渲染、写下：碰磁盘，在阻塞线程里调。
fn written(root: &DataRoot, id: &SessionId, kit: &Kit, wanted: Wanted) -> Result<Written, Refusal> {
    let Wanted {
        target,
        language,
        options,
        append,
        files,
    } = wanted;
    let folder = match (&target, files) {
        (Target::File(out), true) => Some(folder_of(out)),
        _ => None,
    };
    if let Target::File(out) = &target {
        refuse_inside(root, out)?;
        taken(out, folder.as_ref().map(|(_, path)| path.as_path()))?;
    }
    let mut files = folder.as_ref().map(|(name, _)| Files::new(name.clone()));
    let made = transcript(root, id, kit, language, options, append, files.as_mut())?;
    let blobs = Blobs::new(root.blobs());
    let mut copied = 0;
    match &target {
        Target::File(out) => {
            if let (Some(files), Some((_, path))) = (&files, &folder) {
                copied = super::assets::write(path, files, &blobs)?;
            }
            if let Err(error) = bundle::write_file(out, made.rendered.text.as_bytes()) {
                if let (Some((_, path)), true) = (&folder, copied > 0) {
                    discard(path);
                }
                return Err(refused(id, error));
            }
        }
        Target::Inline if made.rendered.text.len() > INLINE_MOST => {
            return Err(Refusal::EXPORT_TOO_BIG);
        }
        Target::Inline => {}
    }
    Ok(Written {
        made,
        target,
        folder: folder.map(|(_, path)| path),
        copied,
        missing: files.map(|files| files.missing()),
    })
}

/// 渲染好的一份：接起来、最后过了一遍的 Markdown，主会话导到的最后一条，附在后面的子会话。
pub(super) struct Made {
    /// Markdown 和隐去了几处。
    pub(super) rendered: Rendered,
    /// 主会话导到的最后一条。
    pub(super) upto: Seq,
    /// 附在后面的子会话，照先后。
    pub(super) children: Vec<Child>,
}

/// 把会话 `session` 照 `options` 渲染成 Markdown（字照 `language`）：整份喂投影、记下条目里没有的原文，要的再取 blob；
/// `append` 的再把子会话一个个照同样渲染、照任务编号接在后面；`files` 是 `assets: files` 的附件文件夹（起名字、写链接）。
/// 碰磁盘：在阻塞线程里调。
///
/// # Errors
///
/// 没有这个会话的 `session_not_found`；日志读不下去的（子会话的也算）`session_broken`；给人看的字读不出来的 `internal`。
pub(super) fn transcript(
    root: &DataRoot,
    session: &SessionId,
    kit: &Kit,
    language: &'static str,
    options: Options,
    append: bool,
    mut files: Option<&mut Files>,
) -> Result<Made, Refusal> {
    let texts: Arc<Texts> = Arc::new(texts(&kit.resources, language, &kit.titles)?);
    let one = One {
        root,
        texts: &texts,
        blobs: Blobs::new(root.blobs()),
        offset: kit.offset,
    };
    let (main, upto) = one
        .draft(session, options, None, files.as_deref_mut())?
        .ok_or(Refusal::NOT_FOUND)?;
    let mut pending: BTreeMap<JobId, Child> = BTreeMap::new();
    if append {
        pending.extend(main.children.iter().map(|c| (c.job.clone(), c.clone())));
    }
    let mut seen: BTreeSet<SessionId> = BTreeSet::from([session.clone()]);
    let mut drafts = vec![main];
    let mut children = Vec::new();
    // 照任务编号一个个取：子会话又派的（`j2.1`）排在 `j2` 后面、`j3` 前面，取到它时正好接上。
    while let Some((_, child)) = pending.pop_first() {
        if !seen.insert(child.session.clone()) {
            continue;
        }
        let whole = Options {
            range: Range::All,
            ..options
        };
        let Some((drafted, _)) =
            one.draft(&child.session, whole, Some(&child), files.as_deref_mut())?
        else {
            continue;
        };
        pending.extend(drafted.children.iter().map(|c| (c.job.clone(), c.clone())));
        drafts.push(drafted);
        children.push(child);
    }
    let secrets: Vec<(&str, &str)> = kit
        .secrets
        .iter()
        .map(|(name, value)| (name.as_str(), value.as_str()))
        .collect();
    let rendered = finish(&drafts, &secrets, texts.home.as_deref());
    Ok(Made {
        rendered,
        upto,
        children,
    })
}

/// 渲染一个会话要的、几个会话共用的。
struct One<'a> {
    /// 数据根。
    root: &'a DataRoot,
    /// 给人看的字。
    texts: &'a Arc<Texts>,
    /// blob 库。
    blobs: Blobs,
    /// 时区。
    offset: UtcOffset,
}

impl One<'_> {
    /// 一个会话渲染成一份草稿，交回它和导到的最后一条；日志不在的（一条都没有的也算）交回 `None`。
    fn draft(
        &self,
        session: &SessionId,
        options: Options,
        child: Option<&Child>,
        files: Option<&mut Files>,
    ) -> Result<Option<(Draft, Seq)>, Refusal> {
        let mut sources = match options.range {
            Range::Context => Sources::keeping_history(),
            Range::All | Range::Turns(_) => Sources::default(),
        };
        let mut row: Option<Row> = None;
        let fed = whole(
            &self.root.session_dir(session),
            Arc::clone(self.texts),
            self.blobs.clone(),
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
        );
        let fed = match fed {
            Ok(fed) => fed,
            Err(OpenError::Missing(_)) => return Ok(None),
            Err(error) => return Err(crate::view::unread(session, error)),
        };
        sources.settle();
        let entries = fed.projector.entries();
        if options.tools == Tools::Full {
            for hash in sources.wanted(entries) {
                if let Ok(bytes) = self.blobs.get(&hash) {
                    sources
                        .blobs
                        .insert(hash, String::from_utf8_lossy(&bytes).into_owned());
                }
            }
        }
        let files = files.map(|files| {
            files.have(present(&self.blobs, assets(entries)));
            &*files
        });
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
        let drafted = draft(&Document {
            entries,
            about: &about,
            sources: &sources,
            texts: self.texts,
            options,
            offset: self.offset,
            secrets: &[],
            child,
            files,
        });
        Ok(Some((drafted, fed.upto)))
    }
}
