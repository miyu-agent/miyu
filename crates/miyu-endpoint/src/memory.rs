//! 人经协议碰记忆（施工 R-3 补，`docs/blueprint/memory.md`「协议」、第二条、第九条；`protocol.md` 的 `memory.*`）：列、搜、
//! 记、改、忘和清空，恢复作废的（施工 R-12 下）。怎么记、怎么挑在会话那一层的 [`Keeper`]，她的工具也用它；这里只管找哪一间、
//! 读参数、写回应。现在就整理（`memory.dream`、`/dream`，施工 R-7 补）在 `memory/dream.rs`，搜以前的对话（`memory.turns`，
//! 施工 R-12 下）在 `memory/turns.rs`。
//!
//! - 哪一间：`persona` 是这个人格那一间（记忆账号照 `personas.md` 第 5 条，属主是这个连接的账号）；`session` 是那个会话用的
//!   那一间（照会话交出来的 [`Handle::memory_room`]）；都不写照默认人格，没有默认人格的没有哪一间（无人格不是人格，施工 P-12）；
//!   `persona` 写 `_none`（[`NO_PERSONA`]）的是无人格那一间（施工 R-12 再补：网页的记忆页没有会话可借）。
//! - 听众是这个连接的人：本机的令牌、网页登录都是管理员。代表外部的人（`as`）不收，随 O 线。
//! - 碰记忆日志、检索库的在阻塞线程里做。

mod checks;
pub(crate) mod dream;
mod params;
mod shown;
pub(crate) mod slash;
#[cfg(test)]
mod tests;
mod turns;
mod write;

use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::{Value, json};

use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_recall::MemoryId;
use miyu_session::{
    ConfigSource, EmbedSetup, Embedder, Filter, Handle, Keeper, Listener, Memory, Stamp, Turn,
    Using, Vectors,
};
use miyu_store::recall::{NO_PERSONA, Room};
use miyu_tool::{Refused, Remember};

use crate::Core;
use crate::list::LOCAL;
use crate::personas;
use crate::refusal::Refusal;
use crate::sessions::{admin, now};

use checks::{checked, limit};
use params::{Forget, List, Restore, Search, Where};
use shown::shown_all;

impl Core {
    /// 同一份家底，接上照意思找的那一路（施工 R-5 下、补，`recall.md` 第四条）：本机的照 `local` 算（没装内置语义模型的是
    /// 空的，装卸以后照新的清单换，`Vectors::replace_local`），远程的照模型资料里的供应商发。核心起来时接一次，接在
    /// [`Core::with_model_data`] 后面（远程的照那一份查供应商、记账）；不接的（测试里）只照关键词找。（施工 R-10 从 `lib.rs`
    /// 挪过来。）
    #[must_use]
    pub fn with_vectors(self, local: Option<EmbedSetup>) -> Core {
        let vectors = Vectors::new(local.map(Embedder::new), Arc::clone(&self.model_data));
        self.memory.give_vectors(Arc::new(vectors));
        self
    }

    /// 账号 `owner` 的会话用的记忆的登记，交给造的、载入的会话（施工 R-2 上、R-3 中）：管理员和系统账号的会话有（施工 O-4
    /// 下）；记忆归哪个账号另照 [`Core::memory_owner`] 算，系统账号的归管理员。
    pub(crate) fn memory_for(&self, owner: &AccountId) -> Option<Arc<Memory>> {
        self.knows(owner).then(|| Arc::clone(&self.memory))
    }
}

/// `memory.list` 不写 `limit` 时给几条。
const LIST: usize = 50;
/// `memory.search` 不写 `limit` 时给几条。
const SEARCH: usize = 10;

/// 按顺序答的 `memory.*`（`methods.rs` 照它转过来）；`memory.dream` 在后台答，另走。
pub(crate) const METHODS: [&str; 8] = [
    "memory.list",
    "memory.search",
    "memory.turns",
    "memory.remember",
    "memory.update",
    "memory.forget",
    "memory.restore",
    "memory.dream_stop",
];

/// 照方法名办一条 `memory.*`，命令编号是 `id`；不是 [`METHODS`] 里的没有这个方法。
pub(crate) async fn call(
    core: &Arc<Core>,
    method: &str,
    id: &CommandId,
    params: &Value,
) -> Result<Value, Refusal> {
    if !installed(core) {
        return Err(Refusal::MEMORY_NOT_INSTALLED);
    }
    match method {
        "memory.list" => list(core, read(params)?).await,
        "memory.search" => search(core, read(params)?).await,
        "memory.turns" => turns::turns(core, read(params)?).await,
        "memory.remember" => write::remember(core, stamp(core, id), read(params)?).await,
        "memory.update" => write::update(core, stamp(core, id), read(params)?).await,
        "memory.forget" => forget(core, stamp(core, id), read(params)?).await,
        "memory.restore" => restore(core, stamp(core, id), read(params)?).await,
        "memory.dream_stop" => dream::stop(core, params).await,
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// 会话 `handle` 里人记一条（`/remember`，施工 R-3 补）：记进这个会话那一间，类 `user`，出处空，听众是 `by`，命令编号是
/// `id`（同一个编号再来交回头一次的）。交回记下的那句话（去掉了前后空白的）：回执照它说，不露编号（施工 R-3 四补）。
///
/// # Errors
///
/// 这个会话没有记忆（范围 `off`、场所会话）：`memory_unavailable`；空的：`bad_params`；超长：`memory_too_long`。
pub(crate) async fn remember_in(
    core: &Arc<Core>,
    handle: &Handle,
    id: &CommandId,
    by: By,
    text: &str,
) -> Result<String, Refusal> {
    let keeper = keeper_of(core, handle, by.clone())?;
    let text = checked(text)?;
    let remember = Remember {
        class: "user".to_string(),
        text: text.clone(),
        replaces: None,
        keywords: None,
        who: None,
    };
    let stamp = Stamp {
        at: now(),
        by,
        cause: Some(id.clone()),
    };
    blocking({
        let keeper = keeper.clone();
        move || keeper.save(stamp, remember, Vec::new())
    })
    .await?;
    fill(core, keeper).await;
    Ok(text)
}

/// 会话 `handle` 产生的记忆清掉（`/clear-memory`，施工 R-3 五补）：同 `memory.forget` 带 `clear: "session"`，出处全在这个会话
/// 里的清掉，别的出处还有的、人记的（没有出处）不动；会话那一间的就是整间。`by` 是打命令的人，命令编号是 `id`。交回清掉了几条
/// （不算改掉的旧版本）。
///
/// # Errors
///
/// 这个会话没有记忆（范围 `off`、场所会话、没装人格记忆）：`memory_unavailable`；记忆日志写不进：内部出错。
pub(crate) async fn clear_in(
    core: &Arc<Core>,
    handle: &Handle,
    id: &CommandId,
    by: By,
) -> Result<usize, Refusal> {
    let keeper = keeper_of(core, handle, by.clone())?;
    let session = matches!(keeper.room(), Room::Persona { .. }).then(|| handle.id().clone());
    let stamp = Stamp {
        at: now(),
        by,
        cause: Some(id.clone()),
    };
    blocking(move || keeper.clear(stamp, session)).await
}

async fn list(core: &Arc<Core>, params: List) -> Result<Value, Refusal> {
    let filter = Filter {
        class: params.class,
        from: params.from.as_deref().map(session_id).transpose()?,
        forgotten: params.forgotten,
        limit: limit(params.limit, LIST)?,
    };
    let keeper = find(core, params.at).await?;
    let (found, used) = blocking(move || {
        let found = keeper.list(&filter).map_err(Refused::Failed)?;
        Ok((found, keeper.used().map_err(Refused::Failed)?))
    })
    .await?;
    Ok(json!({"memories": shown_all(&found, &used)}))
}

async fn search(core: &Arc<Core>, params: Search) -> Result<Value, Refusal> {
    if params.query.trim().is_empty() {
        return Err(Refusal::BAD_PARAMS);
    }
    let limit = limit(params.limit, SEARCH)?;
    let keeper = find(core, params.at).await?;
    let (query, forgotten) = (params.query, params.forgotten);
    // 照意思找（施工 R-5 下）照这时的配置：先算问句的向量（`off` 的、等不到的只走关键词），搜完在后台补这一间缺的。远程的
    // 用量记在管理员名下（M8 只有他，和 `model.call` 一样）。
    let using = using(core);
    let near = match keeper.vectors() {
        Some(vectors) => vectors.query(&using, &query).await,
        None => None,
    };
    let searched = keeper.clone();
    let (found, used) = blocking(move || {
        let found = searched
            .search(&query, forgotten, limit, near.as_ref())
            .map_err(Refused::Failed)?;
        Ok((found, searched.used().map_err(Refused::Failed)?))
    })
    .await?;
    if keeper.vectors().is_some() {
        blocking(move || {
            keeper.fill(&using);
            Ok(())
        })
        .await?;
    }
    Ok(json!({"memories": shown_all(&found, &used)}))
}

async fn forget(core: &Arc<Core>, stamp: Stamp, params: Forget) -> Result<Value, Refusal> {
    match (params.id, params.clear.as_deref()) {
        (Some(id), None) => {
            let id = memory_id(&id)?;
            let why = params.why.unwrap_or_default();
            let keeper = find(core, params.at).await?;
            blocking(move || keeper.retire(stamp, id, why)).await?;
            Ok(json!({}))
        }
        (None, Some(clear @ ("session" | "me"))) => {
            // 清会话的：那个会话就是写的 `session`；会话那一间的就是整间。
            let session = match clear {
                "session" => Some(session_id(
                    params.at.session.as_deref().ok_or(Refusal::BAD_PARAMS)?,
                )?),
                _ => None,
            };
            let keeper = find(core, params.at).await?;
            let session = session.filter(|_| matches!(keeper.room(), Room::Persona { .. }));
            let cleared = blocking(move || keeper.clear(stamp, session)).await?;
            Ok(json!({"cleared": cleared}))
        }
        _ => Err(Refusal::BAD_PARAMS),
    }
}

/// 恢复作废的一条（施工 R-12 下）：本来就算数的什么都不写，也回 `{}`。
async fn restore(core: &Arc<Core>, stamp: Stamp, params: Restore) -> Result<Value, Refusal> {
    let id = memory_id(&params.id)?;
    let keeper = find(core, params.at).await?;
    blocking(move || keeper.restore(stamp, id)).await?;
    Ok(json!({}))
}

/// 人经协议写的：此刻、管理员、这条命令的编号。
fn stamp(core: &Core, id: &CommandId) -> Stamp {
    Stamp {
        at: now(),
        by: admin(core),
        cause: Some(id.clone()),
    }
}

/// 照参数找哪一间，听众是这个连接的人（管理员）。
async fn find(core: &Arc<Core>, at: Where) -> Result<Keeper, Refusal> {
    let hearer = admin(core);
    match (at.persona, at.session, at.as_external) {
        (_, _, Some(_)) | (Some(_), Some(_), None) => Err(Refusal::BAD_PARAMS),
        (Some(persona), None, None) if persona == NO_PERSONA => {
            // 无人格那一间（施工 R-12 再补）：记忆账号是这个连接的账号，同无人格的会话照属主。
            let room = Room::persona(&core.admin, NO_PERSONA);
            Ok(Keeper::new(&memory(core)?, room, Listener::Owner(hearer)))
        }
        (persona, None, None) => {
            // 无人格不是人格（施工 P-12）：不写、默认人格也没设的，照人格找没有哪一间；无人格那一间照会话找。
            let found = personas::resolve(core, persona.as_deref().map(Some))
                .await?
                .ok_or(Refusal::MEMORY_UNAVAILABLE)?;
            let account = personas::memory_account(Some(&found), &core.admin);
            let room = Room::persona(&account, &found.id);
            Ok(Keeper::new(&memory(core)?, room, Listener::Owner(hearer)))
        }
        (None, Some(session), None) => {
            let session = session_id(&session)?;
            let found = core.sessions.get(core, &session).await?;
            keeper_of(core, &found.handle, hearer)
        }
    }
}

/// 一间记忆日志订阅里挑过的、之后的最多攒几条没写出去的（施工 R-12 上，`protocol.md`「慢和掉队」第 1 条）：同包的变化。
/// 记忆一轮至多追加几条，梦一次几十条；头的写队列（256 行）堵着才攒得起来，攒满了掉队，带 `after` 重新订阅补回来。
const FOLLOWING: usize = 64;

/// 跟着看 `at` 那一间的记忆日志（施工 R-12 上，流 `memory`）：找哪一间同 `memory.*`，听众是这个连接的人；`after` 以后的补。
///
/// # Errors
///
/// 人格记忆没装：`memory_not_installed`；找不到、写错了的同 `memory.*`；记忆日志读不了：`internal_error`，记一行。
pub(crate) async fn follow(
    core: &Arc<Core>,
    at: &crate::subscriptions::MemoryAt,
    after: Option<u64>,
) -> Result<miyu_session::Following, Refusal> {
    if !installed(core) {
        return Err(Refusal::MEMORY_NOT_INSTALLED);
    }
    let keeper = find(
        core,
        Where {
            persona: at.persona.clone(),
            session: at.session.clone(),
            as_external: None,
        },
    )
    .await?;
    blocking(move || keeper.follow(after, FOLLOWING).map_err(Refused::Failed)).await
}

/// 这个会话能不能记（施工 O-6 补，`/remember` 在 `command.catalog` 里列不列）：同 [`keeper_of`] 的判法。
pub(crate) fn remembers(core: &Core, handle: &Handle) -> bool {
    room(handle).is_some() && memory(core).is_ok()
}

/// 会话 `handle` 用的那一间，听众是 `hearer`：本机的、记忆开着的才有。
fn keeper_of(core: &Core, handle: &Handle, hearer: By) -> Result<Keeper, Refusal> {
    let room = room(handle).ok_or(Refusal::MEMORY_UNAVAILABLE)?;
    Ok(Keeper::new(&memory(core)?, room, Listener::Owner(hearer)))
}

/// 会话 `handle` 用的那一间：本机的、记忆开着的才有。
fn room(handle: &Handle) -> Option<Room> {
    handle
        .memory_room()
        .filter(|_| handle.venue().as_str() == LOCAL)
}

/// 照意思找的那一路照这时的配置（施工 R-5 下）：远程的用量记在管理员名下（M8 只有他，和 `model.call` 一样）。
pub(crate) fn using(core: &Core) -> Using {
    let config = core.config_now().borrow().clone();
    let resolved = config.resolved().clone();
    let source: Arc<dyn ConfigSource> = config;
    Using {
        config: Arc::new(Turn::new(resolved, source)),
        owner: core.admin.clone(),
    }
}

/// 记下、改了以后在后台补这一间缺的向量（施工 R-5 五补：不等下一次搜）。没接向量的、`off` 的什么都不做。
async fn fill(core: &Core, keeper: Keeper) {
    if keeper.vectors().is_none() {
        return;
    }
    let using = using(core);
    // 记下已经成了，回应照常：补不成的 `blocking` 记了 `WARN memory failed`，补本身的出错 `Keeper::fill` 自己记。
    let _filled: Result<(), Refusal> = blocking(move || {
        keeper.fill(&using);
        Ok(())
    })
    .await;
}

/// 人格记忆这个软件包这时装着没有（施工 R-10）：核心照清单设在记忆上（`Memory::set_installed`）。没有记忆的核心照装着算，
/// 由后面的检查说这里没有记忆。
pub(crate) fn installed(core: &Core) -> bool {
    core.memory_for(&core.admin)
        .is_none_or(|memory| memory.installed())
}

/// 核心一份的记忆：管理员的才有（多用户以后照账号）。人格记忆没装的当没有（施工 R-10）。
fn memory(core: &Core) -> Result<Arc<miyu_session::Memory>, Refusal> {
    core.memory_for(&core.admin)
        .filter(|memory| memory.installed())
        .ok_or(Refusal::MEMORY_UNAVAILABLE)
}

fn memory_id(text: &str) -> Result<MemoryId, Refusal> {
    MemoryId::parse(text).ok_or(Refusal::BAD_PARAMS)
}

fn session_id(text: &str) -> Result<SessionId, Refusal> {
    SessionId::parse(text).map_err(|_| Refusal::BAD_PARAMS)
}

fn read<T: DeserializeOwned>(params: &Value) -> Result<T, Refusal> {
    serde_json::from_value(params.clone()).map_err(|_| Refusal::BAD_PARAMS)
}

/// 在阻塞线程里做，[`Refused`] 写成拒绝：没有的、听众不合的 `unknown_memory`，不算了的 `memory_not_current`，磁盘上出的错
/// 记一行、是内部出错。
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, Refused> + Send + 'static,
) -> Result<T, Refusal> {
    match tokio::task::spawn_blocking(work).await {
        Ok(Ok(done)) => Ok(done),
        Ok(Err(Refused::NoSuch(_))) => Err(Refusal::UNKNOWN_MEMORY),
        Ok(Err(Refused::NotCurrent(_))) => Err(Refusal::MEMORY_NOT_CURRENT),
        Ok(Err(Refused::Failed(error))) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "memory failed");
            Err(Refusal::INTERNAL)
        }
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", panic = error.is_panic(), "memory failed");
            Err(Refusal::INTERNAL)
        }
    }
}

/// 联想要的几样（施工 R-8，`memory.md` 第四条）：外壳的字、门槛表照资源目录读好交给记忆；读不出来的（安装坏了）记一行，这个
/// 核心不联想，别的照常。
pub(crate) fn give_recall(
    memory: &miyu_session::Memory,
    resources: &miyu_store::resources::ResourceRoot,
) {
    match miyu_session::RecallTexts::load(resources) {
        Ok(recall) => {
            memory.give_recall(recall);
        }
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", error = error.as_str(), "memory recall texts unreadable");
        }
    }
}

/// 抽取要的几样（施工 R-6 上，`memory.md` 第六条）：请求模型的照一次性入口（照剧本回的端口没有，不抽）、资源目录里的字和 key 的
/// 写法、管理员的 blob（抽取的请求只有字，用不上）。读不出来的记一行，没有：这个核心不抽，别的照常。
pub(crate) fn extraction(
    models: &dyn miyu_session::Models,
    resources: &miyu_store::resources::ResourceRoot,
    root: &miyu_store::root::DataRoot,
    admin: &miyu_kernel::id::AccountId,
) -> Option<miyu_session::Extraction> {
    let ask = models.one_shot()?;
    let texts = miyu_session::ExtractTexts::load(resources.path())
        .and_then(|texts| {
            Ok((
                texts,
                miyu_session::MergeTexts::load(resources.path())?,
                miyu_session::TagTexts::load(resources.path())?,
            ))
        })
        .map_err(|error| error.to_string());
    let shapes = resources
        .memory_secrets()
        .map_err(|error| error.to_string())
        .and_then(|text| miyu_recall::redact::KeyShapes::parse(&text));
    match texts.and_then(|texts| shapes.map(|shapes| (texts, shapes))) {
        Ok(((texts, merge, tag), shapes)) => Some(miyu_session::Extraction {
            texts,
            shapes,
            ask,
            blobs: miyu_store::blob::Blobs::new(root.blobs(admin)),
            idle: None,
            merge,
            tag: Some(tag),
        }),
        Err(error) => {
            tracing::warn!(target: "miyu::endpoint", error = error.as_str(), "memory extraction unavailable");
            None
        }
    }
}
