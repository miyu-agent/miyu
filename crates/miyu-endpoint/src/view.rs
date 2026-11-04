//! `view.page`：历史按页读（施工 9-6 下，`docs/blueprint/protocol.md`「`view.page`」）；`view.detail`：一次调用的完整差异
//! （施工 9-6 三补，`view/detail.rs`）。头接一个老会话时先拿最新一页画出来，
//! 再 `subscribe {"after": last}` 只接新的；往上翻到顶拿这一页的 `first` 当 `before` 要更早的一页。页里是原始事件，写法同补发；
//! 写了 `view: true` 的换成视图投影算好的条目（施工 9-8 中，`view/project.rs`，`docs/blueprint/view.md`）；视图流订阅时
//! 照同一个切法拿最新一页（施工 9-8 下，[`newest`]）。怎么切在 `view/page.rs`。
//!
//! 只读地读会话目录里的日志，不为翻历史载入会话：读的时候会话照常跑，正在写的那半行不算。只读一页要的那一截（施工 V-41
//! 上，`view/read.rs`）：先数一遍行、不解，再只解最后几轮和切点前要的那几行；原来每一页都整份读（`read_events`）。

mod detail;
mod page;
mod project;
mod read;
pub(crate) mod status;
pub(crate) mod tree;

pub(crate) use detail::detail;
pub(crate) use read::Slice;

use std::collections::BTreeMap;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use std::path::Path;

use miyu_kernel::event::Event;
use miyu_kernel::id::SessionId;
use miyu_store::blob::Blobs;
use miyu_store::log::{OpenError, read_events};
use miyu_store::packages::Found;
use miyu_store::resources::ResourceRoot;
use miyu_store::root::DataRoot;
use miyu_view::{Projector, Texts};

use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 不写 `turns` 的一页几轮。
const TURNS: usize = 20;

/// 一页最多几轮。
const MOST_TURNS: usize = 50;

/// 一页到了这么多字节就在回合之间提前切：1 MiB。至少一整轮。
const CAP: usize = 1 << 20;

/// `view.page` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PageParams {
    /// 会话编号。
    session: String,
    /// 只要序号小于它的；不写的是最新一页。
    #[serde(default)]
    before: Option<u64>,
    /// 最多几轮：1 到 50，不写是 20。
    #[serde(default)]
    turns: Option<usize>,
    /// 写 `true` 的交条目、不交事件（施工 9-8 中）。
    #[serde(default)]
    view: bool,
}

/// `view.page`：回应 `{"events", "more"}`，有事件的再带 `first`、`last`，因为字节少给了轮数的带 `capped: true`，这一页里报完了、
/// 在切点前派出去的任务带 `jobs`。写了 `view: true` 的 `events` 换成 `entries`，照这个连接的语言算（施工 9-8 中）。
pub(crate) async fn page(core: &Core, peer: Peer, params: PageParams) -> Result<Value, Refusal> {
    let session = SessionId::parse(&params.session).map_err(|_| Refusal::BAD_PARAMS)?;
    let turns = params.turns.unwrap_or(TURNS);
    if !(1..=MOST_TURNS).contains(&turns) || params.before == Some(0) {
        return Err(Refusal::BAD_PARAMS);
    }
    if !core.sessions.exists(core, &session).await {
        return Err(Refusal::NOT_FOUND);
    }
    let dir = core.root.session_dir(&session);
    // 要条目的：投影要的字照这个连接的语言读，改了多少行照 blob 算。
    let view = match params.view {
        true => Some((
            (core.resources.clone(), Titles::of(core)),
            peer.language,
            Blobs::new(core.root.blobs()),
        )),
        false => None,
    };
    let before = params.before;
    // 读日志、切页、算条目都在阻塞线程里：先读（读不了的先报），再读投影要的字；投影是一条条喂的纯计算。
    tokio::task::spawn_blocking(move || {
        let slice = read::read(&dir, before, (turns, CAP), view.is_some(), &size)
            .map_err(|error| unread(&session, error))?;
        let view = match view {
            Some(((resources, skills), language, blobs)) => {
                Some((Arc::new(texts(&resources, language, &skills)?), blobs))
            }
            None => None,
        };
        Ok(paged(&slice, view).0)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?
}

/// 视图流订阅时的最新一页要的（施工 9-8 下；施工 V-41 上起只读要的那一截）：会话目录 `dir` 里到第 `upto` 条（订阅那一刻落了盘
/// 的最后一条）为止。`upto` 是 0 的一条都没有，不读。碰磁盘。
///
/// # Errors
///
/// 同 `view.page` 读日志。
pub(crate) fn newest_of(dir: &Path, upto: u64) -> Result<Slice, OpenError> {
    if upto == 0 {
        return Ok(Slice::empty());
    }
    read::read(dir, upto.checked_add(1), (TURNS, CAP), true, &size)
}

/// 最新一页写成视图流订阅的回应的那几格（施工 9-8 下）：交回回应和喂过这一页的投影，之后的推送接着喂它。
pub(crate) fn newest(slice: &Slice, texts: Texts, blobs: Blobs) -> (Value, Projector) {
    let (reply, projector) = paged(slice, Some((Arc::new(texts), blobs)));
    (
        reply,
        projector.unwrap_or_else(|| unreachable!("交了字就有投影")),
    )
}

/// 投影要的字：照资源目录读 `language` 一份、英文一份，技能、工具的给人看的名字照 `titles` 挑（施工 9-8 四补、六补）。读不成的
/// 记一行日志、回内部出错。碰磁盘。
pub(crate) fn texts(
    resources: &ResourceRoot,
    language: &str,
    titles: &Titles,
) -> Result<Texts, Refusal> {
    let mut texts = project::texts(resources, language).map_err(|project::Unreadable(why)| {
        tracing::warn!(target: "miyu::endpoint", why = %why, "view words not read");
        Refusal::INTERNAL
    })?;
    texts.skills = titles.skills(language);
    texts.tools = titles.tools(language);
    texts.subjects = titles.subjects.clone();
    texts.packages.clone_from(&titles.packages_of);
    Ok(texts)
}

/// 投影兜底用的给人看的名字从哪来：找技能的地方（施工 9-8 四补：看得到的包、数据根，施工 U23 上下；拿到阻塞线程里再找），工具
/// 目录里这一刻的工具交的显示名（施工 9-8 六补：脚本、MCP 的、扩展的工具，照语言）。
#[derive(Clone)]
pub(crate) struct Titles {
    packages: Arc<Vec<Found>>,
    root: DataRoot,
    /// 配置开着的别的目录（施工 PT-2 外）。
    external: Vec<miyu_store::skills::External>,
    tools: BTreeMap<String, BTreeMap<String, String>>,
    /// 工具目录里这一刻的工具自己说的标题后面跟哪个参数（施工 9-8 十一补，网页报的：扩展登记的 `subject` 没进标题）。
    subjects: BTreeMap<String, String>,
    /// 每件工具属于哪个包（施工 X-24，收起那一行判「只为发一张图」）。
    packages_of: BTreeMap<String, String>,
}

impl Titles {
    /// 看得到的（施工 U23 上下，[`Core::packages_of`]）。
    pub(crate) fn of(core: &Core) -> Titles {
        let catalog = core.tools.current();
        let tools = catalog
            .specs()
            .filter_map(|spec| {
                let names = catalog.get(&spec.name)?.display_name();
                (!names.is_empty()).then(|| (spec.name.clone(), names))
            })
            .collect();
        let subjects = catalog
            .specs()
            .filter_map(|spec| Some((spec.name.clone(), catalog.get(&spec.name)?.subject()?)))
            .collect();
        let packages_of = catalog
            .specs()
            .filter_map(|spec| {
                let package = catalog.package_of(&spec.name)?;
                Some((spec.name.clone(), package.to_string()))
            })
            .collect();
        Titles {
            packages_of,
            packages: core.packages_of(),
            root: core.root.clone(),
            external: crate::skills::external(core),
            tools,
            subjects,
        }
    }

    /// 技能的名字到给人看的名字，照语言 `language` 挑，没写的照名字。碰磁盘。
    fn skills(&self, language: &str) -> BTreeMap<String, String> {
        miyu_store::skills::find(&self.packages, &self.root, &self.external)
            .0
            .into_iter()
            .map(|skill| {
                let title = crate::personas::pick(&skill.head.display_name, language)
                    .unwrap_or_else(|| skill.head.name.clone());
                (skill.head.name, title)
            })
            .collect()
    }

    /// 写了显示名的工具的名字到给人看的名字，照语言 `language` 挑。
    fn tools(&self, language: &str) -> BTreeMap<String, String> {
        self.tools
            .iter()
            .filter_map(|(name, names)| {
                Some((name.clone(), crate::personas::pick(names, language)?))
            })
            .collect()
    }
}

/// 一条事件多少字节：写成一行的长度（切页照它算 1 MiB）。
fn size(event: &Event) -> usize {
    serde_json::to_string(event).map_or(0, |line| line.len())
}

/// 读好、切好的一页写成回应：`view` 有的交条目，连同喂过这一页的投影；没有的交事件。
fn paged(slice: &Slice, view: Option<(Arc<Texts>, Blobs)>) -> (Value, Option<Projector>) {
    let cut = slice.page();
    let (mut reply, projector) = match view {
        Some((texts, blobs)) => {
            // 切点前的日志只学派出去的任务：只交派出、了结任务的那几条（`view/read.rs`），学出来的和整份一样。
            let projector = project::fed(texts, blobs, slice.learned(), &cut.events);
            (
                json!({"entries": projector.entries(), "more": cut.more}),
                Some(projector),
            )
        }
        None => (json!({"events": cut.events, "more": cut.more}), None),
    };
    if let (Some(first), Some(last)) = (cut.first, cut.last) {
        reply["first"] = json!(first);
        reply["last"] = json!(last);
    }
    if cut.capped {
        reply["capped"] = json!(true);
    }
    // 这一页里报完了、在切点前派出去的任务（施工 9-6 再补）：没有的不写。
    if !cut.jobs.is_empty() {
        reply["jobs"] = json!(cut.jobs);
    }
    (reply, projector)
}

/// 读不了日志：没有这个会话的 `session_not_found`，别的记一行日志、回 `session_broken`。
fn unread(session: &SessionId, error: OpenError) -> Refusal {
    match error {
        OpenError::Missing(_) => Refusal::NOT_FOUND,
        error => {
            tracing::warn!(target: "miyu::endpoint", session = session.as_str(), error = %error, "log not read");
            Refusal::BROKEN
        }
    }
}

/// 只读地读会话 `session` 的整份日志，交回事件（`view.detail` 用）。没有这个会话的 `session_not_found`，读不下去的
/// `session_broken`。
async fn events_of(core: &Core, session: &SessionId) -> Result<Vec<Event>, Refusal> {
    if !core.sessions.exists(core, session).await {
        return Err(Refusal::NOT_FOUND);
    }
    let dir = core.root.session_dir(session);
    let read = tokio::task::spawn_blocking(move || read_events(&dir))
        .await
        .map_err(|_| Refusal::INTERNAL)?;
    read.map_err(|error| unread(session, error))
}
