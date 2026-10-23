//! `miyu service` 给人看的字（施工 Z-1 下）。路径、系统的原话里的控制字符换成 `�`：它们可能混着终端的控制序列。

use miyu_store::human::clean;

use super::Language;
use crate::service::{Manager, Problem, Said};

impl Language {
    /// 走完了要说的那一句。
    pub(crate) fn service(&self, said: &Said) -> String {
        let chinese = *self == Language::Chinese;
        let shown = |path: &std::path::Path| clean(&path.display().to_string());
        match said {
            Said::Enabled(manager, file) => {
                let when = self.service_when(*manager);
                match chinese {
                    true => format!("装好了：{}，{when}。", shown(file)),
                    false => format!("Installed: {}; {when}.", shown(file)),
                }
            }
            Said::Disabled(_, file) => match chinese {
                true => format!("卸掉了：停了核心的服务，删了 {}。", shown(file)),
                false => format!("Removed: the core's service is stopped and {} deleted.", shown(file)),
            },
            Said::NotEnabled(..) => match chinese {
                true => "没装：miyu service enable 装上。".to_string(),
                false => "Not installed: miyu service enable installs it.".to_string(),
            },
            Said::Status {
                file,
                installed,
                on,
                ..
            } => self.service_status(&shown(file), *installed, *on),
            Said::Cannot(Problem::DataRootOnWindows) => match chinese {
                true => "装不了：Windows 的任务计划带不了 MIYU_HOME，不设它再装。".to_string(),
                false => "Cannot install: a Windows scheduled task cannot carry MIYU_HOME; unset it and try again.".to_string(),
            },
            Said::NoHome => match chinese {
                true => "装不了：不知道家目录在哪（HOME 没设）。".to_string(),
                false => "Cannot install: the home directory is unknown (HOME is not set).".to_string(),
            },
            Said::Failed(step, why) => match chinese {
                true => format!("没成：{}：{}", clean(step), clean(why)),
                false => format!("Failed: {}: {}", clean(step), clean(why)),
            },
        }
    }

    /// 装好以后什么时候起。
    fn service_when(&self, manager: Manager) -> &'static str {
        match (self, manager) {
            (Language::Chinese, Manager::Systemd) => {
                "现在起了，以后登录就起（没人登录也要开着的：sudo loginctl enable-linger）"
            }
            (Language::Chinese, Manager::Autostart) => "下次登录桌面时起",
            (Language::Chinese, Manager::Launchd) => "现在起了，以后登录就起",
            (Language::Chinese, Manager::Task) => "下次登录时起",
            (Language::English, Manager::Systemd) => {
                "started now and at every login (to keep it running with nobody logged in: sudo loginctl enable-linger)"
            }
            (Language::English, Manager::Autostart) => "it starts at the next desktop login",
            (Language::English, Manager::Launchd) => "started now and at every login",
            (Language::English, Manager::Task) => "it starts at the next login",
        }
    }

    /// 看的那一句。
    fn service_status(&self, file: &str, installed: bool, on: Option<bool>) -> String {
        let chinese = *self == Language::Chinese;
        match (installed, on, chinese) {
            (false, _, true) => "没装：miyu service enable 装上。".to_string(),
            (false, _, false) => "Not installed: miyu service enable installs it.".to_string(),
            (true, Some(true), true) => format!("装了，开着：{file}"),
            (true, Some(true), false) => format!("Installed and on: {file}"),
            (true, Some(false), true) => format!("装了，没开着：{file}"),
            (true, Some(false), false) => format!("Installed but not on: {file}"),
            (true, None, true) => format!("装了，登录桌面时起：{file}"),
            (true, None, false) => format!("Installed; starts at desktop login: {file}"),
        }
    }
}
