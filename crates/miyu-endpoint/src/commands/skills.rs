//! 人用 `$` 点名技能（施工 SN-1，发版前检查第 26 条，项目主人 2026-11-09 定：技能不在 `/` 命令里，在输入框里用 `$` 点名，一句话
//! 里能点几个；原来的 `/技能名` 是施工 PT-2 下做的，去掉）：`session.send` 的 `skills` 照先后找能点名用的技能（找法同造会话，
//! `miyu_store::skills::find`），读那份 `SKILL.md`，去掉开头，套一层 `<skill name=… path=…>` 外壳，一个技能一块，排在人说的那句
//! 前面；时间线上是人敲的那一行（同模板命令）。外壳的字在资源目录的 `core/skills/invoked-*.txt`，名字、路径照模板转义；正文原样，
//! 同她自己读那份文件。
//!
//! 谁能用：`user-invocable: false` 的认不出；会话的预设开着这个技能（功能 `skill-<名字>`）才行，同列进 system 的那一份。只许人
//! 点名用的（`disable-model-invocation`）照常能点名。`command.catalog` 的 `skills` 列出能点名的，头照它做 `$` 菜单。
//!
//! 碰磁盘的在阻塞线程里做。

use std::collections::BTreeMap;

use serde_json::{Value, json};

use miyu_kernel::block::{Block, Text};
use miyu_kernel::event::TypedCommand;
use miyu_kernel::template::Template;
use miyu_policy::skill::{body, feature_id};
use miyu_session::Handle;
use miyu_store::skills::Skill;

use super::{Said, templates};
use crate::Core;
use crate::hello::Peer;
use crate::refusal::Refusal;

/// 能点名用的技能（`user-invocable` 不是 `false` 的），照名字排。
pub(super) async fn found(core: &Core) -> Vec<Skill> {
    let (packages, root) = (core.packages_of(), core.root.clone());
    let external = crate::skills::external(core);
    tokio::task::spawn_blocking(move || miyu_store::skills::find(&packages, &root, &external).0)
        .await
        .unwrap_or_default()
        .into_iter()
        .filter(|skill| skill.head.user_invocable)
        .collect()
}

/// 名字是 `name`、能点名用的那一个技能；没有的没有。
pub(super) async fn named(core: &Core, name: &str) -> Option<Skill> {
    found(core)
        .await
        .into_iter()
        .find(|skill| skill.head.name == name)
}

/// 技能挂在哪个包下（预设照它开关，同造会话的 `miyu_session::skills::opened`）：包带的照包的编号，别的是空的。
pub(super) fn package(skill: &Skill) -> &str {
    skill.package.as_deref().unwrap_or_default()
}

/// 会话的预设开没开技能 `skill`（功能 `skill-<名字>`）：关着的拒 `command_not_allowed`，说「预设「…」未启用技能 $名字。」。
pub(super) async fn opened(
    core: &Core,
    peer: &Peer,
    handle: &Handle,
    skill: &Skill,
) -> Result<(), Refusal> {
    let name = skill.head.name.as_str();
    let closed = (feature_id(name), package(skill));
    let Some(label) = templates::closed_by(core, peer, handle, (&closed.0, closed.1)).await else {
        return Ok(());
    };
    let said = Said {
        key: "commands/unavailable/skill-preset-off",
        fields: vec![("preset", label), ("skill", name.to_string())],
        lists: Vec::new(),
    };
    Err(Refusal::COMMAND_NOT_ALLOWED.because(super::words(core, peer, &said).await))
}

/// 人用 `$` 点名的技能 `names`，`typed` 是人敲的原文：一个技能一块套好外壳的正文，和记在日志里的那一行（`name` 是 `$名字` 用空格
/// 接起来，`text` 是人敲的原文，时间线照它）。没点名的没有。写了两遍的 `bad_params`；没有、不许人点名的 `unknown_skill`；预设
/// 关着的 `command_not_allowed`，说清哪个预设；读不出那份文件的照内部出错。
pub(crate) async fn mentioned(
    core: &Core,
    peer: &Peer,
    handle: &Handle,
    (names, typed): (&[String], &str),
) -> Result<Option<(Vec<Block>, TypedCommand)>, Refusal> {
    if names.is_empty() {
        return Ok(None);
    }
    let mut blocks = Vec::with_capacity(names.len());
    for (index, name) in names.iter().enumerate() {
        if names[..index].contains(name) {
            return Err(Refusal::BAD_PARAMS);
        }
        let skill = named(core, name)
            .await
            .ok_or_else(|| Refusal::unknown_skill(name))?;
        opened(core, peer, handle, &skill).await?;
        let file = skill.file.clone();
        let text = tokio::task::spawn_blocking(move || std::fs::read_to_string(file))
            .await
            .map_err(|_| Refusal::INTERNAL)?
            .map_err(|_| Refusal::INTERNAL)?;
        let text = wrapped(core, &skill, body(&text))?;
        blocks.push(Block::Text(Text { text }));
    }
    let name = names
        .iter()
        .map(|name| format!("${name}"))
        .collect::<Vec<_>>()
        .join(" ");
    let text = typed.trim().to_string();
    Ok(Some((blocks, TypedCommand { name, text })))
}

/// 套好外壳的一段：外壳头（名字、路径）、正文、外壳尾。外壳的字读不出、写法不对照内部出错。
fn wrapped(core: &Core, skill: &Skill, body: &str) -> Result<String, Refusal> {
    let texts = core
        .resources
        .invoked_texts()
        .map_err(|_| Refusal::INTERNAL)?;
    let path = miyu_session::path_text(&skill.file);
    let fields = BTreeMap::from([("name", skill.head.name.as_str()), ("path", path.as_str())]);
    let mut text = Template::parse(&texts.open)
        .ok()
        .and_then(|template| template.render(&fields).ok())
        .ok_or(Refusal::INTERNAL)?;
    text.push_str(body);
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&texts.close);
    Ok(text.trim_end().to_string())
}

/// `command.catalog` 的 `skills` 里的一项（施工 SN-1）：名字、给人看的一句（照连接的语言挑，没写的照给模型的说明）。
pub(super) fn listed(skill: &Skill, language: &str) -> Value {
    let summary = crate::personas::pick(&skill.head.summary, language)
        .unwrap_or_else(|| skill.head.description.clone());
    json!({"name": skill.head.name, "summary": summary})
}
