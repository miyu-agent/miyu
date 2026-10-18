//! `config.schema` 里选项的两样另说的（施工 R-5 再补、三补，`docs/blueprint/config.md`「协议」）：后面暗字写什么（`note`）、现在
//! 用不用得了（`available`）。只有核心查得出的才有，不进给人看的字；现在只有语义模型的「内置模型」。

use miyu_session::EMBED_LOCAL;

/// 语义模型那一项的键（`miyu_models::settings::UseSettings` 的 `embedding`）。
const EMBEDDING: &str = "models.embedding";

/// 选项后面暗字写的（施工 R-5 再补，`config.md`「协议」）：只有核心查得出的才有，不进给人看的字。现在只有语义模型
/// （`UseSettings` 的 `embedding`）的「内置模型」：本机清单的模型名 `local`，本机的那一路用不上的没有。
pub(super) fn note(key: &str, option: &str, local: Option<&str>) -> Option<String> {
    match (key, option) {
        (EMBEDDING, EMBED_LOCAL) => local.map(str::to_string),
        _ => None,
    }
}

/// 选项现在用不用得了（施工 R-5 三补，`config.md`「协议」）：只有核心查得出用不了的才说用不了。现在只有语义模型的「内置模型」：
/// 本机的那一路没有（没装内置模型那个小程序包、程序不在、模型清单读不了）的用不了，头画成灰的、选不了。
pub(super) fn usable(key: &str, option: &str, local: bool) -> bool {
    !matches!((key, option), (EMBEDDING, EMBED_LOCAL)) || local
}

/// 选项后面暗字写的：核心查得出的（[`note`]），没有的照内置包说的这一刻的状态（[`state`]）。
pub(super) fn noted(
    core: &crate::Core,
    (words, values): (&miyu_store::human::Human, &miyu_config::Values),
    key: &str,
    option: &str,
    local: Option<&str>,
) -> Option<String> {
    note(key, option, local).or_else(|| state(core, (words, values), key, option))
}

/// 选项后面暗字写的另一种（施工 N-2 下、N-3 上）：内置包照这个账号的最终值 `values` 说的这一刻的状态
/// （`Builtins::option_state`），照连接的语言 `words`、核心这台机器的时区拼成一句，例如搜索来源的「冷却到 15:30」「要 key」。
/// 配置的锁由调的一方拿着（`config.schema`），这里不再拿。没装内置包端口的核心（测试里造的）、没有要说的没有。
fn state(
    core: &crate::Core,
    (words, values): (&miyu_store::human::Human, &miyu_config::Values),
    key: &str,
    option: &str,
) -> Option<String> {
    use crate::builtins::OptionState;
    use miyu_kernel::event::Said;
    let said = match core.builtins.as_ref()?.option_state(key, option, values)? {
        OptionState::CoolingUntil(until) => {
            let ms = until
                .duration_since(std::time::UNIX_EPOCH)
                .ok()?
                .as_millis();
            let at = miyu_kernel::time::Timestamp::from_unix_millis(i64::try_from(ms).ok()?)?;
            let clock = at.local_clock(crate::sessions::offset());
            Said::new("core/config/option-cooling").with("time", clock)
        }
        OptionState::NeedsKey => Said::new("core/config/option-needs-key"),
        OptionState::NeedsAddress => Said::new("core/config/option-needs-address"),
    };
    words.say(&said)
}
