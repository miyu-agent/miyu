//! `miyu upgrade` 给人看的字（施工 RL-4）：照设计 47 第三节，结果行动词打头、不带句号，能照着做的另起一行写提示，没成的
//! 写成报错。路径、系统的原话里的控制字符换成 `�`：它们可能混着终端的控制序列。

use miyu_store::human::clean;

use super::Language;
use crate::diagnostic::{Level, Problem};
use crate::upgrade::{Channel, NetError, Said, Step};

impl Language {
    /// 走完了要说的几行。
    pub(crate) fn upgrade(&self, said: &Said) -> Vec<String> {
        let zh = *self == Language::Chinese;
        let shown = |path: &std::path::Path| clean(&path.display().to_string());
        let pick = |chinese: String, english: String| if zh { chinese } else { english };
        let problem = match said {
            Said::Upgraded { from, to, left } => {
                let mut lines = vec![
                    pick(format!("已升级 Miyu {from} → {to}"), format!("upgraded Miyu {from} → {to}")),
                    pick(
                        "提示：已经在跑的核心还是旧版：运行 'miyu core stop'，下次打开就是新版".to_string(),
                        "hint: a core that is already running is still the old version: run 'miyu core stop' and the next start uses the new one".to_string(),
                    ),
                ];
                if let Some(left) = left {
                    lines.push(pick(
                        format!("提示：旧版目录 {} 还在用，下次运行 'miyu upgrade' 时删除", shown(left)),
                        format!("hint: the old version in {} is still in use and is removed by the next 'miyu upgrade'", shown(left)),
                    ));
                }
                return lines;
            }
            Said::UpToDate(version) => {
                return vec![pick(
                    format!("已是最新版本 {version}"),
                    format!("already up to date ({version})"),
                )];
            }
            Said::PackageManager(channel) => {
                let (manager, command) = match channel {
                    Channel::Aur => ("AUR", "paru -Syu"),
                    Channel::Brew => ("Homebrew", "brew upgrade"),
                    Channel::Script => ("", ""),
                };
                return vec![
                    pick(
                        format!("这份 Miyu 由 {manager} 安装，用包管理器升级"),
                        format!("this Miyu was installed with {manager}; upgrade it with the package manager"),
                    ),
                    format!("  {command}"),
                ];
            }
            Said::Unmarked(page) => {
                return vec![
                    pick(
                        "这份 Miyu 不是用安装脚本或包管理器装的，不自动升级".to_string(),
                        "this Miyu was not installed by the install script or a package manager, so it does not upgrade itself".to_string(),
                    ),
                    pick(format!("提示：新版在 {page}"), format!("hint: get the new version from {page}")),
                ];
            }
            Said::Cancelled => return vec![pick("已取消".to_string(), "cancelled".to_string())],
            Said::Unconfirmed => {
                pick_problem(zh, ("已取消：没有收到确认", "cancelled: no confirmation"))
                    .hint(pick("加 '--yes' 跳过确认".to_string(), "pass '--yes' to skip the question".to_string()))
            }
            Said::BadMarker(path, why) => Problem::new(pick(
                format!("无法读取安装标记 {}：{}", shown(path), clean(why)),
                format!("cannot read the install marker {}: {}", shown(path), clean(why)),
            )),
            Said::NoKeys => pick_problem(
                zh,
                (
                    "无法升级：这一版没有内置发版公钥，无法核对下载的新版",
                    "cannot upgrade: this build has no release key to verify the download",
                ),
            ),
            Said::UnknownSystem => pick_problem(
                zh,
                ("无法升级：没有这种系统的发行包", "cannot upgrade: no release is built for this system"),
            ),
            Said::Net(NetError::Offline(why)) => Problem::new(pick(
                format!("无法连接 GitHub：{}", clean(why)),
                format!("cannot reach GitHub: {}", clean(why)),
            ))
            .hint(pick("检查网络或代理后重试".to_string(), "check the network or proxy and try again".to_string())),
            Said::Net(NetError::Limited(reset)) => {
                let when = reset
                    .and_then(|reset| jiff::Timestamp::from_second(reset).ok())
                    .map(|at| at.to_zoned(jiff::tz::TimeZone::system()).strftime("%H:%M").to_string());
                let hint = match (&when, zh) {
                    (Some(at), true) => format!("{at} 以后再试"),
                    (Some(at), false) => format!("try again after {at}"),
                    (None, true) => "过一会儿再试".to_string(),
                    (None, false) => "try again later".to_string(),
                };
                pick_problem(
                    zh,
                    (
                        "GitHub 限流：匿名访问每小时 60 次，已经用完",
                        "GitHub rate limit: the 60 anonymous requests per hour are used up",
                    ),
                )
                .hint(hint)
            }
            Said::Net(NetError::Status(status)) => Problem::new(pick(
                format!("GitHub 回应了 HTTP {status}"),
                format!("GitHub answered HTTP {status}"),
            )),
            Said::NoRelease => pick_problem(zh, ("没有找到可用的发行版", "no release found")),
            Said::OnlyPre => pick_problem(zh, ("还没有正式版", "there is no stable release yet"))
                .hint(pick(
                    "加 '--prerelease' 可以看预发布".to_string(),
                    "pass '--prerelease' to include pre-releases".to_string(),
                )),
            Said::MissingAsset(name) => Problem::new(pick(
                format!("发行版里缺少 {}", clean(name)),
                format!("the release is missing {}", clean(name)),
            )),
            Said::BadSignature(why) => Problem::new(pick(
                format!("签名核对失败，不升级：{}", clean(why)),
                format!("the signature does not verify, not upgrading: {}", clean(why)),
            )),
            Said::BadSums(why) => Problem::new(pick(
                format!("校验和清单格式不对，不升级：{}", clean(why)),
                format!("the checksum list is malformed, not upgrading: {}", clean(why)),
            )),
            Said::HashMismatch(name) => Problem::new(pick(
                format!("{} 的 SHA-256 和清单不一致，不升级", clean(name)),
                format!("the SHA-256 of {} does not match the checksum list, not upgrading", clean(name)),
            )),
            Said::BadPackage(missing) => Problem::new(pick(
                format!("新版缺少 {}，不升级", clean(&missing.join(", "))),
                format!("the new version is missing {}, not upgrading", clean(&missing.join(", "))),
            )),
            Said::WrongVersion { expected, said } => Problem::new(pick(
                format!("新版报告的是「{}」，应为「{}」，不升级", clean(said), clean(expected)),
                format!("the new version reports \"{}\" instead of \"{}\", not upgrading", clean(said), clean(expected)),
            )),
            Said::NotWritable(dir) => Problem::new(pick(
                format!("无法升级：没有 {} 的写权限", shown(dir)),
                format!("cannot upgrade: no write permission for {}", shown(dir)),
            ))
            .hint(pick(
                "Miyu 不会提权；装在系统目录里的请用安装它的方式升级".to_string(),
                "Miyu does not raise its privileges; upgrade a system-wide install the way it was installed".to_string(),
            )),
            Said::Failed(step, why) => Problem::new(pick(
                format!("升级失败（{}）：{}，已保留原来的版本", clean(step), clean(why)),
                format!("upgrade failed ({}): {}; the installed version is kept", clean(step), clean(why)),
            )),
        };
        problem.lines(*self, Level::Error)
    }

    /// 问的那一句，后面不换行。
    pub(crate) fn upgrade_question(&self, from: &str, to: &str) -> String {
        match self {
            Language::Chinese => format!("升级 Miyu {from} → {to}？[Y/n] "),
            Language::English => format!("Upgrade Miyu {from} → {to}? [Y/n] "),
        }
    }

    /// 走到哪一步了。
    pub(crate) fn upgrade_step(&self, step: &Step) -> String {
        match (self, step) {
            (Language::Chinese, Step::Downloading(name)) => format!("正在下载 {}", clean(name)),
            (Language::English, Step::Downloading(name)) => format!("downloading {}", clean(name)),
            (Language::Chinese, Step::Checking) => "正在核对新版".to_string(),
            (Language::English, Step::Checking) => "checking the new version".to_string(),
            (Language::Chinese, Step::Replacing) => "正在替换".to_string(),
            (Language::English, Step::Replacing) => "replacing".to_string(),
        }
    }
}

/// 两种语言写死的一句报错。
fn pick_problem(chinese: bool, (zh, en): (&str, &str)) -> Problem {
    Problem::new(if chinese { zh } else { en })
}
