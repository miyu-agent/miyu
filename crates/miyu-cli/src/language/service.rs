//! `miyu service` 给人看的字（施工 Z-1 下；CL-2 下照设计 47 第三节改：结果行动词打头、不带句号，能照着做的另起一行写提示，
//! 没成的写成报错）。路径、系统的原话里的控制字符换成 `�`：它们可能混着终端的控制序列。

use miyu_store::human::clean;

use super::Language;
use crate::diagnostic::{Level, Problem};
use crate::service::{self, Manager, Said};

impl Language {
    /// 走完了要说的几行：成了的是结果行（有的接提示），没成的照报错写。
    pub(crate) fn service(&self, said: &Said) -> Vec<String> {
        let chinese = *self == Language::Chinese;
        let shown = |path: &std::path::Path| clean(&path.display().to_string());
        let failed = match said {
            Said::Enabled(manager, file) => return self.service_enabled(*manager, &shown(file)),
            Said::Disabled(_, file) => {
                return vec![match chinese {
                    true => format!("已停用并删除 {}", shown(file)),
                    false => format!("disabled and removed {}", shown(file)),
                }];
            }
            Said::NotEnabled(..)
            | Said::Status {
                installed: false, ..
            } => return self.service_not_installed(),
            Said::Status { file, on, .. } => return vec![self.service_status(&shown(file), *on)],
            Said::Cannot(service::Problem::DataRootOnWindows) => match chinese {
                true => Problem::new("无法安装服务：Windows 任务计划无法携带 MIYU_HOME")
                    .hint("取消设置 MIYU_HOME 后重试"),
                false => Problem::new(
                    "cannot install the service: a Windows scheduled task cannot carry MIYU_HOME",
                )
                .hint("unset MIYU_HOME and try again"),
            },
            Said::NoHome => match chinese {
                true => Problem::new("无法确定家目录：HOME 未设置"),
                false => Problem::new("cannot find the home directory: HOME is not set"),
            },
            Said::Failed(step, why) => match chinese {
                true => Problem::new(format!("{} 失败：{}", clean(step), clean(why))),
                false => Problem::new(format!("{} failed: {}", clean(step), clean(why))),
            },
        };
        failed.lines(*self, Level::Error)
    }

    /// 装好了：哪种管理器，什么时候起；systemd 的接一行提示，没人登录也要开着怎么办（设计 47 第四节）。
    fn service_enabled(&self, manager: Manager, file: &str) -> Vec<String> {
        let line = match (self, manager) {
            (Language::Chinese, Manager::Systemd | Manager::Launchd) => {
                format!("已启用并启动 {file}")
            }
            (Language::English, Manager::Systemd | Manager::Launchd) => {
                format!("enabled and started {file}")
            }
            (Language::Chinese, Manager::Autostart) => format!("已启用 {file}，下次登录桌面时启动"),
            (Language::English, Manager::Autostart) => {
                format!("enabled {file}, starts at the next desktop login")
            }
            (Language::Chinese, Manager::Task) => format!("已启用 {file}，下次登录时启动"),
            (Language::English, Manager::Task) => {
                format!("enabled {file}, starts at the next login")
            }
        };
        let linger = match self {
            Language::Chinese => "提示：注销后保持运行：sudo loginctl enable-linger $USER",
            Language::English => {
                "hint: keep it running after logout: sudo loginctl enable-linger $USER"
            }
        };
        match manager {
            Manager::Systemd => vec![line, linger.to_string()],
            _ => vec![line],
        }
    }

    /// 没装：怎么装。
    fn service_not_installed(&self) -> Vec<String> {
        let (line, hint) = match self {
            Language::Chinese => ("未安装", "提示：运行 'miyu service enable' 安装"),
            Language::English => (
                "not installed",
                "hint: run 'miyu service enable' to install it",
            ),
        };
        vec![line.to_string(), hint.to_string()]
    }

    /// 装了的看的那一句：`on` 是服务管理器那边开着没有，问不了的（自启动）说登录桌面时起。
    fn service_status(&self, file: &str, on: Option<bool>) -> String {
        match (self, on) {
            (Language::Chinese, Some(true)) => format!("已安装，已启用：{file}"),
            (Language::English, Some(true)) => format!("installed, enabled: {file}"),
            (Language::Chinese, Some(false)) => format!("已安装，未启用：{file}"),
            (Language::English, Some(false)) => format!("installed, not enabled: {file}"),
            (Language::Chinese, None) => format!("已安装，登录桌面时启动：{file}"),
            (Language::English, None) => format!("installed, starts at desktop login: {file}"),
        }
    }
}
