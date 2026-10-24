//! 造会话时拼快照的料（施工 CX-1 下从 `create` 挪出来）：造会话和 `session.context` 预览新会话（[`preview_context`]）走同一条
//! 路，预览的就是造出来的那一份。找技能、读知识库、读资源这些只读，不写盘。

use std::path::PathBuf;

use miyu_kernel::session::{Input, Session};
use miyu_models::provider::chat;
use miyu_policy::preset::{Chosen, ROLEPLAY};
use miyu_store::blob::Blobs;

use super::face::{self, Opened};
use super::{Create, CreateError};
use crate::agents::Agents;
use crate::blocking::blocking;
use crate::config::Turning;
use crate::knowledge::listing;
use crate::port::ForSession;
use crate::snapshot::{Parts, laid};
use crate::{Context, SystemPart, skills};

/// 拼好的料，和造会话还要的几样：技能的文件夹（放开只读）、名字，回合开始再找技能用的。
pub(super) struct Drafted {
    pub(super) parts: Parts,
    pub(super) shelves: Vec<PathBuf>,
    pub(super) names: skills::Names,
    pub(super) watch: Option<skills::Watch>,
}

/// 照 `setup` 和拼好的工具面 `opened` 备料：工具面挪进料里，知识库查得到的那几个 `listed` 用来列那一块。
///
/// # Errors
///
/// 随核心附带的字读不出来。
pub(super) async fn draft(
    setup: &Create<'_>,
    opened: &Opened,
    (face, listed): (Vec<miyu_policy::ToolEntry>, Vec<crate::knowledge::Listed>),
) -> Result<Drafted, CreateError> {
    // 装了的功能（施工 F-3 上）：没交预设几层的（测试里造的）当都装着。
    let features = setup.presets.as_ref().map(|places| &places.features);
    let roleplay = features.is_none_or(|features| features.installed(ROLEPLAY));
    let preset = opened.preset.as_ref();
    // 技能照这一刻装了的软件包、属主家目录里的找（施工 X-10 上），只要预设开着的（施工 X-11 上）：列进 system，文件夹放开
    // 只读。
    let watch = setup.installed.clone().map(|installed| skills::Watch {
        installed,
        root: setup.root.clone(),
        owner: setup.owner.clone(),
    });
    let (found, shelves, names) = match setup.installed.clone() {
        Some(installed) => {
            let (root, owner) = (setup.root.clone(), setup.owner.clone());
            let file = preset.map(|chosen| chosen.file.clone());
            blocking(move || {
                let found =
                    skills::opened(skills::find(&installed(), &root, &owner), file.as_ref());
                let shelves = skills::shelves(&found);
                let names = skills::names(&found);
                (found, shelves, names)
            })
            .await
        }
        None => (Vec::new(), Vec::new(), skills::Names::new()),
    };
    // 扩展这时说过要放在 system 尾部的（施工 X-20），MCP 服务这时的说明（施工 X-39）。
    let asked = setup
        .hooks
        .as_ref()
        .map(|port| port.appended(setup.id.as_str()))
        .unwrap_or_default();
    let appended = crate::appended::resolve(
        &opened.edition.catalog,
        &face,
        (&Default::default(), &asked),
        true,
    );
    // 工具面上有知识库那两件的，system 里列出她查得到的库（施工 K-1 补）。
    let shelved = face.iter().any(|tool| tool.name == miyu_tool::KB_SEARCH);
    let resources = setup.resources.clone();
    let profile = setup.profile.clone();
    let parts = Parts {
        name: setup.persona.map(str::to_string),
        texts: setup.persona_texts.clone(),
        attended: setup.attended,
        face,
        memory: Some(opened.scope.as_str().to_string()),
        child: setup.lineage.is_some(),
        preset: preset.map(Chosen::pin),
        roleplay,
        group: setup
            .group
            .as_ref()
            .map(|group| (setup.environment.offset.minutes(), group.recent)),
        foreground: Agents::foreground(preset.map(|chosen| &chosen.file)),
        show_hint: opened.show_hint,
        knowledge: None,
        skills: None,
        profile: None,
        project: None,
        appended,
        bare: setup.dream.is_some(),
        plugin_tools: Vec::new(),
    };
    let parts = blocking(move || -> Result<Parts, CreateError> {
        let skills =
            skills::listing(&resources, &parts.face, &found).map_err(CreateError::Shipped)?;
        let knowledge = shelved
            .then(|| listing(&resources, &listed))
            .transpose()
            .map_err(CreateError::Shipped)?
            .flatten();
        let profile = profile.block(&resources.profile().map_err(CreateError::Shipped)?);
        Ok(Parts {
            skills,
            knowledge,
            profile,
            ..parts
        })
    })
    .await?;
    Ok(Drafted {
        parts,
        shelves,
        names,
        watch,
    })
}

/// 照 `setup` 造出来的会话这时的上下文一块块的（施工 CX-1 下，`session.context` 不写会话的）：只拼不造，什么都不写。窗口、
/// 压缩线照它会用的模型；没有对话，用量全是估的。
///
/// # Errors
///
/// 同造会话：人格、随核心附带的字读不出来，策略造不出来。
pub async fn preview_context(setup: Create<'_>) -> Result<Context, CreateError> {
    let config = Turning::start(setup.configs.clone(), setup.environment.cwd.clone()).await;
    let values = config.current().resolved.values();
    let mut opened = face::open(&setup, &values).await;
    let taken = (
        std::mem::take(&mut opened.face),
        std::mem::take(&mut opened.listed),
    );
    let drafted = draft(&setup, &opened, taken).await?;
    let resources = setup.resources.clone();
    let no_tools = setup.no_tools;
    let (snapshot, layout, policy, texts) = blocking(move || {
        let (mut snapshot, layout) =
            laid(&resources, drafted.parts).map_err(CreateError::Persona)?;
        snapshot.no_tools = no_tools;
        let policy = snapshot.policy().map_err(CreateError::Policy)?;
        let texts = snapshot.driver_texts().map_err(CreateError::Policy)?;
        Ok::<_, CreateError>((snapshot, layout, policy, texts))
    })
    .await?;
    let reference = setup.model.clone().or_else(|| chat(&values));
    setup.models.ready().await;
    let model = setup.models.port(ForSession {
        id: setup.id.clone(),
        owner: setup.owner.clone(),
        config: std::sync::Arc::clone(config.current()),
        texts,
        blobs: Blobs::new(setup.root.blobs(&setup.owner)),
        reference: reference.clone(),
        sent: None,
    });
    let created = miyu_kernel::event::SessionCreated {
        model: reference,
        ..snapshot.session_created(setup.owner.clone(), setup.venue.clone(), setup.permission)
    };
    let (mut session, _) = Session::create(
        setup.id.clone(),
        setup.command.clone(),
        setup.by.clone(),
        crate::clock::Clock::default().now(),
        created,
        policy,
        setup.environment.clone(),
    );
    let mut limits = model.limits();
    crate::snapshot::stamp_preset(&mut limits, snapshot.preset.as_ref());
    session.handle(Input::Limits(limits));
    let breakdown = session.breakdown();
    let system = match layout {
        Some(layout) => crate::snapshot::pieces(&layout, breakdown.system),
        None => vec![(SystemPart::Other, breakdown.system)],
    };
    let offered = miyu_policy::offered(&snapshot.tools, snapshot.core.loading.as_ref());
    Ok(Context {
        breakdown,
        system,
        offered,
    })
}
