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

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde_json::{Value, json};

use miyu_fs::{Boundary, Places, ResolveError, Zone, resolve, resolve_itself};
use miyu_kernel::event::{Level, Permission, Said};
use miyu_kernel::id::ModuleId;
use miyu_kernel::raw::RawJson;
use miyu_kernel::session::Verdict;
use miyu_kernel::time::UtcOffset;
use miyu_kernel::tool::{Access, Worded};
use miyu_policy::GuardTexts;
use miyu_tool::{Call, Shelf, Stop, Target, Tool};

use crate::lettering::Lettering;

mod asking;
mod confined;

use asking::{Asked, ask, granted};
pub(crate) use confined::Confined;

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

/// 实际生效的那一级。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Effective {
    /// 完全放开。
    Full,
    /// 工作区。
    Workspace,
    /// 只读：开着只读开关，或者常用的那一级不认识（按最严的算）。
    ReadOnly,
}

/// 一条路径判下来是什么。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// 放行。
    Allow,
    /// 要问人。
    Ask,
    /// 拒绝：碰到了数据根。
    Forbidden,
    /// 拒绝：只读的时候要写。
    ReadOnly,
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
            places,
            lettering,
            sandboxed,
            place,
            confined,
            shelves: RwLock::default(),
            attachments: None,
        }
    }

    /// 这个会话的附件目录（施工 3-9 六补）：能读，写一律拒；别的会话的附件目录照数据根，谁都不能碰。
    #[must_use]
    pub(crate) fn with_attachments(mut self, dir: PathBuf) -> Guard {
        self.attachments = Some(dir);
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

    /// 判一次调用：工具名 `name`，修正过的参数 `args`，这一轮的工作目录 `cwd`，实际生效的级别 `permission`，本会话放行过的
    /// 规则 `grants`（施工 D-1）。
    pub(crate) fn judge(
        &self,
        name: &str,
        args: String,
        cwd: String,
        dirs: &[String],
        permission: &Permission,
        grants: &[RawJson],
    ) -> Verdict {
        // 目录里没有的：放行，执行时报现在用不了（施工 4-2）。
        let Some(tool) = self.catalog.get(name) else {
            return Verdict::Allow;
        };
        let level = effective(permission);
        let access = tool.spec().access.clone();
        // 在场所里做的事不报路径、不在沙盒外跑：只看会话在不在场所里（施工 O-31 前）。
        if access == Access::Venue {
            return in_venue(&self.place);
        }
        // 只碰得到自己工作区的会话：命令只在把读也关进工作区的沙盒里跑（施工 5-12 下），关不住读的平台、沙盒用不了的
        // 机器上一律拒。
        if let Some(confined) = &self.confined
            && access == Access::Execute
            && !(self.sandboxed && miyu_sandbox::CONFINES_READS)
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
            ids: None,
        };
        let asking = tool.asking(&call);
        let verdict = self.paths(tool.as_ref(), name, level, &call, dirs, grants, &asking);
        // 要在沙盒外跑的：只碰得到自己工作区的会话一律拒，没人能放行（施工 5-12 下）。
        if let Some(confined) = &self.confined
            && tool.outside_sandbox(&call)
        {
            return deny(confined.no_commands());
        }
        // 改装了什么的（施工 F-10 下）照沙盒外跑的同一张表：报的路径照判，再照级别：工作区每次问人、只读拒绝。
        if tool.outside_sandbox(&call) || access == Access::Install {
            beyond(
                verdict,
                level,
                name,
                access,
                &asking,
                &self.lettering.guard(),
            )
        } else {
            verdict
        }
    }

    /// 照报的路径判；一条都没报的照访问类别判（第四条）。
    #[allow(clippy::too_many_arguments)]
    fn paths(
        &self,
        tool: &dyn Tool,
        name: &str,
        level: Effective,
        call: &Call,
        dirs: &[String],
        grants: &[RawJson],
        asking: &[(&'static str, String)],
    ) -> Verdict {
        let access = tool.spec().access.clone();
        let targets = tool.targets(call);
        if targets.is_empty() {
            return untargeted(level, name, access, self.sandboxed, asking);
        }
        let cwd = &call.cwd;
        // 工作目录本身也换成真实的位置：头报来的可能是 `~`。
        let cwd = resolve(Path::new(cwd), self.home.as_deref(), cwd)
            .unwrap_or_else(|_| PathBuf::from(cwd));
        // 加进来的目录照工作目录的办法换（施工 5-10 上）：边界表照工作区算。
        let dirs = dirs
            .iter()
            .map(|dir| {
                resolve(Path::new(dir), self.home.as_deref(), dir)
                    .unwrap_or_else(|_| PathBuf::from(dir))
            })
            .collect();
        let places = Places {
            workspace: cwd.clone(),
            dirs,
            shelves: self
                .shelves
                .read()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .iter()
                .chain(&self.attachments)
                .cloned()
                .collect(),
            ..self.places.clone()
        };
        let boundary = Boundary::new(&places);
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
            let zone = boundary.zone(&real);
            // 只碰得到自己工作区的：工作区、加进来的目录以外的一律拒（施工 5-12），读放开只读的技能文件夹除外（施工 X-10 上）。
            if let Some(confined) = &self.confined
                && !confined::inside(&real, &cwd, &places.dirs)
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
                Mark::Forbidden => return deny(self.lettering.guard().forbidden(&target.path)),
                Mark::ReadOnly => return deny(self.lettering.guard().read_only()),
            }
        }
        // 放行过的不问（施工 D-1）：只读、数据根上面已经拦下了，走不到这里。
        asked.retain(|asked| !granted(grants, asked));
        if asked.is_empty() {
            Verdict::Allow
        } else {
            ask(name, access, &asked, self.real_home.as_deref())
        }
    }
}

impl Guard {
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

/// 实际生效的那一级：执行器照同一个算法给命令写沙盒的规格。
pub(crate) fn effective(permission: &Permission) -> Effective {
    if permission.read_only {
        return Effective::ReadOnly;
    }
    match permission.level {
        Level::Full => Effective::Full,
        Level::Workspace => Effective::Workspace,
        Level::Other(_) => Effective::ReadOnly,
    }
}

/// 一条路径照级别判（11 第二节的判法表）：读哪儿都放行，数据根除外（施工 5-4 上，原来边界以外的读要问人）；别的程序的私密
/// 目录读写都问人，完全放开不问，只读时写照拒（施工 5-13）。
fn mark(level: Effective, zone: Zone, write: bool) -> Mark {
    match (zone, level, write) {
        (Zone::Forbidden, _, _) | (Zone::Shelf, _, true) => Mark::Forbidden,
        (_, Effective::Full, _) => Mark::Allow,
        (Zone::Private, Effective::ReadOnly, true) => Mark::ReadOnly,
        (Zone::Private, _, _) => Mark::Ask,
        (_, _, false) => Mark::Allow,
        (_, Effective::ReadOnly, true) => Mark::ReadOnly,
        (Zone::Writable, Effective::Workspace, true) => Mark::Allow,
        (Zone::Readable | Zone::Outside, Effective::Workspace, true) => Mark::Ask,
    }
}

/// 不报路径的调用：执行命令，沙盒能用（`sandboxed`）就工作区、只读都放行，在沙盒里跑；用不了的问人（施工 5-4 上）。
/// 读写放行（查不到路径的执行时自己报错），联网、对外发消息这些还没有的，除了完全放开都问人。问人时的说明并进工具交的
/// `asking`（施工 D-4）。
fn untargeted(
    level: Effective,
    name: &str,
    access: Access,
    sandboxed: bool,
    asking: &[(&'static str, String)],
) -> Verdict {
    let fine = matches!(
        (&access, level),
        (_, Effective::Full) | (Access::Read | Access::Write, _)
    ) || (access == Access::Execute && sandboxed);
    if fine {
        return Verdict::Allow;
    }
    let detail = described(name, &access, asking);
    Verdict::Ask {
        module: module(),
        access,
        rule: None,
        detail: Some(detail),
    }
}

/// 在场所里做的事（访问类别 `venue`，施工 O-31 前，第四条）：场所会话里放行、不问人；本机的会话拒绝。
fn in_venue(place: &Place) -> Verdict {
    match place {
        Place::Venue => Verdict::Allow,
        Place::Local(worded) => deny(worded.clone()),
    }
}

/// 要在沙盒外跑的（施工 D-4，第四条）：拒绝的照拒（数据根这些），完全放开照判的（本来就不套沙盒），只读拒绝，工作区问人、
/// 不提规则（一次放开整个沙盒，不该一劳永逸；本会话放行过的规则只管路径，管不到它）。
fn beyond(
    verdict: Verdict,
    level: Effective,
    name: &str,
    access: Access,
    asking: &[(&'static str, String)],
    texts: &GuardTexts,
) -> Verdict {
    match (verdict, level) {
        (denied @ Verdict::Deny { .. }, _) => denied,
        (verdict, Effective::Full) => verdict,
        (_, Effective::ReadOnly) => deny(texts.read_only()),
        (_, Effective::Workspace) => {
            let detail = described(name, &access, asking);
            Verdict::Ask {
                module: module(),
                access,
                rule: None,
                detail: Some(detail),
            }
        }
    }
}

/// 不报路径的调用问人时的说明（第五条第 4 款，施工 D-4）：工具交的几格，再写上工具名；执行类的写明这一次不在沙盒里跑
/// （走到问人的执行，不是要在沙盒外跑，就是沙盒用不了）。键照字母排。
fn described(name: &str, access: &Access, asking: &[(&'static str, String)]) -> RawJson {
    let mut detail = json!({});
    for (key, value) in asking {
        detail[*key] = json!(value);
    }
    detail["tool"] = json!(name);
    if *access == Access::Execute {
        detail["sandbox"] = json!(false);
    }
    raw(&detail)
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
