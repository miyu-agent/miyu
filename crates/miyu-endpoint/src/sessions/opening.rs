//! 造会话时定下的几样（施工 8-32、P-9，从 `sessions.rs` 抽出来）：谁开的、在哪开的，开局的级别，带不带「关于你」。

use miyu_kernel::event::Level;
use miyu_kernel::id::{AccountId, VenueId};
use miyu_policy::Profile;
use miyu_policy::memory::MemoryScope;

use crate::settings::PermissionSettings;
use miyu_session::settings::ProfileSettings;

/// 造会话时要记下的几样：有没有人能确认，是不是一次性的，用哪个模型。
#[derive(Debug, Clone)]
pub(crate) struct Opening {
    /// 属主（施工 O-4 下）：`session.create` 的是连接是谁，`venue.session` 的照对应表、系统账号定。
    pub(crate) owner: AccountId,
    /// 有没有人能确认：头握手时报的。
    pub(crate) attended: bool,
    /// 一次性的：`miyu ask` 开的（施工 3-9 下）。
    pub(crate) oneshot: bool,
    /// 用哪个模型（施工 8-8）：`session.create` 的 `model` 照这时的配置解析好的引用，模型或 `@池`；没写的是空的，照这时的
    /// `models.chat`。
    pub(crate) model: Option<String>,
    /// 场所（施工 O-3）：通讯平台的场所会话写它，本机的是空的（`local`）。
    pub(crate) venue: Option<VenueId>,
    /// 记忆的范围（施工 R-3 下）：`session.create` 的 `memory`；没写的照人格的 `persona.toml`，那也没写的跟着人格。
    pub(crate) memory: Option<MemoryScope>,
    /// 用哪个预设（施工 P-2 上）：`session.create`、`venue.session` 的 `preset`；没写的照这时的 `preset.default`。
    pub(crate) preset: Option<String>,
    /// 群会话（施工 O-13 中）：`venue.session` 的 `kind` 是 `group` 的有，带群聊近况最多几条（施工 O-13 再补）。
    pub(crate) group: Option<miyu_session::GroupSettings>,
    /// 场所会话要不要「关于你」（施工 P-9）：`venue.session` 的 `profile`。本机的不看它，一律要。
    pub(crate) profile: bool,
    /// 场所显示得了图片吗（施工 X-25）：`session.create`、`venue.session` 的 `images`，不写的是显示得了。
    pub(crate) images: bool,
}

/// 新会话开局的级别（施工 8-32）：本机开的照 `permission.level`，场所会话照旧从工作区开局（外部身份另有限制）。
pub(super) fn opening_level(permission: &PermissionSettings, venue: Option<&VenueId>) -> Level {
    match (permission.level.as_str(), venue) {
        ("full", None) => Level::Full,
        _ => Level::Workspace,
    }
}

/// 「关于你」（施工 P-9）：配置里只有管理员的个人设置，属主是管理员本人、又是本机开的或者桥说要的（`asked`，
/// `venue.session` 的 `profile`）才带；别的一律空的（子代理、别人的会话、群聊都不带）。
pub(super) fn profile_for(
    settings: ProfileSettings,
    venue: Option<&VenueId>,
    asked: bool,
    admin: bool,
) -> Profile {
    match admin && (venue.is_none() || asked) {
        true => Profile {
            name: settings.name.unwrap_or_default(),
            about: settings.about.unwrap_or_default(),
            preferred_reply_language: settings.preferred_reply_language.unwrap_or_default(),
        },
        false => Profile::default(),
    }
}
