//! 这一轮发给谁（施工 E-M，2026-10-22 项目主人定）：供应商的显示名、模型、思考强度，随限额交给内核，写进模型那一块事实；
//! 预设由 actor 交之前补上（路由不知道预设）。

use miyu_config::Value;
use miyu_kernel::facts::Identity;
use miyu_models::facts::Fact;
use miyu_models::provider::Target;

use crate::config::TurnConfig;

/// 思考强度没配的写法。
const DEFAULT: &str = "default";

/// 发给 `target` 的这一次：供应商名照 `providers.<编号>.name`，没写的是编号；档位照这个模型配的，没配的 `default`。
pub(super) fn identity(
    config: &TurnConfig,
    target: &Target,
    effort: &Fact<Option<String>>,
) -> Identity {
    let values = config.resolved.values();
    let key = format!("providers.{}.name", target.provider.id);
    let provider = match values.get(&key) {
        Some(Value::Text(name)) if !name.trim().is_empty() => name.trim().to_string(),
        _ => target.provider.id.clone(),
    };
    Identity {
        provider,
        model: target.model.clone(),
        effort: effort.value.clone().unwrap_or_else(|| DEFAULT.to_string()),
        preset: String::new(),
    }
}
