//! 造会话时的工具面（施工 S-T 从 `create` 挪出来）：造会话和 `session.tools` 预览新会话（[`preview`]）走同一条路，预览的
//! 就是造出来会拿到的。读知识库、找人格这些只读，不写盘。

use std::sync::Arc;

use miyu_config::Values;
use miyu_policy::memory::MemoryScope;
use miyu_policy::preset::{Chosen, MEMORY};
use miyu_policy::{Offered, ToolEntry};
use miyu_tool::Edition;

use super::Create;
use super::setup::blinded;
use crate::agents::{Agents, Holds, Offers, Reach, Site, persona_choices};
use crate::blocking::blocking;
use crate::config::Turning;
use crate::knowledge::{Listed, Shelves, Shelving};
use crate::memory;

/// 照造会话的几样拼出来的。
pub(super) struct Opened {
    /// 照场所能不能显示图扣过的预设。
    pub(super) preset: Option<Chosen>,
    /// 记忆的范围。
    pub(super) scope: MemoryScope,
    /// 知识库：架子、查得到的那几个、够不够得着。
    pub(super) shelves: Option<Arc<Shelves>>,
    pub(super) listed: Vec<Listed>,
    /// 工具面，和照的那一代目录。
    pub(super) face: Vec<ToolEntry>,
    pub(super) edition: Edition,
}

/// 照 `setup` 和这时的配置 `values` 拼工具面。
pub(super) async fn open(setup: &Create<'_>, values: &Values) -> Opened {
    // 装了的功能（施工 F-3 上）：没交预设几层的（测试里造的）当都装着。
    let features = setup.presets.as_ref().map(|places| &places.features);
    let preset = blinded(setup.preset.clone(), setup.images, features);
    let installed = |feature| features.is_none_or(|features| features.installed(feature));
    // 预设没开记忆的，范围一律 `off`（施工 P-2 中，走查 E2：开不开记忆归预设）。没装人格记忆的也是（施工 F-3 上）。
    // 无人格的也有记忆（施工 P-12，2026-10-17 项目主人定，改掉 P-4 上的「无人格记忆不生效」）：跟着人格的落在无人格那一间。
    let opened = installed(MEMORY)
        && preset
            .as_ref()
            .is_none_or(|chosen| chosen.file.opens(MEMORY));
    let scope = memory::scope(setup.lineage.is_some(), opened, setup.memory_scope);
    // 知识库照记忆的账号、人格查（施工 K-1 下）；一个都查不到的工具面上没有两件，回合开始再判。
    let shelves = setup.knowledge.clone().map(|knowledge| {
        let shelving = Shelving {
            knowledge,
            persona: setup.persona.map(str::to_string),
            personas: setup.personas.clone(),
            owner: setup.owner.clone(),
            home: setup.home.map(std::path::Path::to_path_buf),
            vectors: setup.vectors.clone(),
            recall: crate::knowledge::recall::texts(setup.resources),
        };
        Arc::new(Shelves::new(shelving, setup.memory_account.clone()))
    });
    let (listed, reach) = match shelves.clone() {
        Some(shelves) => {
            blocking(move || {
                let listed = shelves.listed();
                let reach = shelves.reach(&listed);
                (listed, reach)
            })
            .await
        }
        None => (Vec::new(), Reach::Save),
    };
    // 工具面照这时的配置拼：`subagent` 能选哪几个池（施工 8-8 补）、哪几个人格（施工 P-2 补），以后照快照、载入不重拼。
    // 照预设筛（施工 P-2 中）。
    let offers = Offers::of(values, persona_choices(&setup.personas));
    // 照架子上现在的那一代拼；记下第几代，回合开头看换没换（施工 O-2 中）。
    let edition = setup.tools.edition();
    let face = Agents::face(
        &edition.catalog,
        Site {
            venue: &setup.venue,
            group: setup.group.is_some(),
        },
        setup.lineage.as_ref(),
        &offers,
        setup.attended,
        Holds {
            memory: scope,
            knowledge: reach,
        },
        preset.as_ref().map(|chosen| (&chosen.file, features)),
    );
    Opened {
        preset,
        scope,
        shelves,
        listed,
        face,
        edition,
    }
}

/// 照 `setup` 造出来的会话第一次请求会发的工具（施工 S-T，`session.tools` 不写会话的）：只拼不造，什么都不写。
pub async fn preview(setup: Create<'_>) -> Vec<Offered> {
    let config = Turning::start(setup.configs.clone(), setup.environment.cwd.clone()).await;
    let values = config.current().resolved.values();
    let opened = open(&setup, &values).await;
    let texts = setup.resources.loading().ok();
    miyu_policy::offered(&opened.face, texts.as_ref())
}
