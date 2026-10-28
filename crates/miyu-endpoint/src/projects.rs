//! 项目（施工 H-1，设计 `38-项目.md`）：`project.list`、`project.create`、`project.update`、`project.delete`，存在这个账号家目录的
//! `projects.json`（`miyu_store::projects`）。改的几个排着做（[`Core`] 的 `projects` 锁）：读、改、照读到的版本写，写成了报一声
//! 「这个账号的这个项目变了」（[`changes`]），订阅 `projects` 的连接照它推（`subscriptions/listed.rs`）。只给人（`connection/rights.rs`
//! 的表，施工 X-30）。

pub(crate) mod changes;
pub(crate) mod sessions;

use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_store::projects::{self, Project, Projects};

use crate::Core;
use crate::methods::params;
use crate::refusal::Refusal;
use crate::wire::Request;

/// 运行日志的目标。
const TARGET: &str = "miyu::projects";

/// 能清掉的格（`unset`）。
const CLEARABLE: [&str; 6] = [
    "description",
    "color",
    "workspace",
    "persona",
    "preset",
    "prompt",
];

/// 答一条 `project.*`；不认识的 `unknown_method`。
pub(crate) async fn call(core: &Arc<Core>, request: &Request) -> Result<Value, Refusal> {
    let account = &core.admin;
    match request.method.as_str() {
        "project.list" => {
            let read = read(core, account).await?;
            Ok(json!({"projects": read.projects.projects}))
        }
        "project.create" => create(core, account, params(request)?).await,
        "project.update" => update(core, account, params(request)?).await,
        "project.delete" => delete(core, account, request, params(request)?).await,
        _ => Err(Refusal::UNKNOWN_METHOD),
    }
}

/// `project.create`、`project.update` 能写的几格；`update` 另有编号和 `unset`。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Fields {
    name: Option<String>,
    description: Option<String>,
    color: Option<String>,
    workspace: Option<String>,
    #[serde(default, deserialize_with = "crate::personas::written")]
    persona: Option<Option<String>>,
    preset: Option<String>,
    prompt: Option<String>,
    open: Option<bool>,
}

/// `project.update` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UpdateParams {
    project: String,
    #[serde(default)]
    unset: Vec<String>,
    #[serde(flatten)]
    fields: Fields,
}

/// `project.delete` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct DeleteParams {
    project: String,
}

/// `project.create`：名称必写；查好各格、人格和预设在不在，起编号，加在最后。
async fn create(core: &Arc<Core>, account: &AccountId, fields: Fields) -> Result<Value, Refusal> {
    let Some(name) = fields.name.clone() else {
        return Err(Refusal::bad_field("name"));
    };
    let now = crate::sessions::now();
    let mut project = Project {
        project: miyu_session::new_id(now).as_str().to_string(),
        name,
        description: None,
        color: None,
        workspace: None,
        persona: None,
        preset: None,
        prompt: None,
        open: false,
        created: now,
    };
    apply(&mut project, fields);
    known(core, account, &project).await?;
    let id = project.project.clone();
    change(core, account, &id, move |projects| {
        projects.projects.push(project);
        Ok(())
    })
    .await?;
    tracing::info!(target: TARGET, project = id.as_str(), "project created");
    Ok(json!({"project": id}))
}

/// `project.update`：写了值的换上，`unset` 列的清掉；一格又写又清、清名称、清不认识的格都是参数不对。
async fn update(
    core: &Arc<Core>,
    account: &AccountId,
    params: UpdateParams,
) -> Result<Value, Refusal> {
    for field in &params.unset {
        if !CLEARABLE.contains(&field.as_str()) || written(&params.fields, field) {
            return Err(Refusal::bad_field(field));
        }
    }
    let current = read(core, account).await?;
    let mut project = current
        .projects
        .find(&params.project)
        .cloned()
        .ok_or(Refusal::UNKNOWN_PROJECT)?;
    apply(&mut project, params.fields);
    for field in &params.unset {
        clear(&mut project, field);
    }
    known(core, account, &project).await?;
    let id = params.project;
    change(core, account, &id, move |projects| {
        let slot = projects
            .find_mut(&project.project)
            .ok_or(Refusal::UNKNOWN_PROJECT)?;
        *slot = project;
        Ok(())
    })
    .await?;
    Ok(json!({}))
}

/// `project.delete`：先把里面的会话挪出来（施工 H-2，[`sessions::move_out`]），再从文件里拿掉；回挪了几个。
async fn delete(
    core: &Arc<Core>,
    account: &AccountId,
    request: &Request,
    params: DeleteParams,
) -> Result<Value, Refusal> {
    let id = params.project;
    sessions::find(core, account, &id).await?;
    let moved = sessions::move_out(core, account, request, &id).await?;
    change(core, account, &id, |projects| {
        projects
            .remove(&id)
            .map(drop)
            .ok_or(Refusal::UNKNOWN_PROJECT)
    })
    .await?;
    tracing::info!(target: TARGET, project = id.as_str(), moved, "project deleted");
    Ok(json!({"moved": moved}))
}

/// 写了值的格换上：空字符串当清掉（名称除外，空的照查出错）。
fn apply(project: &mut Project, fields: Fields) {
    let given = |text: Option<String>| text.map(|text| Some(text).filter(|text| !text.is_empty()));
    if let Some(name) = fields.name {
        project.name = name;
    }
    if let Some(value) = given(fields.description) {
        project.description = value;
    }
    if let Some(value) = given(fields.color) {
        project.color = value;
    }
    if let Some(value) = given(fields.workspace) {
        project.workspace = value;
    }
    if let Some(persona) = fields.persona {
        project.persona = Some(persona);
    }
    if let Some(value) = given(fields.preset) {
        project.preset = value;
    }
    if let Some(value) = given(fields.prompt) {
        project.prompt = value;
    }
    if let Some(open) = fields.open {
        project.open = open;
    }
}

/// `fields` 里写了 `field` 这一格。
fn written(fields: &Fields, field: &str) -> bool {
    match field {
        "description" => fields.description.is_some(),
        "color" => fields.color.is_some(),
        "workspace" => fields.workspace.is_some(),
        "persona" => fields.persona.is_some(),
        "preset" => fields.preset.is_some(),
        "prompt" => fields.prompt.is_some(),
        _ => false,
    }
}

/// 清掉一格（[`CLEARABLE`] 里的）。
fn clear(project: &mut Project, field: &str) {
    match field {
        "description" => project.description = None,
        "color" => project.color = None,
        "workspace" => project.workspace = None,
        "persona" => project.persona = None,
        "preset" => project.preset = None,
        "prompt" => project.prompt = None,
        _ => {}
    }
}

/// 各格的写法对、写了的人格和预设在：人格、预设照造会话那一套找（设计第二节第 5 条：造项目时就查）。
async fn known(core: &Core, account: &AccountId, project: &Project) -> Result<(), Refusal> {
    project
        .check()
        .map_err(|wrong| Refusal::bad_field(wrong.field()))?;
    if let Some(Some(persona)) = &project.persona {
        crate::personas::resolve(core, account, Some(Some(persona.as_str()))).await?;
    }
    if let Some(preset) = &project.preset {
        crate::presets::resolve(core, account, Some(preset.as_str())).await?;
    }
    Ok(())
}

/// 这个账号的项目文件。
fn file(core: &Core, account: &AccountId) -> PathBuf {
    core.root.account_dir(account).join(projects::FILE)
}

/// 读这个账号的项目：读不成的记一行、内部出错（不替人改坏了的文件）。
async fn read(core: &Core, account: &AccountId) -> Result<projects::Read, Refusal> {
    let path = file(core, account);
    let read = tokio::task::spawn_blocking(move || projects::read(&path)).await;
    match read {
        Ok(Ok(read)) => Ok(read),
        Ok(Err(error)) => {
            tracing::warn!(target: TARGET, error = %error, "projects unreadable");
            Err(Refusal::INTERNAL)
        }
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// 排着改一次：读、照 `edit` 改、照读到的版本写回，写成了报项目 `id` 变了。`edit` 拒绝的什么都不写。
async fn change(
    core: &Core,
    account: &AccountId,
    id: &str,
    edit: impl FnOnce(&mut Projects) -> Result<(), Refusal> + Send,
) -> Result<(), Refusal> {
    let _turn = core.projects.lock().await;
    let mut current = read(core, account).await?;
    edit(&mut current.projects)?;
    let path = file(core, account);
    let written = tokio::task::spawn_blocking(move || {
        projects::write(&path, &current.projects, current.version.as_deref())
    })
    .await;
    match written {
        Ok(Ok(())) => {
            core.project_changes.notify(account, id);
            Ok(())
        }
        Ok(Err(error)) => {
            tracing::warn!(target: TARGET, error = %error, "projects not written");
            Err(Refusal::INTERNAL)
        }
        Err(_) => Err(Refusal::INTERNAL),
    }
}

/// 订阅 `projects` 的回应：这个账号这一刻的整张表。
pub(crate) async fn listed(core: &Core, account: &AccountId) -> Result<Value, Refusal> {
    let read = read(core, account).await?;
    Ok(json!({"projects": read.projects.projects}))
}

/// 推送里的一项：这个账号的项目 `id` 这一刻的样子；没有了的是删了。读不成的没有（不推）。
pub(crate) fn item(core: &Core, account: &AccountId, id: &str) -> Option<Value> {
    let read = projects::read(&file(core, account)).ok()?;
    Some(match read.projects.find(id) {
        Some(project) => json!({"project": project}),
        None => json!({"project": id, "deleted": true}),
    })
}
