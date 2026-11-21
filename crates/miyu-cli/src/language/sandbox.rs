//! 沙盒用不了时给人看的字（施工 5-4 下，设计 47）。原因里的控制字符换成 `�`：它们可能混着终端的控制序列。

use miyu_sandbox::{Platform, Unusable};
use miyu_store::human::clean;

use super::Language;
use crate::diagnostic::Problem;

impl Language {
    /// `miyu ask` 开头，沙盒用不了的告警（施工 5-4 下，设计 47）：原因 `reason` 是协议上的写法，照这台机器的系统 `platform`
    /// 写成人话，能修的另起一行提示；不认得的原因照原样写。
    pub(crate) fn unsandboxed(&self, reason: &str, platform: Platform) -> Problem {
        let (why, fix) = match Unusable::from_code(reason) {
            Some(known) => self.why_unsandboxed(known, platform),
            None => (clean(reason), None),
        };
        let problem = match self {
            Language::Chinese => Problem::new(format!("沙盒不可用：{why}")),
            Language::English => Problem::new(format!("sandbox unavailable: {why}")),
        };
        match fix {
            Some(fix) => problem.hint(fix),
            None => problem,
        }
    }

    /// 沙盒用不了的原因和怎么修，写成人话。
    fn why_unsandboxed(
        &self,
        reason: Unusable,
        platform: Platform,
    ) -> (String, Option<&'static str>) {
        let chinese = *self == Language::Chinese;
        let (why, fix) = match (reason, platform, chinese) {
            (Unusable::HelperMissing, _, true) => {
                ("主程序旁边没有 miyu-sandbox", Some("重新安装 Miyu"))
            }
            (Unusable::HelperMissing, _, false) => (
                "miyu-sandbox is missing beside the main program",
                Some("reinstall Miyu"),
            ),
            (Unusable::HelperFailed, _, true) => ("miyu-sandbox 无法运行", Some("重新安装 Miyu")),
            (Unusable::HelperFailed, _, false) => {
                ("miyu-sandbox does not run", Some("reinstall Miyu"))
            }
            (Unusable::NoMechanism, Platform::Linux, true) => (
                "内核没有可用的 Landlock",
                Some("需要 Linux 5.13 或更高版本，并在启动参数 lsm= 里启用 Landlock"),
            ),
            (Unusable::NoMechanism, Platform::Linux, false) => (
                "the kernel has no usable Landlock",
                Some("use Linux 5.13 or later with Landlock enabled in the lsm= boot parameter"),
            ),
            (Unusable::NoMechanism, Platform::Macos, true) => {
                ("无法应用 Seatbelt 配置，Miyu 可能运行在另一个沙盒里", None)
            }
            (Unusable::NoMechanism, Platform::Macos, false) => (
                "the Seatbelt profile cannot be applied; Miyu may be running inside another sandbox",
                None,
            ),
            // Windows 上的沙盒是 MXC 的基础容器（施工 5-9 改上）：探不出基础容器的机器手段是空的（施工 WN-1 改：原来说「此版本还
            // 不能」，和现状对不上）。
            (Unusable::NoMechanism, Platform::Windows, true) => {
                ("这台 Windows 无法启动 MXC 的基础容器", None)
            }
            (Unusable::NoMechanism, Platform::Windows, false) => {
                ("this Windows cannot start an MXC base container", None)
            }
            (Unusable::NoMechanism, Platform::Other, true) => ("这个系统上没有可用的沙盒", None),
            (Unusable::NoMechanism, Platform::Other, false) => {
                ("no sandbox is available on this system", None)
            }
        };
        (why.to_string(), fix)
    }
}
