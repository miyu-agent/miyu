//! 场所会话与外部身份（施工 O-3，`docs/blueprint/venues.md`）：主人对应表 `external.bindings` 怎么查，扩展经 `venue.binding`
//! 问一个平台身份对着谁（施工 O-31 前）；`venue.session` 找回或者造
//! 一个场所的主线会话，`venue.sessions` 列出平台桥那个平台的（施工 O-32 前，`sessions`）；`session.send` 的 `as` 记成谁。核心不认识 QQ：场所编号不解读，是不是私聊、对方是谁，桥照实报。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::{CommandId, ExternalId, VenueId};
use miyu_kernel::origin::{By, External, Person, Role};
use miyu_store::index::Row;
use miyu_tool::Stop;

use crate::Core;
use crate::hello::Caller;
use crate::list::{Listed, scan};
use crate::refusal::Refusal;
use crate::sessions::Opening;

mod bridge;
mod message;
mod plugins;
mod records;
mod sessions;
pub(crate) use bridge::bridge;
pub(crate) use sessions::platforms_of_package;

pub(crate) use message::{VenueMessageParams, platform_id};
pub(crate) use plugins::plugins;
pub(crate) use records::records;
pub(crate) use sessions::{SessionsParams, sessions};

/// 主人对应表在配置里的样子（`settings::EXTERNAL_BINDINGS`）。
const BINDINGS: &str = "external.bindings.<external>";

/// `venue.session` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VenueParams {
    venue: String,
    kind: Kind,
    #[serde(default)]
    peer: Option<String>,
    #[serde(default)]
    cwd: Option<String>,
    /// 新造的会话用哪个人格（施工 P-1 上）：桥照场所规则算好交来，核心不读场所规则；不写的照默认人格，写 `null` 的明着
    /// 无人格（施工 P-4 上）。找回已有的会话时不看。
    #[serde(default, deserialize_with = "crate::personas::written")]
    persona: Option<Option<String>>,
    /// 新造的会话用哪个预设（施工 P-2 上）：同 `persona`，桥照场所规则算好交来；不写的照默认预设。找回已有的会话时不看。
    #[serde(default)]
    preset: Option<String>,
    /// 新造的会话带不带你写的「关于你」（施工 P-9）：桥照场所规则算好交来，你本人的私聊这类才写 `true`；不写的
    /// 不带，群聊不写。找回已有的会话时不看。
    #[serde(default)]
    profile: bool,
    /// 这个场所显示得了图片吗（施工 X-25）：语音这类显示不了的写 `false`，要图的包在这个会话里整包当没开；不写的是显示得了。
    /// 找回已有的会话时不看。
    #[serde(default = "shows_images")]
    images: bool,
    /// 这个会话自动起标题吗（施工 O-43）：机器人这类不要标题的写 `false`；不写的起。找回已有的会话时不看。
    #[serde(default = "shows_images")]
    titles: bool,
    /// 群聊近况最多带几条（施工 O-13 再补，接入通讯平台要的）：只给群，1 到 [`RECENT_MOST`]；不写的只照字节预算。
    #[serde(default)]
    recent: Option<u64>,
}

/// 群聊近况最多能写几条。
const RECENT_MOST: u64 = 1000;

/// 没写 `images` 的场所显示得了图片（施工 X-25）。
pub(crate) fn shows_images() -> bool {
    true
}

/// 场所是私聊还是群。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Kind {
    Private,
    Group,
}

/// `session.send` 的 `as`：代表通讯平台上的哪个人，他在场所里是什么身份。
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct AsParams {
    external: String,
    #[serde(default)]
    role: Option<Role>,
}

/// 平台身份 `id` 在主人对应表里是不是主人。
pub(crate) fn bound(core: &Core, id: &ExternalId) -> bool {
    owners_ids(core).iter().any(|one| one == id.as_str())
}

/// 主人对应表里是主人的平台身份（施工 SU-1 补：写了非空的字就是；SU-3 改成只列平台身份）。
/// 值不是字的、空的不算。
fn owners_ids(core: &Core) -> Vec<String> {
    let values = core.config().resolved().values();
    values
        .keys()
        .filter_map(|key| {
            let segments = miyu_config::key::split(key)?;
            let miyu_config::key::Fit::Yes(names) = miyu_config::key::fit(BINDINGS, &segments)
            else {
                return None;
            };
            match values.get(key) {
                Some(miyu_config::Value::Text(value)) if !value.is_empty() => {
                    Some(names.first()?.clone())
                }
                _ => None,
            }
        })
        .collect()
}

/// `venue.binding` 的参数（施工 O-31 前）：一次问一个平台身份。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BindingParams {
    id: String,
}

/// `venue.binding`（施工 O-31 前，`venues.md`「问对应表」）：平台身份 `id` 在主人对应表里是不是主人本人，回 `{"owner": true|false}`
/// （施工 SU-4 中）。只给平台桥：提供者照它挡在场所里做的事（`providers.md`「在场所里做的事」），别的连接照没批
/// `act_for_external` 拒。只读。
pub(crate) fn binding(
    core: &Core,
    caller: &Caller,
    params: BindingParams,
) -> Result<Value, Refusal> {
    bridge(core, caller).ok_or_else(bridge::refused)?;
    let id = ExternalId::parse(&params.id).map_err(|_| Refusal::BAD_PARAMS)?;
    Ok(json!({"owner": bound(core, &id)}))
}

/// `venue.session`：找回或者造场所 `venue` 的主线会话（`venues.md`「对外的样子」，施工 SU-4 中）：
/// 私聊、对方在对应表里的是主人本人，不关，主人这边的连接也开得了；别的（群、陌生人的私聊）关起来、只有平台桥开，记进
/// `session.created` 的 `confined`。照「场所加关不关」
/// 找：主会话里场所是它的、关不关一样的最新的那一个（对应表后来加了主人的号，原来关着的那个不接着用，另开一个）。
pub(crate) async fn session(
    core: &Arc<Core>,
    caller: &Caller,
    command: CommandId,
    params: VenueParams,
) -> Result<Value, Refusal> {
    let venue = VenueId::parse(&params.venue).map_err(|_| Refusal::BAD_PARAMS)?;
    let peer = match (params.kind, params.peer) {
        (Kind::Private, Some(peer)) => {
            Some(ExternalId::parse(&peer).map_err(|_| Refusal::BAD_PARAMS)?)
        }
        (Kind::Group, None) => None,
        _ => return Err(Refusal::BAD_PARAMS),
    };
    let recent = match (params.kind, params.recent) {
        (_, None) => None,
        (Kind::Group, Some(recent @ 1..=RECENT_MOST)) => Some(recent),
        _ => return Err(Refusal::bad_field("recent")),
    };
    let confined = !peer.as_ref().is_some_and(|peer| bound(core, peer));
    // 主人本人的私聊（对应表认出来的）主人这边的连接也开得了（同原来）；群、陌生人的私聊只有平台桥开。
    if confined {
        bridge(core, caller).ok_or_else(bridge::refused)?;
    }
    // 同一个场所同时来两次：一个找、一个造，排着来，免得造出两个主线会话。
    let _one_at_a_time = core.venues.lock().await;
    if let Some(found) = find(core, &venue, confined).await? {
        return Ok(json!({"session": found.as_str(), "created": false}));
    }
    let who = Opening {
        attended: false,
        oneshot: false,
        model: None,
        venue: Some(venue),
        memory: None,
        preset: params.preset,
        group: (params.kind == Kind::Group).then_some(miyu_session::GroupSettings { recent }),
        profile: params.profile,
        images: params.images,
        titles: params.titles,
        origin: None,
        confined,
    };
    let cwd = params
        .cwd
        .unwrap_or_else(|| crate::list::NO_CWD.to_string());
    let created = core
        .sessions
        .create(
            core,
            command,
            params.persona.as_ref().map(Option::as_deref),
            cwd,
            Vec::new(),
            who,
        )
        .await?;
    Ok(json!({"session": created.id.as_str(), "created": true}))
}

/// 主人的主会话里，场所是 `venue`、关不关是 `confined` 的最新的那一个。
async fn find(
    core: &Arc<Core>,
    venue: &VenueId,
    confined: bool,
) -> Result<Option<miyu_kernel::id::SessionId>, Refusal> {
    let venue = venue.as_str().to_string();
    for listed in mains(core, move |row| row.venue == venue, None).await? {
        if crate::sessions::confined_of(core, &listed.id).await == confined {
            return Ok(Some(listed.id));
        }
    }
    Ok(None)
}

/// 主会话（不是子会话）里合 `pick` 的，从新到旧，最多 `limit` 个：照会话列表的索引找，和 `session.list` 同一个
/// 读法。
async fn mains(
    core: &Arc<Core>,
    pick: impl Fn(&Row) -> bool + Send + 'static,
    limit: Option<usize>,
) -> Result<Vec<Listed>, Refusal> {
    let root = core.root.clone();
    let index = core.index();
    let found = tokio::task::spawn_blocking(move || {
        let pick = |row: &Row| row.parent.is_none() && pick(row);
        scan(
            &root,
            Some(&*index),
            &Default::default(),
            pick,
            limit,
            &Stop::default(),
        )
    })
    .await;
    match found {
        Ok(Ok(listed)) => Ok(listed),
        Ok(Err(error)) => {
            tracing::warn!(target: "miyu::endpoint", error = %error, "venue session not found");
            Err(Refusal::INTERNAL)
        }
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// `session.send` 的 `as` 记成谁（`venues.md`「`session.send` 的 `as`」）：场所会话 `handle`。没关起来的（主人本人的私聊）里对应表
/// 认出的号是本人（带 `via`）；关起来的（群、陌生人的私聊）里照旧是外部身份，对应表里有的记上
/// `owner`：主人在群里说话不当本人，不给主人才有的那些。
pub(crate) fn speaker(
    core: &Core,
    handle: &miyu_session::Handle,
    params: AsParams,
) -> Result<By, Refusal> {
    let venue = handle.venue();
    let id = ExternalId::parse(&params.external).map_err(|_| Refusal::BAD_PARAMS)?;
    let owner = bound(core, &id);
    if !handle.confined() && owner {
        return Ok(By::Person(Person { via: Some(id) }));
    }
    Ok(By::External(External {
        venue: venue.clone(),
        id,
        owner,
        role: Some(params.role.unwrap_or(Role::Member)),
    }))
}
