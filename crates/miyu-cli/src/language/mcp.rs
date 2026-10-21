//! `miyu mcp` 给人看的字（施工 X-42 上，`docs/blueprint/cli/mcp.md`）。

use super::Language;

impl Language {
    /// 印授权页地址前的那一句。
    pub(crate) fn open_to_log_in(&self, service: &str) -> String {
        match self {
            Language::Chinese => format!("在浏览器里打开这个地址登录 {service}："),
            Language::English => format!("Open this address in a browser to log in to {service}:"),
        }
    }

    /// 打不开浏览器。
    pub(crate) fn browser_not_opened(&self) -> &'static str {
        match self {
            Language::Chinese => "打不开浏览器，请自己复制上面的地址。",
            Language::English => "Couldn't open a browser; copy the address above yourself.",
        }
    }

    /// 连上了。
    pub(crate) fn logged_in(&self, service: &str) -> String {
        match self {
            Language::Chinese => format!("登录好了，{service} 连上了。"),
            Language::English => format!("Logged in; {service} is connected."),
        }
    }

    /// 包里带的服务：登录好以后它自己连上。
    pub(crate) fn connects_by_itself(&self) -> &'static str {
        match self {
            Language::Chinese => "登录好以后服务会自己连上。",
            Language::English => "The server connects by itself once you've logged in.",
        }
    }

    /// 等到时还没连上。
    pub(crate) fn login_timed_out(&self, service: &str) -> String {
        match self {
            Language::Chinese => format!("等了 10 分钟，{service} 还没登录好。"),
            Language::English => format!("Waited 10 minutes; {service} still isn't logged in."),
        }
    }
}
