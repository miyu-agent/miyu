//! 造会话交给 `miyu_session` 的那一份（施工 S-T 从 `Sessions::create` 挪出来）：造会话和 `session.tools` 预览新会话照同一份
//! 拼，预览的就是造出来会拿到的。先找好预设、人格（[`Setup::resolve`]），再借着它拼 [`Create`]（[`Setup::create`]）。

use std::sync::Arc;

use miyu_kernel::event::Permission;
use miyu_kernel::id::{CommandId, SessionId};
use miyu_policy::Profile;
use miyu_policy::memory::MemoryScope;
use miyu_session::Create;
use miyu_session::settings::ProfileSettings;

use super::opening::{Opening, opening_level, profile_for};
use super::{environment, local};
use crate::Core;
use crate::personas;
use crate::presets;
use crate::refusal::Refusal;
use crate::settings::PermissionSettings;
use crate::spawn;

/// 找好了的几样。
pub(crate) struct Setup {
    pub(crate) id: SessionId,
    command: CommandId,
    workspace: String,
    dirs: Vec<String>,
    who: Opening,
    permission: Permission,
    profile: Profile,
    preset: Option<miyu_store::presets::Found>,
    persona: Option<miyu_store::personas::Found>,
}

impl Setup {
    /// 找预设（`who.preset`，没写的照默认预设，施工 P-2 上），再找人格（`persona` 照 [`personas::resolve`]），开局的级别、
    /// 「关于你」照会话实际干活的目录 `workspace` 的配置（带上信任着的项目配置）。
    pub(crate) async fn resolve(
        core: &Core,
        (id, command): (SessionId, CommandId),
        persona: Option<Option<&str>>,
        (workspace, dirs): (String, Vec<String>),
        who: Opening,
    ) -> Result<Setup, Refusal> {
        // 照属主那一份（施工 Y-2 下下中上）。
        let (resolved, _) = core.mine(&who.owner).config().with_project(&workspace);
        let permission = PermissionSettings::from(&resolved.values());
        let level = opening_level(&permission, who.venue.as_ref());
        let profile = profile_for(
            ProfileSettings::from(&resolved.values()),
            who.venue.as_ref(),
            who.profile,
            who.owner == core.admin,
        );
        // 空的编号是明着无工具（施工 P-16 下，`presets::opening`）：不找预设。
        let preset = match who.preset.as_deref() {
            Some("") => None,
            // 照新会话的属主找（施工 Y-2 中上）。
            wanted => Some(presets::resolve(core, &who.owner, wanted).await?),
        };
        let persona = personas::resolve(core, &who.owner, persona).await?;
        Ok(Setup {
            id,
            command,
            workspace,
            dirs,
            who,
            permission: Permission {
                level,
                read_only: permission.start_read_only,
            },
            profile,
            preset,
            persona,
        })
    }

    /// 交给 `miyu_session` 的那一份。预设只交一次：再拼一次的没有预设。
    pub(crate) fn create<'a>(&'a mut self, core: &'a Arc<Core>) -> Create<'a> {
        let who = &self.who;
        let persona = self.persona.as_ref();
        Create {
            root: &core.root,
            resources: &core.resources,
            id: self.id.clone(),
            persona: persona.map(|found| found.id.as_str()),
            persona_texts: persona.map(|found| found.texts.clone()).unwrap_or_default(),
            profile: self.profile.clone(),
            personas: personas::personas(core, &who.owner),
            memory_account: core.memory_owner(&who.owner),
            memory_scope: who
                .memory
                .or(persona.and_then(|found| found.file.memory))
                .unwrap_or(MemoryScope::Persona),
            venue: who.venue.clone().unwrap_or_else(local),
            owner: who.owner.clone(),
            permission: self.permission.clone(),
            attended: who.attended,
            oneshot: who.oneshot,
            environment: environment(self.workspace.clone(), self.dirs.clone()),
            command: self.command.clone(),
            by: super::owner_by(core, &who.owner),
            models: &*core.models,
            tools: &core.tools,
            home: core.home.as_deref(),
            sandbox: core.sandbox.helper(),
            sandbox_cache: core.sandbox_cache_of(&who.owner),
            lineage: None,
            sessions: Some(spawn::port(core)),
            packages: Some(crate::packages::port::port(core, &who.owner)),
            hooks: Some(crate::hooks::port(core)),
            jobs: &core.jobs,
            index: Some(core.index_for(&who.owner)),
            usage: core.ledger(),
            knowledge: Some(Arc::clone(&core.knowledge)),
            vectors: core.memory.vectors().cloned(),
            installed: Some(crate::packages::installed(core, &who.owner)),
            plugins: core.plugins.clone(),
            memory: Some(Arc::clone(&core.memory)),
            configs: core.configs_of(&who.owner),
            model: who.model.clone(),
            preset: self
                .preset
                .take()
                .map(|found| presets::chosen(core, &who.owner, found)),
            no_tools: who.preset.as_deref() == Some(""),
            presets: Some(presets::places(core, &who.owner)),
            group: who.group,
            owner_is_admin: who.owner == core.admin,
            images: who.images,
            titles: who.titles,
            dream: None,
            origin: who.origin.clone(),
        }
    }
}
