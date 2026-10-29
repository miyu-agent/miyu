//! 人点名用技能（施工 PT-2 下，`docs/blueprint/skills.md`「人点名用」）：`/技能名 参数`，登记表、模板、扩展声明的都没有这个
//! 名字时，照会话属主找得到的技能认（找法同造会话，`miyu_store::skills::find`）。读那份 `SKILL.md`，去掉开头，套一层
//! `<skill name=… path=…>` 外壳，人写了参数的再跟一行 `ARGUMENTS: …`，当这个人说的一句发出去（同模板命令，时间线上是人敲的
//! 那一行）。外壳的字在资源目录的 `core/skills/invoked-*.txt`，名字、路径、参数照模板转义；正文原样，同她自己读那份文件。
//!
//! 谁能用：`user-invocable: false` 的认不出；会话的预设开着这个技能（功能 `skill-<名字>`）才行，同列进 system 的那一份；
//! 停用、身份照模板命令（没有出厂值）。只许人点名用的（`disable-model-invocation`）照常能点名。
//!
//! 碰磁盘的在阻塞线程里做。

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::TypedCommand;
use miyu_kernel::id::{AccountId, CommandId, SessionId};
use miyu_kernel::origin::By;
use miyu_kernel::session::Command;
use miyu_kernel::template::Template;
use miyu_policy::skill::{body, feature_id};
use miyu_session::Handle;
use miyu_store::skills::Skill;

use super::{RunParams, accepted, command, templates};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 账号 `owner` 能点名用的技能（`user-invocable` 不是 `false` 的），照名字排。
pub(super) async fn found(core: &Core, owner: &AccountId) -> Vec<Skill> {
    let (packages, root) = (core.packages_of(owner), core.root.clone());
    let external = crate::skills::external(core);
    tokio::task::spawn_blocking(move || miyu_store::skills::find(&packages, &root, &external).0)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|skill| skill.head.user_invocable)
        .collect()
}

/// 名字是 `name`、能点名用的那一个技能；没有的没有。
pub(super) async fn named(core: &Core, owner: &AccountId, name: &str) -> Option<Skill> {
    found(core, owner)
        .await
        .into_iter()
        .find(|skill| skill.head.name == name)
}

/// 技能挂在哪个包下（预设照它开关，同造会话的 `miyu_session::skills::opened`）：包带的照包的编号，别的是空的。
pub(super) fn package(skill: &Skill) -> &str {
    skill.package.as_deref().unwrap_or_default()
}

/// 会话的预设开没开技能 `skill`（功能 `skill-<名字>`）：同 [`templates::opened_by`]。
pub(super) async fn opened(
    core: &Core,
    peer: &Peer,
    handle: &Handle,
    skill: &Skill,
) -> Result<(), Refusal> {
    let name = skill.head.name.as_str();
    templates::opened_by(
        core,
        peer,
        handle,
        (name, &feature_id(name), package(skill)),
    )
    .await
}

/// 点名用技能 `skill`，后面跟 `rest`：会话、说话的人照模板命令认好了（`templates::run`）。预设关着的拒；读不出那份文件的
/// 照内部出错。发出去，交回 `{"command", "events", "said": ""}`。
pub(super) async fn run(
    core: &Arc<Core>,
    peer: &Peer,
    id: &CommandId,
    params: &RunParams,
    (session, handle, by): (&SessionId, &Handle, &By),
    (skill, rest): (&Skill, &str),
) -> Result<Value, Refusal> {
    opened(core, peer, handle, skill).await?;
    let file = skill.file.clone();
    let text = tokio::task::spawn_blocking(move || std::fs::read_to_string(file))
        .await
        .map_err(|_| Refusal::INTERNAL)?
        .map_err(|_| Refusal::INTERNAL)?;
    let wrapped = wrapped(core, skill, body(&text), rest.trim())?;
    let typed = TypedCommand {
        name: skill.head.name.clone(),
        text: params.text.trim().to_string(),
    };
    let sent = Command::Typed {
        blocks: vec![Block::Text(Text { text: wrapped })],
        command: typed,
    };
    // 文件改了、前缀要变的，开这一轮之前问人（施工 OV-1 下）。
    crate::sessions::prefix::ask(core, (*peer, by), (handle, id), params.prefix).await?;
    let events = accepted(command(core, session, handle, id, by, sent).await?)?;
    Ok(json!({"command": skill.head.name, "events": events, "said": ""}))
}

/// 套好外壳的一段：外壳头（名字、路径）、正文、外壳尾，写了参数的再跟一行。外壳的字读不出、写法不对照内部出错。
fn wrapped(core: &Core, skill: &Skill, body: &str, arguments: &str) -> Result<String, Refusal> {
    let texts = core
        .resources
        .invoked_texts()
        .map_err(|_| Refusal::INTERNAL)?;
    let say = |source: &str, fields: &[(&str, &str)]| {
        Template::parse(source)
            .ok()
            .and_then(|template| {
                template
                    .render(&fields.iter().copied().collect::<BTreeMap<_, _>>())
                    .ok()
            })
            .ok_or(Refusal::INTERNAL)
    };
    let path = miyu_session::path_text(&skill.file);
    let mut text = say(&texts.open, &[("name", &skill.head.name), ("path", &path)])?;
    text.push_str(body);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&texts.close);
    if !arguments.is_empty() {
        text.push_str(&say(&texts.arguments, &[("arguments", arguments)])?);
    }
    Ok(text.trim_end().to_string())
}

/// `command.catalog` 里的一项：名字、没有别名、给人看的一句（照连接的语言挑，没写的照给模型的说明），标着是技能。
pub(super) fn listed(skill: &Skill, language: &str) -> Value {
    let summary = crate::personas::pick(&skill.head.summary, language)
        .unwrap_or_else(|| skill.head.description.clone());
    json!({"name": skill.head.name, "aliases": [], "summary": summary, "skill": true})
}
