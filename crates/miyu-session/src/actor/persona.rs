//! 人格、预设的文件改了，包的工具变了，下一个回合换上（施工 P-1 再补、P-2 下、O-2 中，`docs/blueprint/kernel/session.md`
//! 「换策略快照」，`personas.md`「怎么走」第 6 条，`presets.md`「改了文件」，`providers.md`「目录换代」）。每个回合开始，照会话的
//! 人格、预设的编号把几层重新找一遍、算指纹，看工具目录的架子换没换代，和现在的快照比：一样的什么都不做；不一样的照新的重拼
//! （记忆的范围、有没有人能确认照旧快照；工具面照新的预设、现在的目录重新筛，提供者的工具照现在的登记，别的以前就有的照旧
//! 快照里的原样），拼出来不一样、核心的字没变才换：存成 blob，策略交给内核放着，交回新快照的哈希。以前造的快照没有指纹的
//! 不换；找不着、写错了的照旧用原来的，记一行运行日志。换的时候执行器手里取自快照的几份字（驱动的占位、替工具写的两句、
//! 权限策略的几句）一起换成新快照的（施工 P-1 三补）：升级过的老会话也换得了；光是升级不换（核心的字变了、人格预设工具面
//! 都没变的照旧），不为它断一次缓存。知识库的两件照这一刻查不查得到给、拿掉（施工 K-1 下，`knowledge.md`「怎么走」第六条）：
//! 建了第一个库下一个回合有，删光了下一个回合没有。

use std::collections::BTreeMap;
use std::sync::Arc;

use miyu_config::Values;
use miyu_drivers::DriverTexts;
use miyu_kernel::id::{ContentHash, VenueId};
use miyu_kernel::session::Policy;
use miyu_policy::PersonaTexts;
use miyu_policy::features::Features;
use miyu_policy::preset::{Chosen, MEMORY, ROLEPLAY};
use miyu_policy::{GuardTexts, RunTexts, Snapshot};
use miyu_store::blob::Blobs;
use miyu_store::personas::{PersonaError, Personas};
use miyu_store::resources::ResourceRoot;
use miyu_tool::{Edition, Shelf};

use super::Actor;
use crate::TARGET;
use crate::agents::{Agents, Reach};
use crate::blocking::blocking;
use crate::knowledge::Shelves;
use crate::open::PresetPlaces;
use crate::snapshot::{Parts, build};
use crate::spawn::Lineage;
use face::{followed, offered, refaced, reshelved, shelf_tools};
use recast::Wanted;

/// 换快照要的：人格的几层、资源目录、存快照的地方，现在的快照，是不是子会话；预设的几层和装了的软件，重新筛工具面要的目录
/// 架子、场所、父会话（施工 P-2 下、O-2 中）。
#[derive(Clone)]
pub(crate) struct Refresh {
    pub(crate) personas: Personas,
    pub(crate) resources: ResourceRoot,
    pub(crate) blobs: Blobs,
    pub(crate) snapshot: Snapshot,
    pub(crate) child: bool,
    pub(crate) presets: Option<PresetPlaces>,
    pub(crate) tools: Shelf,
    /// 现在的快照照的是架子上的哪一代（施工 O-2 中）：载入的不知道是哪一代，是没有，第一个回合照现在的对一次。
    pub(crate) seen: Option<Edition>,
    pub(crate) venue: VenueId,
    pub(crate) lineage: Option<Lineage>,
    /// 这个会话的知识库（施工 K-1 下）：回合开始照它判查不查得到。没交的当查不到。
    pub(crate) knowledge: Option<Arc<Shelves>>,
    /// 回合开始再找一遍技能（施工 X-11 下，`crate::skills::Watch`）：没交装了的软件包的（测试里造的）不找。
    pub(crate) skills: Option<crate::skills::Watch>,
    /// 场所显示得了图片吗（施工 X-25）：显示不了的，换预设时要图的包照旧扣下。
    pub(crate) images: bool,
    /// 属主的项目文件（施工 H-3，`miyu_store::projects`）：回合开始照会话现在在哪个项目找附加提示词。
    pub(crate) projects: std::path::PathBuf,
    /// 记忆放在哪一间（施工 P-13）：和 [`crate::Handle`] 共用，换人格时跟着换。
    pub(crate) room: crate::memory::RoomCell,
    /// 每个场所开着的场所插件（施工 O-60）：回合开始照这一刻的重筛工具面。
    pub(crate) plugins: crate::VenuePlugins,
    /// 快照冻住了（fork 子代理，施工 A-F 中）：一直用父会话那一份，请求前缀才和父会话、兄弟的一样，回合开始不换。
    pub(crate) frozen: bool,
}

/// 看了一遍的结果。
enum Seen {
    /// 没改，或者以前造的快照没有指纹。
    Same,
    /// 改了，换上这一份：快照、照它造的策略、它的哈希，执行器的几份字（施工 P-1 三补），换了人格的连同新人格的知识库
    /// （施工 P-13）。
    Swapped(
        Box<Snapshot>,
        Box<Policy>,
        ContentHash,
        Box<Retext>,
        Option<Recast>,
    ),
    /// 人格、预设找不着、写错了、读不了：照旧。
    Unreadable(String),
    /// 改了，换不了：拼不成、存不进：照旧。
    Kept(String),
}

impl Actor {
    /// 人格的文件改了，下一个回合换上：造会话、载入时交。
    pub(crate) fn watch_persona(&mut self, refresh: Refresh) {
        self.persona = Some(refresh);
    }

    /// 回合开始：看人格、预设的文件改了没有，工具目录换没换代。改了、换得了的，新策略交给内核放着，交回新快照的哈希。
    /// `values` 是这一轮冻结的配置：新打开的 `subagent` 照它填能选的池。
    pub(super) async fn refresh_persona(&mut self, values: Values) -> Option<ContentHash> {
        // 裸的（整理会话，施工 D-1 上）不换：换了就拼回 system 和工具。fork 子代理的（施工 A-F 中）也不换。
        let refresh = self
            .persona
            .clone()
            .filter(|refresh| !refresh.snapshot.bare && !refresh.frozen)?;
        let persona = refresh.snapshot.persona.clone();
        let now = refresh.tools.edition();
        let seen = now.clone();
        // 扩展这一刻要放在 system 尾部的（施工 X-20）：没说过的照快照里钉的。
        let asked = self
            .tools
            .hooks()
            .map(|(port, session)| port.appended(session.as_str()))
            .unwrap_or_default();
        // 会话现在在哪个项目（施工 H-3）：附加提示词照它找。人中途换的人格、预设（施工 P-13）。
        let project = self.session.project().map(str::to_string);
        let wanted = Wanted::of(&self.session, &refresh.snapshot, refresh.child);
        match blocking(move || look(&refresh, &values, &now, (asked, project, wanted))).await {
            Seen::Same => {
                if let Some(refresh) = self.persona.as_mut() {
                    refresh.seen = Some(seen);
                }
                None
            }
            Seen::Swapped(snapshot, policy, hash, texts, recast) => {
                tracing::info!(target: TARGET, persona = persona.as_deref(), to = snapshot.persona.as_deref(), "persona swapped");
                self.session.stage_policy(*policy);
                let Retext { driver, run, guard } = *texts;
                self.model.retext(driver);
                self.tools.lettering().set(run, guard);
                let to = snapshot.persona.clone();
                if let Some(refresh) = self.persona.as_mut() {
                    refresh.snapshot = *snapshot;
                    refresh.seen = Some(seen);
                }
                if let Some(Recast { shelves }) = recast {
                    self.rehome(to, shelves).await;
                }
                Some(hash)
            }
            Seen::Unreadable(error) => {
                tracing::warn!(target: TARGET, persona = persona.as_deref(), error = error.as_str(), "persona unreadable");
                None
            }
            Seen::Kept(error) => {
                tracing::warn!(target: TARGET, persona = persona.as_deref(), error = error.as_str(), "persona not swapped");
                None
            }
        }
    }
}

/// 在阻塞线程里看一遍：`now` 是架子上现在的那一代，`asked` 是开场所会话的扩展这一刻要插的那一段（施工 X-20，没说过的是
/// `None`）。
fn look(
    refresh: &Refresh,
    values: &Values,
    now: &Edition,
    (asked, project, wanted): (BTreeMap<String, Option<String>>, Option<String>, Wanted),
) -> Seen {
    let old = &refresh.snapshot;
    // 项目的附加提示词（施工 H-3）：照这一刻拼，和快照里的不一样这一轮要换。
    let projected = match project::block(refresh, project.as_deref()) {
        Ok(block) => block,
        Err(error) => return Seen::Kept(error),
    };
    let reprojected = projected != old.project;
    // 扩展说的和钉的不一样（施工 X-20）：这一轮要换。
    // 只看扩展放的：MCP 服务的说明跟着工具面变（施工 X-39），说明自己变了不算。
    let said = asked.iter().any(|(key, text)| {
        crate::appended::from_extension(key) && old.appended.get(key) != text.as_ref()
    });
    // 无人格的（施工 P-4 上）只看预设；人格的文件没了的照快照里的接着用，不换也不报。人换了人格的照新的（施工 P-13）。
    let (name, texts) = match (&wanted.persona, &old.persona, &old.persona_digest) {
        (Some(to), _, _) => match recast::texts(refresh, to.as_deref()) {
            Ok(texts) => (to.clone(), texts),
            Err(seen) => return seen,
        },
        (None, None, _) => (None, PersonaTexts::default()),
        (None, Some(_), None) => return Seen::Same,
        (None, Some(id), Some(_)) => match refresh.personas.find(id) {
            Ok(found) => (old.persona.clone(), found.texts),
            Err(PersonaError::NotFound(_)) => return Seen::Same,
            Err(error) => return Seen::Unreadable(error.to_string()),
        },
    };
    let preset = match &wanted.preset {
        // 换成无工具（施工 P-16 下）：内核记的是空的编号。
        Some(id) if id.is_empty() => Ok(Preset::NoTools { changed: true }),
        Some(id) => recast::preset(refresh, id, values).map(Preset::Changed),
        None => preset(refresh, values),
    };
    let preset = match preset {
        Ok(preset) => preset,
        Err(error) => return Seen::Unreadable(error),
    };
    let same = wanted.persona.is_none()
        && old
            .persona_digest
            .as_ref()
            .is_none_or(|digest| texts.digest() == *digest);
    // 换了人格的知识库照新人格查（施工 P-13）。
    let shelves = match &wanted.persona {
        Some(to) => recast::shelves(refresh, to.as_deref()),
        None => refresh.knowledge.clone(),
    };
    // 目录换了代：预设找得回来的（或者本来没有预设的）照现在的目录重新筛；以前造的、找不回来的照旧（施工 O-2 中）。
    let moved = refresh
        .seen
        .as_ref()
        .is_none_or(|seen| seen.generation != now.generation)
        && !matches!(preset, Preset::Unknown);
    let catalog = &now.catalog;
    // 知识库的两件照这一刻查不查得到、照现在的目录和预设该有哪几件（施工 K-1 下）；和快照里的不一样才算改了。预设找不回来的
    // 照旧。
    let listed = shelves
        .as_ref()
        .map(|shelves| shelves.listed())
        .unwrap_or_default();
    let reach = shelves
        .as_ref()
        .map_or(Reach::Save, |shelves| shelves.reach(&listed));
    let shelved = match &preset {
        Preset::Unknown => None,
        Preset::NoTools { .. } => Some(Vec::new()),
        Preset::Nothing => Some(offered(refresh, values, catalog, None, reach)),
        Preset::Same(chosen) | Preset::Changed(chosen) => {
            Some(offered(refresh, values, catalog, Some(&chosen.file), reach))
        }
    };
    let flipped = shelved
        .as_ref()
        .is_some_and(|fresh| shelf_tools(fresh) != shelf_tools(&old.tools));
    // 场所插件（施工 O-60）：照这一刻的目录、预设，这个场所该给的和工具面上现在的不一样，这一轮要换。预设找不回来的照旧。
    let plugs = (&refresh.plugins, &refresh.venue);
    let replugged = shelved.as_ref().is_some_and(|fresh| {
        let wanted = crate::plugins::replug(Vec::new(), fresh, catalog, plugs);
        crate::plugins::present(&wanted, catalog) != crate::plugins::present(&old.tools, catalog)
    });
    // system 里列出来的那一块照这一刻该有的两件、查得到的库拼（施工 K-1 补）：和快照里的不一样才算改了（建了库、改了名字、
    // 说明）。预设找不回来的照旧。
    let listing = match &shelved {
        Some(fresh) if !shelf_tools(fresh).is_empty() => {
            match crate::knowledge::listing(&refresh.resources, &listed) {
                Ok(block) => block,
                Err(error) => return Seen::Kept(error.to_string()),
            }
        }
        Some(_) => None,
        None => old.knowledge.clone(),
    };
    let relisted = listing != old.knowledge;
    if same
        && !moved
        && !flipped
        && !replugged
        && !relisted
        && !said
        && !reprojected
        && !matches!(
            preset,
            Preset::Changed(_) | Preset::NoTools { changed: true }
        )
    {
        return Seen::Same;
    }
    let (face, pin) = match &preset {
        Preset::NoTools { .. } => (Vec::new(), None),
        Preset::Changed(chosen) => (
            refaced(refresh, values, catalog, &chosen.file, reach),
            Some(chosen.pin()),
        ),
        Preset::Same(chosen) if moved => (
            followed(refresh, values, catalog, Some(&chosen.file), reach),
            old.preset.clone(),
        ),
        Preset::Nothing if moved => (
            followed(refresh, values, catalog, None, reach),
            old.preset.clone(),
        ),
        _ => (old.tools.clone(), old.preset.clone()),
    };
    let face = match &shelved {
        Some(fresh) => crate::plugins::replug(reshelved(face, fresh), fresh, catalog, plugs),
        None => face,
    };
    let plugin_tools = crate::plugins::present(&face, catalog);
    let refaced = face.len() != old.tools.len()
        || face
            .iter()
            .zip(&old.tools)
            .any(|(now, was)| now.name != was.name);
    let appended = crate::appended::resolve(catalog, &face, (&old.appended, &asked), refaced);
    let reappended = appended != old.appended;
    let parts = Parts {
        name,
        texts,
        attended: old.attended,
        face,
        memory: old.memory.clone(),
        child: refresh.child,
        preset: pin,
        roleplay: features(refresh).is_none_or(|features| features.installed(ROLEPLAY)),
        // 群会话照旧快照钉下的时区（施工 O-13 中）：换了时区的机器上换人格，前缀里的钟点也不变。
        group: old.group.as_ref().map(|chat| {
            (
                chat.offset,
                chat.recent.as_ref().and_then(|words| words.limit),
            )
        }),
        // 换了预设的照新的定后台运行（施工 T-1 上），别的照旧。
        foreground: match &preset {
            Preset::Changed(chosen) => Agents::foreground(Some(&chosen.file)),
            _ => old.foreground,
        },
        // show 的那一句照造会话时钉下的（施工 D-5 补二）：会话中途改了配置不跟。
        show_hint: !old.no_show_hint,
        knowledge: listing,
        // 无工具的没有技能那一块（施工 P-16 下）：没有 `read`，列出来也读不了。
        skills: match &preset {
            Preset::NoTools { .. } => None,
            _ => old.skills.clone(),
        },
        // 「关于你」造会话时钉下（施工 P-9）：换人格、换预设照旧，不重读配置。
        profile: old.profile.clone(),
        project: projected,
        appended,
        bare: false,
        plugin_tools,
    };
    let mut new = match build(&refresh.resources, parts) {
        Ok(new) => new,
        Err(error) => return Seen::Kept(error.to_string()),
    };
    new.no_tools = matches!(preset, Preset::NoTools { .. });
    // 目录换了代、这个会话的工具面没变的（只给群的、预设关着的），什么都不记。
    if new.hash() == old.hash() {
        return Seen::Same;
    }
    // 程序升级过（核心的字变了），人格、预设都没改、工具面也没变的：照旧，不为升级断一次缓存（施工 P-1 三补）。
    let edited = !same
        || matches!(
            preset,
            Preset::Changed(_) | Preset::NoTools { changed: true }
        )
        || relisted
        || reappended
        || reprojected;
    if !edited && !old.swappable(&new) && new.tools == old.tools {
        return Seen::Same;
    }
    let built = || -> Result<(Policy, Retext), String> {
        let texts = Retext {
            driver: new.driver_texts().map_err(|error| error.to_string())?,
            run: new.run_texts().map_err(|error| error.to_string())?,
            guard: new.guard_texts().map_err(|error| error.to_string())?,
        };
        Ok((new.policy().map_err(|error| error.to_string())?, texts))
    };
    let (policy, texts) = match built() {
        Ok(built) => built,
        Err(error) => return Seen::Kept(error),
    };
    if let Err(error) = refresh.blobs.put(&new.to_bytes()) {
        return Seen::Kept(error.to_string());
    }
    let hash = new.hash();
    let recast = wanted.persona.map(|_| Recast { shelves });
    Seen::Swapped(
        Box::new(new),
        Box::new(policy),
        hash,
        Box::new(texts),
        recast,
    )
}

/// 换了人格（施工 P-13）：快照换上以后 actor 照它挪记忆、换知识库。
struct Recast {
    /// 新人格的知识库：没接知识库的没有。
    shelves: Option<Arc<Shelves>>,
}

/// 执行器取自快照的几份字（施工 P-1 三补）：换快照时一起换。
struct Retext {
    /// 驱动的占位：交给请求模型的端口。
    driver: DriverTexts,
    /// 执行工具时替工具写的两句。
    run: RunTexts,
    /// 权限策略的几句。
    guard: GuardTexts,
}

/// 快照里的预设现在是什么样（施工 P-2 下；O-2 中重新筛工具面也要它）。
enum Preset {
    /// 这个会话没有预设：以前造的、测试里造的，不筛。
    Nothing,
    /// 明着选了无工具（施工 P-16 下）：工具面是空的。`changed` 是这一轮刚换成它的。
    NoTools { changed: bool },
    /// 找不回来：以前造的快照没有指纹，没交预设的几层。
    Unknown,
    /// 没改：现在找到的那一份。
    Same(Chosen),
    /// 改了：新找到的（记忆照开会话时的）。
    Changed(Chosen),
}

/// 装了的功能（施工 F-3 上）：没交预设几层的（测试里造的、以前的）没有，当都装着。
fn features(refresh: &Refresh) -> Option<&Features> {
    refresh.presets.as_ref().map(|places| &places.features)
}

/// 照快照里预设的编号重新找一遍（施工 P-2 下），全局开关照这一轮的配置 `values`（施工 X-27）。
fn preset(refresh: &Refresh, values: &Values) -> Result<Preset, String> {
    if refresh.snapshot.no_tools {
        return Ok(Preset::NoTools { changed: false });
    }
    let Some(pin) = &refresh.snapshot.preset else {
        return Ok(Preset::Nothing);
    };
    let Some(places) = &refresh.presets else {
        return Ok(Preset::Unknown);
    };
    if pin.digest.is_none() {
        return Ok(Preset::Unknown);
    }
    let found = places
        .presets
        .find(&pin.id)
        .map_err(|error| error.to_string())?;
    let features = &places.features;
    let memory = !pin.off.iter().any(|feature| feature == MEMORY);
    // 这个人关了的包（施工 U23 末上）照这一轮的配置扣下：开着的会话下一个回合就没有它的东西。
    let file = found
        .file
        .following(crate::switches::of(values))
        .switching_off(crate::packages_off::of(values), features);
    let chosen = Chosen::new(found.id, file, features).keeping_memory(memory, features);
    let chosen = match refresh.images {
        true => chosen,
        false => chosen.blind(features),
    };
    // 以前的快照记的是包的编号（施工 F-3 上）：照现在的功能读一样的算没改，不为改了写法换一次快照。
    Ok(match pin.means_the_same(&chosen.pin(), features) {
        true => Preset::Same(chosen),
        false => Preset::Changed(chosen),
    })
}

mod context;
#[cfg(test)]
mod context_tests;
mod face;
mod next;
#[cfg(test)]
mod preset_tests;
mod project;
mod recast;
#[cfg(test)]
mod recast_tests;
#[cfg(test)]
mod shelf_tests;
mod spent;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;
