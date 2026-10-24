//! 权限策略（`11-权限与沙盒.md` 第二节「第一版的权限策略怎么判」，施工 4-3 下）：执行前那条链的默认实现，
//! 模块编号 `permissions`。工具报出这次调用要碰的路径，换成真实的位置、查边界表，每一条照实际生效的那一级判，
//! 合起来照最严的：有一条拒绝就拒绝，有一条要问人就问人。
//!
//! 施工 5-4 上起：读哪儿都放行，数据根除外（沙盒整盘能读，文件工具跟它一样）；执行命令，这台机器的沙盒能用就放行、
//! 在沙盒里跑（执行器写规格，`crate::sandbox`），用不了的问人。
//!
//! 施工 D-1：要问人的那几条，落在本会话放行过的范围里的（人选过「本会话都允许」，内核把那几条规则交来），不问；放行的范围
//! 照 `11-权限与沙盒.md` 第二节「放行规则管多大」，提规则时就算好。
//!
//! 施工 D-4：工具报「这一次要在沙盒外跑」的（`Tool::outside_sandbox`），完全放开照判的，只读拒绝，工作区问人、不提规则；
//! 执行器照同一个报不写沙盒的规格。问人时的说明并进工具交的几格（`Tool::asking`），执行类的写明 `sandbox: false`。
//!
//! 施工 O-31 前：在场所里做的事（访问类别 `venue`）只看会话在不在场所里（[`Place`]）：在的放行、不问，不在的拒绝。
//!
//! 施工 X-36：管理员在系统配置里放行了的工具（[`Unsandboxed`]，`11-权限与沙盒.md` A15），只碰得到自己工作区的会话照拒，只读
//! 拒，别的放行、不问；高危的照问。执行器照同一份放行表不写沙盒的规格。

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde_json::Value;

use miyu_fs::{Places, Readonly, ResolveError, Zone, resolve, resolve_itself};
use miyu_kernel::event::{Permission, Said};
use miyu_kernel::id::{AccountId, ModuleId, SessionId};
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Verdict;
use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::{Access, Worded};
use miyu_store::root::DataRoot;
use miyu_tool::{Call, Shelf, Stop, Target, Tool};

use crate::default_workspace::DefaultWorkspace;
use crate::lettering::Lettering;
use crate::unsandboxed::{Confines, Unsandboxed};

mod asking;
mod bounds;
mod confined;
mod levels;
mod unsandboxed;
mod untargeted;

use bounds::Bounds;
pub(crate) use levels::{Effective, effective};
use levels::{Mark, mark};

use asking::{Asked, ask, granted, hazardous, this_dir};
pub(crate) use confined::{Confined, Own, Reach};
use untargeted::{beyond, described, in_venue, untargeted};

/// 权限策略：一个会话一份。
pub(crate) struct Guard {
    /// 工具目录的架子：判的时候照现在的那一份找（施工 O-2 中）。
    catalog: Shelf,
    data_root: PathBuf,
    home: Option<PathBuf>,
    /// 家目录的真实位置（施工 D-1）：放行的范围拿它和换成真实位置的路径比。不换的话 Windows 上一边带 `\\?\` 前缀、一边不带，
    /// 永远比不上（CI run 960 撞见）；macOS 上家目录在链接后面时也一样。换不成的照原样。
    real_home: Option<PathBuf>,
    /// 边界表里跟环境有关的几片（临时目录、系统目录、工具链目录）：造的时候读一次，以后照它（施工 4-9 再补四下：
    /// 原来每判一次重读环境变量）。工作区每判一次换成这一轮的工作目录。
    places: Places,
    /// 拒绝时的话（施工 P-1 三补：和执行工具的端口同一份，换快照时跟着换）。
    lettering: Arc<Lettering>,
    /// 这台机器上的沙盒能不能用（核心起来时探的）：执行命令照它判。
    sandboxed: bool,
    /// 会话在不在场所里（施工 O-31 前）：在场所里做的事照它判。
    place: Place,
    /// 只碰得到自己工作区的会话才有（施工 5-12，[`Confined`]）：文件类工具只放行工作区里的，命令一律拒。
    confined: Option<Confined>,
    /// 开着的技能文件夹（施工 X-10 上），回合开始照这一刻的换（施工 X-11 下）：和在跑的调用共用这一份策略，所以上锁。
    shelves: RwLock<Vec<PathBuf>>,
    /// 这个会话的附件目录（施工 3-9 六补）：落成文件的附件放在这里，同技能文件夹单独放开只读；不和技能文件夹放在一起，
    /// 读它里面的 `SKILL.md` 不算读技能。
    attachments: Option<PathBuf>,
    /// 工作目录太宽时退回哪（施工 5-14）：没有的工作区就是工作目录。
    workplace: Option<DefaultWorkspace>,
    /// 别的程序的私密目录（施工 5-15 下）：造的时候是出厂的，回合开始照这一轮的配置换（人加的、去掉的），同技能文件夹上锁。
    private: RwLock<Vec<PathBuf>>,
    /// 只禁写的名单（施工 5-17 下）：同私密目录，造的时候是出厂的，回合开始照这一轮的配置换。
    readonly: RwLock<Readonly>,
    /// 管理员放行出沙盒的（施工 X-36）：回合开始照这一轮的配置换，和在跑的调用共用，所以上锁。
    unsandboxed: RwLock<Unsandboxed>,
    /// 管理员管得住的地方（施工 X-37 补）：会话开的时候定下，回合开始随放行表交出去。
    confines: Confines,
}

/// 会话在哪（施工 O-31 前，`session/guard.md` 第四条）：在场所里做的事（访问类别 `venue`）只在场所会话里放行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Place {
    /// 通讯平台的场所会话（场所不是 `local` 的）：放行，不问人，有没有人能确认都一样。
    Venue,
    /// 本机的会话（终端、网页）：拒绝，写给她这一句。
    Local(Worded),
}

impl Place {
    /// 本机的会话，拒绝时写给她 `text`（出厂的 `core/permissions/not-in-venue.txt`），说法是 `core/permissions/not-in-venue`。
    pub(crate) fn local(text: String) -> Place {
        Place::Local(Worded {
            text,
            said: Some(Said::new("core/permissions/not-in-venue")),
        })
    }
}

impl Guard {
    /// 照目录 `catalog` 找工具，数据根是 `data_root`，家目录是 `home`，拒绝时的话照 `lettering`，这台机器上的沙盒能不能用
    /// 是 `sandboxed`，会话在 `place`（施工 O-31 前），只碰得到自己工作区的有 `confined`（施工 5-12）。
    pub(crate) fn new(
        catalog: Shelf,
        data_root: PathBuf,
        home: Option<PathBuf>,
        lettering: Arc<Lettering>,
        (sandboxed, place, confined): (bool, Place, Option<Confined>),
    ) -> Guard {
        let places = Places::here(PathBuf::new(), data_root.clone(), home.as_deref());
        let real_home = home
            .as_deref()
            .map(|home| resolve(home, Some(home), "~").unwrap_or_else(|_| home.to_path_buf()));
        Guard {
            catalog,
            data_root,
            home,
            real_home,
            private: RwLock::new(places.private.clone()),
            readonly: RwLock::new(places.readonly.clone()),
            places,
            lettering,
            sandboxed,
            place,
            confined,
            shelves: RwLock::default(),
            attachments: None,
            workplace: None,
            unsandboxed: RwLock::default(),
            confines: Confines::default(),
        }
    }

    /// 会话自己的几处（施工 X-37 补，造会话、载入共用）：附件目录（[`Guard::with_attachments`]），管理员管得住的地方（[`Confines`]：
    /// 账号 `owner` 家目录自己放东西的几处、资源目录 `resources`，属主是机主的（`machine_owner`）加系统那一层装上的包），属主照 XDG 的那几处（施工 Y-11 上，边界表的 `own`）。
    #[must_use]
    pub(crate) fn for_session(
        mut self,
        root: &DataRoot,
        (owner, id, machine_owner): (&AccountId, &SessionId, bool),
        resources: &Path,
    ) -> Guard {
        self.confines = Confines::of(root, (owner, machine_owner), resources);
        // 会话属主自己的几处（施工 Y-11 上，家目录里照 XDG 的 pictures 这些）：边界表照工作区算，只给文件工具。成员的会话另算
        // 上他的工作区（施工 Y-2 下①）：他报的工作目录不在里面时，数据根照旧禁着，他自己的工作区不能跟着被禁。
        self.places.own = root
            .user_dirs(owner)
            .into_iter()
            .map(|(_, dir)| dir)
            .collect();
        if self
            .confined
            .as_ref()
            .is_some_and(|confined| confined.own().is_some())
        {
            self.places.own.push(root.workspace(owner));
        }
        self.with_attachments(miyu_store::attachments::dir(root, owner, id))
    }

    /// 管理员管得住的地方：回合开始照它和这一轮的放行表拼一份。
    pub(crate) fn confines(&self) -> &Confines {
        &self.confines
    }

    /// 这个会话的附件目录（施工 3-9 六补）：能读，写一律拒；别的会话的附件目录照数据根，谁都不能碰。
    #[must_use]
    pub(crate) fn with_attachments(mut self, dir: PathBuf) -> Guard {
        self.attachments = Some(dir);
        self
    }

    /// 工作目录太宽时退回的这个会话的默认工作区（施工 5-14）。
    #[must_use]
    pub(crate) fn with_workplace(mut self, workplace: DefaultWorkspace) -> Guard {
        self.workplace = Some(workplace);
        self
    }

    /// 数据根里单独放开只读的目录（施工 X-10 上，开着的技能文件夹）：能读，写一律拒；只碰得到自己工作区的会话也能读。
    #[must_use]
    pub(crate) fn with_shelves(self, shelves: Vec<PathBuf>) -> Guard {
        self.set_shelves(shelves);
        self
    }

    /// 换上这一刻开着的技能文件夹（施工 X-11 下）：回合开始照它换，下一次判照新的。
    pub(crate) fn set_shelves(&self, shelves: Vec<PathBuf>) {
        *self
            .shelves
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = shelves;
    }

    /// 照这一轮的配置 `values` 拼出私密目录（施工 5-15 下）：家目录不知道的没有（照造的时候的算）。
    pub(crate) fn private_of(&self, values: &miyu_config::Values) -> Option<Vec<PathBuf>> {
        let home = self.home.as_deref()?;
        Some(miyu_tool::private::dirs(values, home))
    }

    /// 换上这一轮的私密目录（施工 5-15 下）：回合开始照配置换，下一次判照新的。
    pub(crate) fn set_private(&self, private: Vec<PathBuf>) {
        *self
            .private
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = private;
    }

    /// 照这一轮的配置 `values` 拼出只禁写的名单（施工 5-17 下）：家目录不知道的，`~` 开头的不算。
    pub(crate) fn readonly_of(&self, values: &miyu_config::Values) -> Readonly {
        miyu_tool::readonly::list(values, self.home.as_deref())
    }

    /// 换上这一轮的只禁写名单（施工 5-17 下）：回合开始照配置换，下一次判照新的。
    pub(crate) fn set_readonly(&self, readonly: Readonly) {
        *self
            .readonly
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = readonly;
    }

    /// 判一次调用：工具名 `name`，修正过的参数 `args`，这一轮的工作目录 `cwd`，实际生效的级别 `permission`，本会话放行过的
    /// 规则 `grants`（施工 D-1）。交回判下来的，连同报的路径判的时候换成的真实位置（施工 5-18）：执行器交给工具，工具只碰
    /// 这几个。
    pub(crate) fn judge(
        &self,
        name: &str,
        args: String,
        cwd: String,
        dirs: &[String],
        permission: &Permission,
        grants: &[RawJson],
    ) -> (Verdict, Vec<PathBuf>) {
        let mut checked = Vec::new();
        let verdict = self.verdict((name, args, cwd), dirs, permission, grants, &mut checked);
        (verdict, checked)
    }

    /// 成员的会话（施工 Y-2 下①）：工作目录 `cwd` 不在他自己的几处里。场所会话的不看。
    fn astray(&self, confined: &Confined, cwd: &str) -> bool {
        confined.own().is_some_and(|own| {
            resolve(Path::new(cwd), self.home.as_deref(), cwd)
                .map_or(true, |real| !own.holds(&real))
        })
    }

    /// 同 [`Guard::judge`]，换成的真实位置记进 `checked`。
    fn verdict(
        &self,
        (name, args, cwd): (&str, String, String),
        dirs: &[String],
        permission: &Permission,
        grants: &[RawJson],
        checked: &mut Vec<PathBuf>,
    ) -> Verdict {
        // 目录里没有的：放行，执行时报现在用不了（施工 4-2）。
        let Some(tool) = self.catalog.get(name) else {
            return Verdict::Allow;
        };
        let level = effective(permission);
        let access = tool.spec().access.clone();
        // 在场所里做的事不报路径、不在沙盒外跑：只看会话在不在场所里（施工 O-31 前）。也给本机用的（`venues` 有 `local`，
        // 施工 O-31 补：接入QQ 的「主动发消息」本机的会话里也叫她发），本机的会话里照对外发消息判：本机的会话就是管理员本人。
        // 成员的本机会话不是管理员本人（施工 Y-2 下①）：不替他对外发。
        let local_venue = access == Access::Venue
            && matches!(self.place, Place::Local(_))
            && tool.venues().is_some_and(|venues| venues.local)
            && !self
                .confined
                .as_ref()
                .is_some_and(|confined| confined.own().is_some());
        if access == Access::Venue && !local_venue {
            return in_venue(&self.place);
        }
        // 只碰得到自己工作区的会话：命令只在把读也关进工作区的沙盒里跑（施工 5-12 下），关不住读的平台、沙盒用不了的
        // 机器上一律拒；成员的会话工作目录不在他自己的几处里的也拒（施工 Y-2 下①）。
        if let Some(confined) = &self.confined
            && access == Access::Execute
            && (!(self.sandboxed && miyu_sandbox::CONFINES_READS) || self.astray(confined, &cwd))
        {
            return deny(confined.no_commands());
        }
        // 报要碰的路径、要不要在沙盒外跑都只看参数，用不着她看过的。
        let call = Call {
            args,
            cwd: cwd.clone(),
            home: self.home.clone(),
            data_root: Some(self.data_root.clone()),
            seen: Arc::default(),
            stop: Stop::default(),
            sandbox: None,
            log: None,
            offset: UtcOffset::UTC,
            agents: None,
            messages: None,
            jobs: None,
            sessions: None,
            usage: None,
            questions: None,
            memory: None,
            knowledge: None,
            packages: None,
            goals: None,
            nested: None,
            ids: None,
            settings: None,
            checked: None,
            caller: None,
        };
        let asking = tool.asking(&call);
        if local_venue {
            return untargeted(level, name, Access::Outbound, self.sandboxed, &asking);
        }
        let verdict = self.paths(
            tool.as_ref(),
            (name, level),
            &call,
            dirs,
            grants,
            &asking,
            checked,
        );
        // 管理员放行了的（施工 X-36）：照名字和来处认，工具自己报不报都一样。
        let granted = self.admin_granted(name, &call.cwd, dirs, grants);
        // 要在沙盒外跑的：只碰得到自己工作区的会话一律拒，没人能放行（施工 5-12 下）；管理员放行了的也一样（施工 X-36）。
        if let Some(confined) = &self.confined
            && (tool.outside_sandbox(&call) || granted)
        {
            return deny(confined.no_commands());
        }
        // 改装了什么的（施工 F-10 下）照沙盒外跑的同一张表：报的路径照判，再照级别：工作区每次问人、只读拒绝。
        // 高危的（施工 5-17）：灭顶的拒、别的问人，先于别的放行；只读本来就拒一切写。完全放开只拦灭顶的，别的照常放行（施工
        // 5-17 再补，2026-10-22 项目主人定）。
        if level != Effective::ReadOnly
            && let Some(hazard) = tool.hazard(&call)
            && (level == Effective::Workspace || hazard.refused.is_some())
        {
            return hazardous(verdict, (name, access), hazard, &asking);
        }
        // 管理员放行了的：只读拒，别的不问；报的路径判了拒绝的照拒（施工 X-36）。
        if granted {
            return match (verdict, level) {
                (denied @ Verdict::Deny { .. }, _) => denied,
                (_, Effective::ReadOnly) => deny(self.lettering.guard().read_only()),
                _ => Verdict::Allow,
            };
        }
        if tool.outside_sandbox(&call) || access == Access::Install {
            // 命令越过沙盒问人时另提「此目录不再询问」：放行工作目录的写（施工 5-15）。
            let offer =
                (access == Access::Execute).then(|| this_dir(name, &cwd, self.home.as_deref()));
            let texts = self.lettering.guard();
            beyond(verdict, level, (name, access, offer), &asking, &texts)
        } else {
            verdict
        }
    }

    /// 照报的路径判；一条都没报的照访问类别判（第四条）。每一条换成的真实位置记进 `checked`（施工 5-18）。
    #[allow(clippy::too_many_arguments)]
    fn paths(
        &self,
        tool: &dyn Tool,
        (name, level): (&str, Effective),
        call: &Call,
        dirs: &[String],
        grants: &[RawJson],
        asking: &[(&'static str, String)],
        checked: &mut Vec<PathBuf>,
    ) -> Verdict {
        let access = tool.spec().access.clone();
        let targets = tool.targets(call);
        if targets.is_empty() {
            return untargeted(level, name, access, self.sandboxed, asking);
        }
        let Bounds {
            cwd,
            workspace,
            places,
            boundary,
        } = self.bounds(&call.cwd, dirs);
        let mut asked = Vec::new();
        for target in targets {
            let real = match self.real(&cwd, &target) {
                Ok(real) => real,
                Err(error) => {
                    return deny(
                        self.lettering
                            .guard()
                            .unresolvable(&target.path, &error.to_string()),
                    );
                }
            };
            checked.push(real.clone());
            let zone = boundary.zone(&real);
            // 只碰得到自己工作区的：工作区、加进来的目录以外的一律拒（施工 5-12），读放开只读的技能文件夹除外（施工 X-10 上）。
            if let Some(confined) = &self.confined
                && !confined.inside(&real, &workspace, &places.dirs)
                && !(zone == Zone::Shelf && !target.write)
            {
                return deny(confined.outside(&target.path));
            }
            match mark(level, zone, target.write) {
                Mark::Allow => {}
                Mark::Ask => asked.push(Asked::new(real, target.write, zone, &boundary)),
                Mark::Forbidden if boundary.knowledge(&real) => {
                    return deny(self.lettering.guard().forbidden_kb(&target.path));
                }
                Mark::Forbidden => return deny(self.lettering.guard().forbidden(&target.path)),
                Mark::ReadOnly => return deny(self.lettering.guard().read_only()),
            }
        }
        // 放行过的不问（施工 D-1）：只读、数据根上面已经拦下了，走不到这里。
        asked.retain(|asked| !granted(grants, asked, &boundary));
        if asked.is_empty() {
            Verdict::Allow
        } else {
            ask(name, access, &asked, self.real_home.as_deref())
        }
    }
}

impl Guard {
    /// 真实位置是 `cwd` 的工作目录对应的工作区（施工 5-14）：没交默认工作区的就是工作目录；建不成的照样用，写时工具自己报错。
    fn workspace(&self, cwd: &Path) -> PathBuf {
        let Some(place) = &self.workplace else {
            return cwd.to_path_buf();
        };
        let root =
            std::fs::canonicalize(&self.data_root).unwrap_or_else(|_| self.data_root.clone());
        let home = self.real_home.as_deref();
        place.made(cwd, home, &root).unwrap_or_else(|error| {
            tracing::warn!(target: crate::TARGET, error = %error, "default workspace not made");
            place.of(cwd, home, &root)
        })
    }

    /// 要碰的这一条换成真实的位置。碰的是这一条本身的（`trash`，施工 4-9 再补二）：最后一段不跟链接，和工具碰的是
    /// 同一个；没有名字可碰的（`.`、`..`、`~`），照整条换。
    fn real(&self, cwd: &Path, target: &Target) -> Result<PathBuf, ResolveError> {
        let home = self.home.as_deref();
        if target.itself
            && let Some(real) = resolve_itself(cwd, home, &target.path)?
        {
            return Ok(real);
        }
        resolve(cwd, home, &target.path)
    }
}

/// 拒绝，写给她这一句，连同给人看的说法。
fn deny(worded: Worded) -> Verdict {
    Verdict::Deny {
        module: module(),
        text: worded.text,
        human: worded.said,
    }
}

/// 权限策略的模块编号。
///
/// # Panics
///
/// 实际不会：`permissions` 合模块编号的写法。
fn module() -> ModuleId {
    ModuleId::parse("permissions").expect("permissions 合模块编号的写法")
}

/// 自己拼的 JSON，写成原样的 JSON。
///
/// # Panics
///
/// 实际不会：自己拼的 JSON 一定读得回来。
fn raw(value: &Value) -> RawJson {
    serde_json::from_str(&value.to_string()).expect("自己拼的 JSON 读得回来")
}

#[cfg(test)]
mod tests;
