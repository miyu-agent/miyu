//! `session.tools`（施工 S-T，终端 `/tool-list` 要的）：她下一次请求会拿到的工具，照发的先后（名字的字节序），每件带来处。
//!
//! - 写了 `session` 的问那个会话（`Handle::tools`）：照回合开始换快照那一套看一遍，人格、预设、目录换了的照换了以后的算。
//! - 没写的照新会话会拿到的算（`miyu_session::preview`，和 `session.create` 照同一份拼，`sessions/setup.rs`）：`persona`、
//!   `preset` 同 `session.create`，写了 `session` 的不收这两格。
//! - 来处：MCP 的照服务名，脚本照脚本的显示名、一句说明（照连接的语言挑），别的照工具目录记的包，包名照清单；内置的包
//!   `kind` 是 `builtin`。技能不是工具，不会出现。

use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::PackageKind;
use miyu_kernel::id::SessionId;
use miyu_policy::Offered;
use miyu_session::new_id;

use crate::Core;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;
use crate::sessions::{Opening, Setup, now, opening_cwd, roots};

/// `session.tools` 的参数。
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Params {
    #[serde(default)]
    session: Option<String>,
    /// 同 `session.create`：不写的照 `persona.default`，写 `null` 的无人格。
    #[serde(default, deserialize_with = "crate::personas::written")]
    persona: Option<Option<String>>,
    /// 同 `session.create`：不写的照 `preset.default`，写 `null` 的是无工具（施工 P-16 下；CX-2 补：原来当成没写）。
    #[serde(default, deserialize_with = "crate::personas::written")]
    preset: Option<Option<String>>,
}

/// `session.tools`：`{"tools": [...]}`。
pub(crate) async fn tools(
    core: &Arc<Core>,
    peer: Peer,
    account: &miyu_kernel::id::AccountId,
    params: Params,
) -> Result<Value, Refusal> {
    let offered = match params.session {
        Some(_) if params.persona.is_some() || params.preset.is_some() => {
            return Err(Refusal::BAD_PARAMS);
        }
        Some(text) => {
            let id = SessionId::parse(&text).map_err(|_| Refusal::BAD_PARAMS)?;
            let found = core.sessions.get(core, &id).await?;
            found.handle.tools().await.map_err(|_| Refusal::INTERNAL)?
        }
        None => {
            let preset = crate::presets::opening(params.preset)?;
            preview(core, peer, account, params.persona, preset).await?
        }
    };
    let language = peer.language.to_string();
    let tools: Vec<Value> = offered
        .iter()
        .map(|offered| entry(core, &language, offered))
        .collect();
    Ok(json!({ "tools": tools }))
}

/// 新会话会拿到的：照 `session.create` 那一份拼，不造。
async fn preview(
    core: &Arc<Core>,
    peer: Peer,
    account: &miyu_kernel::id::AccountId,
    persona: Option<Option<String>>,
    preset: Option<String>,
) -> Result<Vec<Offered>, Refusal> {
    let mut setup = new_session(core, peer, account, persona, preset).await?;
    Ok(miyu_session::preview(setup.create(core)).await)
}

/// 照 `session.create` 那一份备一个新会话（`session.tools`、`session.context` 不写会话的），只备不造。
pub(crate) async fn new_session(
    core: &Arc<Core>,
    peer: Peer,
    account: &miyu_kernel::id::AccountId,
    persona: Option<Option<String>>,
    preset: Option<String>,
) -> Result<Setup, Refusal> {
    let who = Opening {
        owner: account.clone(),
        attended: peer.input,
        oneshot: false,
        model: None,
        venue: None,
        memory: None,
        preset,
        group: None,
        profile: false,
        images: true,
        titles: true,
        origin: None,
    };
    let id = new_id(now());
    let (_, own) = roots(core, account);
    let (workspace, _) = opening_cwd(core, &who, &own.to_string_lossy(), &id);
    let command = miyu_kernel::id::CommandId::parse("session-tools-preview")
        .map_err(|_| Refusal::INTERNAL)?;
    Setup::resolve(
        core,
        (id, command),
        persona.as_ref().map(Option::as_deref),
        (workspace, Vec::new()),
        who,
    )
    .await
}

/// 一件：名字、来处、说明（有的才写）、只露名字的 `loaded: false`。
fn entry(core: &Core, language: &str, offered: &Offered) -> Value {
    let (source, summary) = source(core, language, &offered.name);
    let mut entry = json!({ "name": offered.name, "source": source });
    if let Some(summary) = summary {
        entry["summary"] = json!(summary);
    }
    if !offered.loaded {
        entry["loaded"] = json!(false);
    }
    entry
}

/// 来处，和给人看的一句说明。
pub(crate) fn source(core: &Core, language: &str, name: &str) -> (Value, Option<String>) {
    if let Some(service) = miyu_tool::mcp_service(name) {
        return (json!({"kind": "mcp", "id": service, "name": service}), None);
    }
    let scripts = core.scripts();
    if let Some(script) = scripts.iter().find(|script| script.head.name == name) {
        let shown = pick(&script.head.display_name, language).unwrap_or_else(|| name.to_string());
        let summary = pick(&script.head.summary, language);
        return (
            json!({"kind": "script", "id": name, "name": shown}),
            summary,
        );
    }
    let package = core
        .tools()
        .package_of(name)
        .map_or_else(|| miyu_tool::BASESYSTEM.to_string(), str::to_string);
    let (kind, shown) = package_of(core, language, &package);
    (json!({"kind": kind, "id": package, "name": shown}), None)
}

/// 包是内置的（`builtin`）还是装的（`package`），和照连接的语言的名字；清单读不出的算内置、名字照编号。
pub(crate) fn package_of(core: &Core, language: &str, package: &str) -> (&'static str, String) {
    let packages = core.system_packages();
    let manifest = packages
        .iter()
        .find(|found| found.id == package)
        .and_then(|found| found.read.as_ref().ok());
    let kind = match manifest.map(|manifest| manifest.kind) {
        Some(PackageKind::Builtin) | None => "builtin",
        Some(_) => "package",
    };
    let shown = manifest
        .and_then(|manifest| pick(&manifest.name, language))
        .unwrap_or_else(|| package.to_string());
    (kind, shown)
}
