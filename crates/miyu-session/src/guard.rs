//! 权限策略（`11-权限与沙盒.md` 第二节「第一版的权限策略怎么判」，施工 4-3 下）：执行前那条链的默认实现，
//! 模块编号 `permissions`。工具报出这次调用要碰的路径，换成真实的位置、查边界表，每一条照实际生效的那一级判，
//! 合起来照最严的：有一条拒绝就拒绝，有一条要问人就问人。
//!
//! 施工 5-4 上起：读哪儿都放行，数据根除外（沙盒整盘能读，文件工具跟它一样）。A16 起（施工 LV-1）只剩完全放开、只读两级：
//! 完全放开执行命令不进沙盒；只读时这台机器的沙盒能用就放行、在沙盒里跑（执行器写规格，`crate::sandbox`），用不了的问人。
//! 高危命令、装包卸包在完全放开下也问人；高危的里面，本人的会话在设置页「受保护的命令」关掉的那几类不问（施工 HZ-1，A18）。
//!
//! 施工 D-1：要问人的那几条，落在本会话放行过的范围里的（人选过「本会话都允许」，内核把那几条规则交来），不问；放行的范围
//! 照 `11-权限与沙盒.md` 第二节「放行规则管多大」，提规则时就算好。
//!
//! 问人时的说明并进工具交的几格（`Tool::asking`），执行类的写明 `sandbox: false`。
//!
//! 施工 O-31 前：在场所里做的事（访问类别 `venue`）只看会话在不在场所里（[`Place`]）：在的放行、不问，不在的拒绝。

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde_json::Value;

use miyu_fs::{Places, ResolveError, Zone, resolve, resolve_itself};
use miyu_kernel::event::{Permission, Said};
use miyu_kernel::id::{ModuleId, SessionId};
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Verdict;
use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::{Access, Worded};
use miyu_store::root::DataRoot;
use miyu_tool::{Call, Shelf, Stop, Target, Tool};

use crate::default_workspace::DefaultWorkspace;
use crate::lettering::Lettering;

mod asking;
mod bounds;
mod confined;
mod levels;
mod untargeted;

use bounds::Bounds;
pub(crate) use levels::{Effective, effective};
use levels::{Mark, mark};

use asking::{Asked, ask, granted, hazardous};
pub(crate) use confined::Confined;
use untargeted::{described, in_venue, untargeted};

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
    /// 受保护的命令里关掉的那几类（施工 HZ-1，`permission.confirm.*`）：造的时候是空的（都问），回合开始照这一轮的配置换。
    /// 只有本人的会话看它，外部身份的会话照旧都问。
    waived: RwLock<Vec<&'static str>>,
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
        let knowledge = miyu_store::root::DataRoot::at(data_root.clone()).knowledge();
        let places = Places::here(
            PathBuf::new(),
            (data_root.clone(), knowledge),
            home.as_deref(),
        );
        let real_home = home
            .as_deref()
            .map(|home| resolve(home, Some(home), "~").unwrap_or_else(|_| home.to_path_buf()));
        Guard {
            catalog,
            data_root,
            home,
            real_home,
            private: RwLock::new(places.private.clone()),
            places,
            lettering,
            sandboxed,
            place,
            confined,
            shelves: RwLock::default(),
            attachments: None,
            workplace: None,
            waived: RwLock::default(),
        }
    }

    /// 会话自己的几处（造会话、载入共用）：附件目录（[`Guard::with_attachments`]），照 XDG 的那几处（施工 Y-11 上，边界表的
    /// `own`）。
    #[must_use]
    pub(crate) fn for_session(mut self, root: &DataRoot, id: &SessionId) -> Guard {
        // 数据根里照 XDG 的几处（施工 Y-11 上，pictures 这些）：边界表照工作区算，只给文件工具。
        self.places.own = root.user_dirs().into_iter().map(|(_, dir)| dir).collect();
        self.with_attachments(miyu_store::attachments::dir(root, id))
    }

    /// 工具目录里这时的 `name` 的访问类别（施工 HK-1：照它查有没有挂着扩展的守卫）；目录里没有的没有。
    pub(crate) fn access_of(&self, name: &str) -> Option<Access> {
        Some(self.catalog.get(name)?.spec().access.clone())
    }

    /// 会话关没关起来（施工 HK-2：发 `tool.after` 照它判谁碰得到这个会话）。
    pub(crate) fn confined(&self) -> bool {
        self.confined.is_some()
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

    /// 换上这一轮关掉的那几类受保护的命令（施工 HZ-1，[`crate::settings::ConfirmSettings::waived`]）：回合开始照配置换，下
    /// 一次判照新的。
    pub(crate) fn set_waived(&self, waived: Vec<&'static str>) {
        *self
            .waived
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = waived;
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
        // 施工 O-31 补：接入QQ 的「主动发消息」本机的会话里也叫她发），本机的会话里照对外发消息判：本机的会话就是主人本人。
        let local_venue = access == Access::Venue
            && matches!(self.place, Place::Local(_))
            && tool.venues().is_some_and(|venues| venues.local);
        if access == Access::Venue && !local_venue {
            return in_venue(&self.place);
        }
        // 只碰得到自己工作区的会话：命令只在把读也关进工作区的沙盒里跑（施工 5-12 下），关不住读的平台、沙盒用不了的
        // 机器上一律拒；系统服务的 Unix 套接字管不住的机器上也拒（施工 MB-1，[`miyu_sandbox::confines_others`]）。
        if let Some(confined) = &self.confined
            && access == Access::Execute
            && !(self.sandboxed && miyu_sandbox::confines_others())
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
            schedules: None,
            nested: None,
            ids: None,
            settings: None,
            checked: None,
            caller: None,
            previewed: None,
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
        // 装包、卸包（施工 SU-4 中）：只碰得到自己工作区的会话一律拒，没人能放行（施工 5-12 下）。
        if let Some(confined) = &self.confined
            && access == Access::Install
        {
            return deny(confined.no_commands());
        }
        // 高危的（施工 5-17）：灭顶的拒、别的问人，先于别的放行；只读本来就拒一切写。A16 起完全放开也照问（施工 LV-1，
        // 2026-11-09 项目主人定：不做沙盒不等于不做保护），改掉 5-17 再补的「完全放开只拦灭顶的」。本人的会话里设置页关掉的那
        // 几类不问（施工 HZ-1，A18）；外部身份的会话不看开关。
        if level != Effective::ReadOnly
            && let Some(hazard) = tool.hazard(&call)
        {
            let waived = match self.confined {
                Some(_) => Vec::new(),
                None => self
                    .waived
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .clone(),
            };
            let tool = (name, access.clone());
            if let Some(verdict) = hazardous(&verdict, tool, hazard, &waived, &asking) {
                return verdict;
            }
        }
        // 装包、卸包照报的路径判（施工 F-10 下）：只读拒；完全放开也每次问人，不提规则（施工 LV-1，2026-11-09 定：装扩展等于在
        // 这台机器上跑别人的程序，同 `curl | sh` 归高危）。没人能确认的地方，内核照问人拒。
        match (verdict, access, level) {
            (denied @ Verdict::Deny { .. }, _, _) => denied,
            (_, Access::Install, Effective::ReadOnly) => deny(self.lettering.guard().read_only()),
            (_, Access::Install, Effective::Full) => Verdict::Ask {
                module: module(),
                access: Access::Install,
                rule: None,
                detail: Some(described(name, &Access::Install, &asking)),
            },
            (verdict, _, _) => verdict,
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
                Mark::Ask => asked.push(Asked {
                    real,
                    write: target.write,
                    zone,
                }),
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
