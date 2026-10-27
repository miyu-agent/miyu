//! 握手（`docs/designs/04-核心协议.md` 第三节、第八节，第九节「先做的几样怎么写」）：协议的主版本取双方
//! 都支持的最高的；本机连接出示本机令牌（第四节）。握手以后这个连接就是管理员（`06-多用户与身份.md`
//! 第二节）。
//!
//! 施工 W-8 起凭据正好写一种（`web-module.md`「怎么走」第一条）：本机令牌 `token`、一次性码 `code`、登录令牌 `login`、
//! 交接票 `ticket`（施工 LT-1），或者用户名 `user` 加密码 `password`。用一次性码的回应多 `"setup": true`，用密码、交接票的多一个
//! 登录令牌 `login`。写了不止一种、用户名和密码只写了一个：`bad_params`，回完断开。
//!
//! 回应带这个连接给人看的字用哪种语言 `language`，和配置里有几处错误 `config_errors`（施工 8-2，`config.md`
//! 「协议」）：语言照 `ui.language` 的最终值，是 `auto` 的照头报的系统语言 `locale`。
//!
//! 施工 8-4 起 `ui.language` 能当场改：连接记着头报的系统语言（[`Shaken`]），每次说话都照这时的 `ui.language` 重算
//! （[`Shaken::now`]），不用再握手（`config.md` 第二条第 8 条）。
//!
//! 回应带 `host`（施工 W-3，`web-module.md`「四、路径」）：系统的家目录（照原样）、核心所在的平台、这个账号的
//! 工作区（换成真实的位置）。管理员的工作区核心起来时就建好了（`core.md`），这里不再建：握手不该替每一个连上来
//! 的头造目录，换不成真实的位置（还没建出来）的就照原样交回。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_sandbox::Availability;

use std::sync::Arc;

use crate::Core;
use crate::config::Config;
use crate::login::{self, Via};
use crate::refusal::{Locale, Refusal};
use crate::settings::UiSettings;

/// 核心支持的协议主版本：现在只有 1。
/// 协议 1 冻结了（施工 Y-5 末下）：方法在 `docs/reference/methods-v1.txt`，只加不改不删，门禁照它查（`xtask/src/frozen.rs`）。
/// 删、改名、改意思的才升到 2。
pub(crate) const PROTOCOL: u32 = 1;

/// `hello` 的参数。认识的几格，别的不理（同一个主版本之内只做加法，第八节）。
#[derive(Debug, Deserialize)]
struct Params {
    /// 头支持的主版本范围：`[最低, 最高]`。
    protocol: [u32; 2],
    /// 头的种类和版本。
    head: Head,
    /// 头所在系统的语言：`ui.language` 是 `auto` 时照它（施工 8-2）。
    #[serde(default)]
    locale: Option<String>,
    /// 头能做什么。
    #[serde(default)]
    caps: Caps,
    /// 本机令牌。
    #[serde(default)]
    token: Option<String>,
    /// 一次性码（施工 W-8）。
    #[serde(default)]
    code: Option<String>,
    /// 登录令牌（施工 W-8）。
    #[serde(default)]
    login: Option<String>,
    /// 邀请码（施工 Y-1 上）：只能注册。
    #[serde(default)]
    invite: Option<String>,
    /// 交接票（施工 LT-1）：`login.ticket` 要的，待遇同密码。
    #[serde(default)]
    ticket: Option<String>,
    /// 用户名（施工 W-8）。
    #[serde(default)]
    user: Option<String>,
    /// 密码（施工 W-8）。
    #[serde(default)]
    password: Option<String>,
}

/// 头的种类和版本。
#[derive(Debug, Deserialize)]
struct Head {
    kind: String,
    version: String,
}

/// 头能做什么：能不能让人输入，认不认识管理员模式。
#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Caps {
    /// 能让人输入：确认、提问有人答。
    input: bool,
    /// 认识管理员模式（施工 Y-12 下）：改系统那一层的请求带 `"scope": "system"`。没声明的老头照以前的意思。
    scope: bool,
}

/// 握手时记下的：头报的系统语言照 `auto` 算出的那一种，能不能让人输入，替谁干活。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Shaken {
    /// `ui.language` 是 `auto` 时用哪一种：头报的 `locale` 照 [`UiSettings::language_for`] 算的。
    system: &'static str,
    /// 能让人输入。
    input: bool,
    /// 认识管理员模式（施工 Y-12 下）。
    scope: bool,
    /// 握手时替谁干活：给人看的字照他那一份配置的 `ui.language`（施工 Y-2 下下末，以前一律照机主的）。
    whose: AccountId,
}

impl Shaken {
    /// 这一刻的这个连接：语言照现在的 `ui.language` 重算。只拿语言、不装包的地方用它，`admin` 是假的；处理请求用 [`Shaken::now_as`]。
    pub(crate) fn now(&self, core: &Core) -> Peer {
        let language = language_of(&core.mine(&self.whose).config(), self.system);
        Peer {
            locale: Locale::of(Some(language)),
            language,
            input: self.input,
            admin: false,
            scoped: self.scope,
        }
    }

    /// 同 [`Shaken::now`]，`admin` 照这一刻替 `acting_for` 干活的人是不是管理员算（施工 Y-5 下下）：机主只比账号，不读组表。
    pub(crate) fn now_as(&self, core: &Core, acting_for: &AccountId) -> Peer {
        Peer {
            admin: crate::login::admins::is_admin(core, acting_for),
            ..self.now(core)
        }
    }

    /// `ui.language` 是 `auto` 时用哪一种。
    pub(crate) fn system(&self) -> &'static str {
        self.system
    }

    /// 握手时替谁干活的那个人（施工 IM-1 中下四：扩展的状态里每人一份的包照他那一份报）。
    pub(crate) fn whose(&self) -> &AccountId {
        &self.whose
    }

    /// 这一刻的这个连接，`admin` 照握手时替谁干活的那个人算（施工 U23-R修二：订列表的回应、推送照它裁格子）。
    pub(crate) fn now_whose(&self, core: &Core) -> Peer {
        self.now_as(core, &self.whose)
    }

    /// 握手时声明了认识管理员模式（施工 Y-12 下）。
    pub(crate) fn scoped(&self) -> bool {
        self.scope
    }
}

/// 照配置 `config` 的 `ui.language`（不算项目配置），`auto` 的用 `system`：给人看的字用哪种语言。
pub(crate) fn language_of(config: &Config, system: &'static str) -> &'static str {
    let ui = UiSettings::from(&config.resolved().values());
    match ui.language_for(Some(system)) {
        "zh" => "zh",
        "ja" => "ja",
        _ => "en",
    }
}

/// 这个连接是谁（施工 O-4 下、O-13 上）：账号，和核心拉起的扩展是哪个包（本机的头没有）。
#[derive(Debug, Clone)]
pub(crate) struct Caller {
    /// 这个连接是谁（身份，施工 Y-5 上）：声明了系统账号的包的扩展是那个系统账号，别的是管理员。凭据、吊销、成员闸、
    /// 上传和附件「谁交的」照它；资源归谁、会话属主照 [`Caller::acting_for`]。
    pub(crate) account: AccountId,
    /// 替谁干活，和身份不是同一个的才有（施工 Y-5 中上）：没声明系统账号的包替机主。核心拉起的连接不登录、不换账号，所以这一格
    /// 不会和 `account` 对不上。
    pub(crate) serving: Option<AccountId>,
    /// 核心拉起的扩展是哪个包：`events.append` 的 `ext.<包>.` 照它查、记成这个模块写的。
    pub(crate) package: Option<String>,
    /// 核心往这个连接发请求的一头（施工 O-2 上）：`provide` 照它记下这个包由谁提供。
    pub(crate) reverse: crate::reverse::Peer,
}

impl Caller {
    /// 这个连接替谁干活（施工 Y-5 上，设计 06 U16、U17）：人格、预设、知识库、技能、脚本、场所、终端会话、用量、造会话的
    /// 属主、项目、订阅都照它。没声明系统账号的包替机主（施工 Y-5 中上），别的和身份是同一个。
    pub(crate) fn acting_for(&self) -> &AccountId {
        self.serving.as_ref().unwrap_or(&self.account)
    }
}

/// 这一刻的这个连接。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Peer {
    /// 核心拒绝时的话用哪种语言：只有中文、英文，`ja` 的照英文。
    pub(crate) locale: Locale,
    /// 给人看的字用哪种语言：`zh`、`en`、`ja` 之一（施工 8-2）。配置的名字、说明、报错的话照它。
    pub(crate) language: &'static str,
    /// 能让人输入：它造的会话有人确认。
    pub(crate) input: bool,
    /// 这一刻是不是管理员（施工 Y-5 下下）：成员不能装包、卸包、启用扩展（`packages/may_manage.rs`）。
    pub(crate) admin: bool,
    /// 握手时声明了认识管理员模式（施工 Y-12 下）。
    pub(crate) scoped: bool,
}

impl Peer {
    /// 这一个请求 `request` 是不是在管理员模式里（施工 Y-12 下）：管理员带了 `"scope": "system"` 的；没声明认识管理员模式的
    /// 老头，管理员的请求照以前的意思都算。成员的一律不算。
    pub(crate) fn admin_mode(self, request: &crate::wire::Request) -> bool {
        self.admin && (request.system || !self.scoped)
    }
}

/// 握手：交回这个连接记下的、它是怎么认出来的（施工 W-8）和回应。拒绝的，交回拒绝和要不要断开。`spawned` 的是核心亲手拉起的
/// 扩展（施工 9-4 上）：不看凭据，写了的也不看。`account` 是这个连接是谁（施工 O-4 下：声明了系统账号的包的扩展是它）。
pub(crate) async fn hello(
    core: &Arc<Core>,
    params: Value,
    spawned: bool,
    account: &AccountId,
) -> Result<(Shaken, Via, Value), (Refusal, bool)> {
    let params: Params =
        serde_json::from_value(params).map_err(|_| (Refusal::BAD_PARAMS, false))?;
    let [low, high] = params.protocol;
    if !(low <= PROTOCOL && PROTOCOL <= high) {
        tracing::warn!(
            target: "miyu::endpoint",
            head = params.head.kind.as_str(),
            low,
            high,
            "protocol mismatch"
        );
        return Err((Refusal::PROTOCOL, true));
    }
    let head = params.head.kind.as_str();
    let checked = match spawned {
        true => Ok((Via::Spawned, None)),
        false => credentials(core, &params).await,
    };
    let (via, login) = checked.map_err(|(refusal, said)| {
        if let Some(said) = said {
            tracing::warn!(target: "miyu::endpoint", head, "{said}");
        }
        (refusal, true)
    })?;
    tracing::info!(
        target: "miyu::endpoint",
        head,
        version = params.head.version.as_str(),
        protocol = PROTOCOL,
        via = via.label(),
        "connected"
    );
    // 凭据认出来的账号（施工 Y-1 上）：登录令牌、密码连上的是那个账号；本机令牌、扩展的照连接原来的。
    let account = via.account().unwrap_or(account);
    let shaken = Shaken {
        system: system(params.locale.as_deref()),
        input: params.caps.input,
        scope: params.caps.scope,
        whose: account.clone(),
    };
    let language = shaken.now(core).language;
    let mut result = json!({
        "protocol": PROTOCOL,
        "core": {"version": env!("CARGO_PKG_VERSION")},
        "account": account.as_str(),
        // 是不是管理员（施工 Y-2 下下中上）：头照它决定给不给设置页、账号管理这些，不用拿请求去试。
        "admin": crate::login::admins::is_admin(core, account),
        "host": host(core, account),
        "sandbox": sandbox(&core.sandbox),
        "language": language,
        // 视图投影的版本（施工 9-8 中，`view.md`「握手」）：头照它决定用条目还是自己算。
        "view": miyu_view::VERSION,
    });
    let errors = core.config().errors();
    if errors > 0 {
        result["config_errors"] = json!(errors);
    }
    if via == Via::Code || core.setting_up() {
        result["setup"] = json!(true);
    }
    if core.setting_up() && via == Via::Token {
        // 还没有机主（施工 Y-12 中上）：注册成了才是谁，同邀请码。
        result["account"] = Value::Null;
        result["admin"] = json!(false);
    }
    if matches!(via, Via::Invite(_)) {
        result["admin"] = json!(false);
        // 还没注册，不是谁（施工 Y-1 上）：注册成了回应里才有账号。
        result["account"] = Value::Null;
        result["register"] = json!(true);
    }
    if let Some(login) = login {
        result["login"] = login;
    }
    Ok((shaken, via, result))
}

/// 照写的凭据认这个连接（施工 W-8）：交回它是怎么认出来的、用密码登录的另交登录令牌。不认的交回拒绝和运行日志里记哪一句
/// （`bad_params` 不记）。
async fn credentials(
    core: &Arc<Core>,
    params: &Params,
) -> Result<(Via, Option<Value>), (Refusal, Option<&'static str>)> {
    let written = [
        params.token.is_some(),
        params.code.is_some(),
        params.login.is_some(),
        params.invite.is_some(),
        params.ticket.is_some(),
        params.user.is_some() || params.password.is_some(),
    ];
    if written.iter().filter(|written| **written).count() > 1 {
        return Err((Refusal::BAD_PARAMS, None));
    }
    if let Some(token) = &params.token {
        return match same(token, &core.token) {
            true => Ok((Via::Token, None)),
            false => Err((Refusal::BAD_TOKEN, Some("bad token"))),
        };
    }
    if let Some(code) = &params.code {
        return match core.identity.take_code(code) {
            true => Ok((Via::Code, None)),
            false => Err((Refusal::BAD_CODE, Some("bad code"))),
        };
    }
    if let Some(token) = &params.login {
        return login::login(core, token)
            .map(|via| (via, None))
            .ok_or((Refusal::BAD_LOGIN, Some("bad login")));
    }
    // 交接票（施工 LT-1）：同密码，对上了当场作废、回应带新的登录令牌。
    if let Some(ticket) = &params.ticket {
        return login::ticket::handshake(core, ticket)
            .await
            .map(|(issued, via)| (via, Some(issued)))
            .map_err(|refusal| (refusal, Some("bad ticket")));
    }
    if let Some(code) = &params.invite {
        return login::invites::admit(core, code)
            .map(|via| (via, None))
            .ok_or((Refusal::BAD_INVITE, Some("bad invite")));
    }
    match (&params.user, &params.password) {
        (None, None) => Err((Refusal::BAD_TOKEN, Some("bad token"))),
        (Some(user), Some(password)) => login::password(core, user, password.clone())
            .await
            .map(|(issued, via)| (via, Some(issued)))
            .map_err(|refusal| {
                let said = match refusal.reason {
                    "login_throttled" => Some("login throttled"),
                    "bad_password" => Some("bad password"),
                    "account_disabled" => Some("account disabled"),
                    _ => None,
                };
                (refusal, said)
            }),
        _ => Err((Refusal::BAD_PARAMS, None)),
    }
}

/// 头报的系统语言 `locale` 照 `auto` 算出的那一种（`config.md` 第二条第 8 条）：`zh` 开头的是 `zh`，`ja` 开头的是 `ja`，
/// 别的、没报的是 `en`。
fn system(locale: Option<&str>) -> &'static str {
    let auto = UiSettings {
        language: "auto".to_string(),
        startup: "new".to_string(),
        head: "tui".to_string(),
        welcomed: false,
    };
    match auto.language_for(locale) {
        "zh" => "zh",
        "ja" => "ja",
        _ => "en",
    }
}

/// 握手的回应里的 `host`（施工 W-3，`web-module.md`「四、路径」）：`home` 是核心起来时拿到的系统的家目录，照
/// 原样，读不出来的是 `null`；`platform` 是核心所在的平台；`workspace` 是这个账号的工作区，换成真实的位置。不在
/// 这里建它：管理员、系统账号的工作区核心起来时就建好了（`core.md`，施工 O-4 下），握手不该替每一个连上来的头造目录；换不成
/// 的（没建出来）照原样交回，工具自己用到时会报错。管理员的另带 `dirs`：照 XDG 的那几处，同 `workspace` 的给法（施工 Y-11 上）。
fn host(core: &Core, account: &AccountId) -> Value {
    // 在的照真实位置，还没建的照原样：握手不替人建目录。
    let shown = |dir: std::path::PathBuf| {
        std::fs::canonicalize(&dir)
            .unwrap_or(dir)
            .display()
            .to_string()
    };
    let mut host = json!({
        "home": core.home.as_deref().map(|home| home.display().to_string()),
        "platform": std::env::consts::OS,
        "workspace": shown(core.root.workspace(account)),
    });
    // 照 XDG 的那几处（施工 Y-11 上）：只给管理员（`06-多用户与身份.md` U25：普通用户看不到运行它的那台电脑，真实路径就是那台
    // 电脑的东西）；远程的连整个 `host` 都不给（`connection/reach.rs`）。
    if *account == core.admin {
        let dirs: serde_json::Map<String, Value> = core
            .root
            .user_dirs(account)
            .into_iter()
            .map(|(name, dir)| (name.to_string(), json!(shown(dir))))
            .collect();
        host["dirs"] = Value::Object(dirs);
    }
    host
}

/// 握手的回应里的 `sandbox`：能用的 `{"usable": true}`，用不了的带原因（施工 5-4 下）。
fn sandbox(availability: &Availability) -> Value {
    match availability {
        Availability::Usable(_) => json!({ "usable": true }),
        Availability::Unusable(reason) => json!({ "usable": false, "reason": reason.code() }),
    }
}

/// 两份令牌一样不一样：每个字节都比，比到哪一个不一样都用一样长的时间。本机令牌、一次性码都照它比（`login.rs`）。
pub(crate) fn same(given: &str, expected: &str) -> bool {
    given.len() == expected.len()
        && given
            .bytes()
            .zip(expected.bytes())
            .fold(0u8, |differ, (a, b)| differ | (a ^ b))
            == 0
}

#[cfg(test)]
mod tests;
