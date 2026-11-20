//! `shell` 照命令的字面认高危（施工 5-17，`miyu_policy::hazard`，`internal/blueprint/tools/shell.md`「高危」；施工 HZ-1 从 `shell.rs`
//! 挪出来）：报给权限策略，不进 `run`。灭顶的交拒的那一句；问人的每一种连同归的类全交，关没关掉由权限策略照这一轮的配置判。

use std::path::Path;

use miyu_kernel::tool::Worded;
use miyu_policy::hazard::{Assessed, Group, Kind, Places, assess, assess_powershell};
use miyu_tool::{Call, Hazard, HazardAsk};

use super::{Args, Shell};
use crate::common::said;
use crate::load::say;

impl Shell {
    /// 这次调用认出来的高危：家目录、数据根照这次调用带的，工作目录照家目录换成绝对路径。bash、zsh 的照 POSIX 的写法认，
    /// PowerShell 的照它自己的写法认（施工 WN-1）。
    pub(super) fn assessed(&self, call: &Call) -> Option<Hazard> {
        let args = serde_json::from_str::<Args>(&call.args).ok()?;
        let home = call
            .home
            .as_deref()
            .map(|home| home.to_string_lossy().into_owned());
        let cwd = miyu_fs::resolve(Path::new("/"), call.home.as_deref(), &call.cwd).map_or_else(
            |_| call.cwd.clone(),
            |cwd| cwd.to_string_lossy().into_owned(),
        );
        let data_root = call
            .data_root
            .as_deref()
            .map(|root| root.to_string_lossy().into_owned())
            .unwrap_or_default();
        let places = Places {
            cwd: &cwd,
            home: home.as_deref(),
            data_root: &data_root,
        };
        let assessed = match self.program.kind.posix() {
            true => assess(&args.command, &places),
            false => assess_powershell(&args.command, &places),
        };
        let asked = match assessed? {
            Assessed::Refused(kind) => return Some(Hazard::Refused(self.refusal(kind))),
            Assessed::Asks(kinds) => kinds,
        };
        // 问人的每一种连同归的类（施工 HZ-1）：关没关掉由权限策略照这一轮的配置判。
        let ask = |kind: Kind| HazardAsk {
            code: kind.code(),
            class: kind.group().map(Group::code),
        };
        Some(Hazard::Asks(asked.into_iter().map(ask).collect()))
    }

    /// 灭顶的 `kind` 拒的时候写给她的一句，连同给人看的说法 `shell/hazard-<种类>`。
    ///
    /// # Panics
    ///
    /// 实际不会：读资源时直接拒的每一种都读了它那一句（[`Shell::load`]）。
    fn refusal(&self, kind: Kind) -> Worded {
        let (_, reason) = self
            .texts
            .reasons
            .iter()
            .find(|(known, _)| *known == kind)
            .expect("直接拒的每一种都读了它那一句");
        Worded {
            text: say(
                &self.texts.hazard,
                &[("reason", say(reason, &[]).trim_end())],
            ),
            said: Some(said(&format!("shell/hazard-{}", kind.code()))),
        }
    }
}
