//! `miyu rescue`（施工 RC-1，`docs/blueprint/cli/rescue.md`）：核心坏了时进 LiteMiyu 的应急模式（`litemiyu --rescue`）修。核心
//! 可能正坏着，所以整条路不经过核心：不连、不拉起。
//!
//! 1. 找程序（[`find`]）：`miyu` 被敲的那个路径的同一个目录、`miyu` 真实位置的同一个目录、`PATH`。找不到的照平台说怎么装，
//!    退出码 1，不做别的。
//! 2. 填环境变量（[`filled`]）：照 Miyu 的 `models.chat` 算出的模型（由主程序交进来，`miyu_core::rescue`）填
//!    `LITEMIYU_RESCUE_*` 四个；本来就有的不覆盖，算不出来的、走扩展驱动的（LiteMiyu 认不了，core1 2026-11-13 定）一样都不填、
//!    说一句为什么，照常拉起。key 只走环境变量。
//! 3. 拉起：`litemiyu --rescue <其余参数原样>`，Unix 上换成它，Windows 上起它、等它（同转交软件包的子命令，
//!    [`crate::packages`]）；退出码照它的。

mod find;
#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::io;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;

use miyu_models::direct::Direct;
use miyu_models::provider::Driver;

use crate::diagnostic;
use crate::exit;
use crate::language;

/// 供应商的地址。
pub(crate) const BASE_URL: &str = "LITEMIYU_RESCUE_BASE_URL";
/// 模型名。
pub(crate) const MODEL: &str = "LITEMIYU_RESCUE_MODEL";
/// 接口种类：`openai-chat`、`anthropic`、`openai-responses`。
pub(crate) const API: &str = "LITEMIYU_RESCUE_API";
/// key：LiteMiyu 只认环境变量里的，不进命令行参数（命令行谁都看得到）。
pub(crate) const API_KEY: &str = "LITEMIYU_RESCUE_API_KEY";

/// 填的那四个，照这个先后。
const VARS: [&str; 4] = [BASE_URL, MODEL, API, API_KEY];

/// `miyu rescue` 的参数。给人看的说明在帮助页里（[`crate::help`]）。
#[derive(Debug, Clone, Args)]
pub struct Rescue {
    /// 原样交给 `litemiyu --rescue` 的参数：`-` 开头的也收，`-h`、`--help` 只在第一个时印这一页的帮助。
    #[arg(
        trailing_var_arg = true,
        allow_hyphen_values = true,
        value_name = "ARGS"
    )]
    pub args: Vec<OsString>,
}

/// 跑一次 `miyu rescue`：找到 `litemiyu`，照 `model` 算出的模型填环境变量，换成它（Windows 上等它），交回它的退出码。
/// 找不到的退出码 1；`model` 只在四个变量没有都设好时调。
pub fn rescue(args: Rescue, model: impl FnOnce() -> Result<Direct, String>) -> ExitCode {
    let language = language::current();
    let places = find::Places {
        typed: std::env::args_os().next().map(PathBuf::from),
        exe: std::env::current_exe().ok(),
        path: std::env::var_os("PATH"),
        cwd: std::env::current_dir().ok(),
    };
    let Some(program) = find::find(&places, cfg!(windows)) else {
        let problem = language.rescue_missing(cfg!(target_os = "linux"));
        diagnostic::error(&mut io::stderr(), language, &problem);
        return ExitCode::from(exit::ERROR);
    };
    let (vars, unfilled) = filled(&|name| std::env::var_os(name).is_some(), model);
    if let Some(unfilled) = unfilled {
        let problem = match unfilled {
            Unfilled::Unreadable(why) => language.rescue_unconfigured(&why),
            Unfilled::Extension(driver) => language.rescue_extension(&driver),
        };
        diagnostic::warning(&mut io::stderr(), language, &problem);
    }
    let mut command = std::process::Command::new(program);
    command.arg("--rescue").args(&args.args).envs(vars);
    ExitCode::from(crate::packages::run(command, &mut io::stderr()))
}

/// 一个变量都没填的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Unfilled {
    /// 算不出对话模型：原话。
    Unreadable(String),
    /// 对话模型走扩展驱动，LiteMiyu 认不了：驱动的写法 `<包>/<驱动名>`。
    Extension(String),
}

/// 要设的环境变量，和一样都没填的原因：`given` 说一个变量本来有没有（有的不覆盖）；四个都有的不调 `model`。`model` 算不出来的、
/// 走扩展驱动的一样都不填；算出来的填本来没有的那几个，没有 key 的不填 key。
pub(crate) fn filled(
    given: &dyn Fn(&str) -> bool,
    model: impl FnOnce() -> Result<Direct, String>,
) -> (Vec<(&'static str, String)>, Option<Unfilled>) {
    let missing: Vec<&str> = VARS.into_iter().filter(|name| !given(name)).collect();
    if missing.is_empty() {
        return (Vec::new(), None);
    }
    let direct = match model() {
        Ok(direct) => direct,
        Err(why) => return (Vec::new(), Some(Unfilled::Unreadable(why))),
    };
    let Some(api) = api(&direct.driver) else {
        let driver = direct.driver.as_str().into_owned();
        return (Vec::new(), Some(Unfilled::Extension(driver)));
    };
    let values = [
        (BASE_URL, Some(direct.base_url)),
        (MODEL, Some(direct.model)),
        (API, Some(api.to_string())),
        (API_KEY, direct.key.map(|key| key.expose().to_string())),
    ];
    let vars = values
        .into_iter()
        .filter(|(name, _)| missing.contains(name))
        .filter_map(|(name, value)| Some((name, value?)))
        .collect();
    (vars, None)
}

/// Miyu 的驱动在 LiteMiyu 里叫什么（`LITEMIYU_RESCUE_API`）：内置的三种同名，扩展提供的 LiteMiyu 没有，是空的。
pub(crate) fn api(driver: &Driver) -> Option<&'static str> {
    match driver {
        Driver::OpenAiChat => Some("openai-chat"),
        Driver::Anthropic => Some("anthropic"),
        Driver::OpenAiResponses => Some("openai-responses"),
        Driver::Extension(_) => None,
    }
}
