//! 只碰得到自己工作区的会话（施工 5-12，`11-权限与沙盒.md` 第三节「外部身份」，2026-10-11 项目主人定）：场所会话、属主不是
//! 管理员的（群会话、陌生人的私聊、成员的私聊）。文件类工具要碰的每一条都要在这一轮的工作区（或者加进来的目录）里，越过的一律
//! 拒（这些会话没人能确认，A11）；命令要等沙盒把读也关进工作区（5-12 下）才放开，现在一律拒。管理员本人的私聊不限。
//!
//! 拒绝时的两句不进策略快照，这种会话造、载入时读（同 `not-in-venue.txt`）。
//!
//! 属主不是管理员的本机会话（成员的，施工 Y-2 下①，设计 06 八再又补第 2 条「普通用户看不到运行它的那台电脑」）也关进来，而且
//! 只到属主自己的几处（[`Own`]：他的工作区、家目录里照 XDG 的那几处），不跟着会话的工作目录、加进来的目录走：那两样是他自己报的，
//! 报成 `/` 也不能把整台电脑算成他的。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use miyu_fs::{resolve, within};
use miyu_kernel::event::Said;
use miyu_kernel::id::AccountId;
use miyu_kernel::template::{Template, TemplateError};
use miyu_kernel::tool::Worded;
use miyu_store::root::DataRoot;

/// 成员的会话只到的几处（施工 Y-2 下①）：属主的工作区、家目录里照 XDG 的那几处（施工 Y-11 上），真实位置；还没建的照最近的、
/// 已经存在的上级换（同边界表的 `own`）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Own(Vec<PathBuf>);

impl Own {
    /// 账号 `owner` 的几处。碰磁盘（换真实位置）。
    pub(crate) fn of(root: &DataRoot, owner: &AccountId) -> Own {
        let places = std::iter::once(root.workspace(owner))
            .chain(root.user_dirs(owner).into_iter().map(|(_, dir)| dir));
        Own(places
            .filter_map(|path| resolve(&path, None, &path.to_string_lossy()).ok())
            .collect())
    }

    /// 真实位置 `real` 在这几处里。
    pub(crate) fn holds(&self, real: &Path) -> bool {
        self.0.iter().any(|dir| within(real, dir))
    }

    /// 这几处里已经在的：沙盒照它们放开（Landlock 认不得不在的路径）。碰磁盘。
    pub(crate) fn existing(&self) -> Vec<PathBuf> {
        self.0.iter().filter(|dir| dir.is_dir()).cloned().collect()
    }
}

/// 关起来的会话的命令沙盒关到哪（施工 5-12 下、Y-2 下①）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Reach {
    /// 场所会话：这一轮的工作区、加进来的目录。
    Workspace,
    /// 成员的会话：只到他自己的几处，工作目录不在里面的连命令都不跑（权限策略先拒）。
    Own(Own),
}

/// 拒绝时写给她的两句。
pub(crate) struct Confined {
    /// 要碰的路径在工作区外面（`core/permissions/outside-workspace.txt`），字段 `path`。
    outside: Template,
    /// 这里还不能跑命令（`core/permissions/no-commands.txt`）。
    no_commands: String,
    /// 成员的会话只到的几处（施工 Y-2 下①）；场所会话没有，照这一轮的工作区、加进来的目录。
    own: Option<Own>,
}

impl Confined {
    /// 照出厂的两句造（[`miyu_store::resources::ResourceRoot::confined_texts`]）。
    ///
    /// # Errors
    ///
    /// 模板的写法坏了，或者要了 `path` 以外的字段。
    pub(crate) fn new(outside: &str, no_commands: String) -> Result<Confined, TemplateError> {
        let outside = Template::parse(outside)?;
        outside.render(&BTreeMap::from([("path", "")]))?;
        Ok(Confined {
            outside,
            no_commands,
            own: None,
        })
    }

    /// 同一份，只到 `own` 那几处（施工 Y-2 下①，成员的会话）。
    #[must_use]
    pub(crate) fn only(mut self, own: Own) -> Confined {
        self.own = Some(own);
        self
    }

    /// 真实位置 `real` 碰得到：成员的会话照他自己的几处；别的照工作区 `workspace`、加进来的目录 `dirs`（都是真实位置）。
    pub(crate) fn inside(&self, real: &Path, workspace: &Path, dirs: &[PathBuf]) -> bool {
        match &self.own {
            Some(own) => own.holds(real),
            None => within(real, workspace) || dirs.iter().any(|dir| within(real, dir)),
        }
    }

    /// 成员的会话只到的几处；场所会话的没有。
    pub(crate) fn own(&self) -> Option<&Own> {
        self.own.as_ref()
    }

    /// 命令的沙盒关到哪。
    pub(crate) fn reach(&self) -> Reach {
        match &self.own {
            Some(own) => Reach::Own(own.clone()),
            None => Reach::Workspace,
        }
    }

    /// `path` 在工作区外面。说法是 `core/permissions/outside-workspace`。
    pub(crate) fn outside(&self, path: &str) -> Worded {
        let text = self
            .outside
            .render(&BTreeMap::from([("path", path)]))
            .unwrap_or_default();
        Worded {
            text,
            said: Some(Said::new("core/permissions/outside-workspace").with("path", path)),
        }
    }

    /// 这里还不能跑命令。说法是 `core/permissions/no-commands`。
    pub(crate) fn no_commands(&self) -> Worded {
        Worded {
            text: self.no_commands.clone(),
            said: Some(Said::new("core/permissions/no-commands")),
        }
    }
}
