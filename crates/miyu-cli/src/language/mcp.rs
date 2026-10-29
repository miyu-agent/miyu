//! `miyu mcp` 给人看的字（施工 X-42 上，`docs/blueprint/cli/mcp.md`；CL-2 下照设计 47 第三节改：结果行动词打头、不带句号，
//! 报错交 [`Problem`]）。

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// 印授权页地址前的那一句。
    pub(crate) fn open_to_log_in(&self, service: &str) -> String {
        match self {
            Language::Chinese => format!("在浏览器中打开以下地址登录 {service}："),
            Language::English => format!("Open this address in a browser to log in to {service}:"),
        }
    }

    /// 打不开浏览器（设计 47 第四节）。
    pub(crate) fn browser_not_opened(&self) -> Problem {
        match self {
            Language::Chinese => Problem::new("无法打开浏览器，请手动打开上面的地址"),
            Language::English => {
                Problem::new("cannot open a browser; open the address above manually")
            }
        }
    }

    /// 连上了。
    pub(crate) fn logged_in(&self, service: &str) -> String {
        match self {
            Language::Chinese => format!("已登录，{service} 已连接"),
            Language::English => format!("logged in, {service} connected"),
        }
    }

    /// 包里带的服务：登录好以后它自己连上。
    pub(crate) fn connects_by_itself(&self) -> &'static str {
        match self {
            Language::Chinese => "登录完成后服务会自动连接",
            Language::English => "the server connects automatically once logged in",
        }
    }

    /// 浏览器在别的机器上的（施工 X-42 下）：授权完把跳到的地址贴回来。
    pub(crate) fn paste_here(&self) -> &'static str {
        match self {
            Language::Chinese => {
                "浏览器在另一台机器上时，授权后把地址栏中的完整地址粘贴到这里并回车"
            }
            Language::English => {
                "If the browser is on another machine, paste the full address it lands on here after authorizing, then press Enter"
            }
        }
    }

    /// 登出了（施工 X-42 下）。
    pub(crate) fn logged_out(&self, service: &str) -> String {
        match self {
            Language::Chinese => format!("已登出 {service}，再次使用需要重新登录"),
            Language::English => format!("logged out of {service}, log in again to use it"),
        }
    }

    /// 等到时还没连上。
    pub(crate) fn login_timed_out(&self, service: &str) -> Problem {
        match self {
            Language::Chinese => Problem::new(format!("等待 10 分钟后 {service} 仍未登录")),
            Language::English => {
                Problem::new(format!("{service} still not logged in after 10 minutes"))
            }
        }
    }
}
