//! `persona.set` 里外观的几格（施工 P-6、P-7，从 `write.rs` 挪出来：那一份到了 500 行）：主题色 `seed`、背景图露出多少
//! `background_opacity`、焦点 `background_focus`，各改成 `persona.toml` 的 `[appearance]` 里那一项。

use serde::Deserialize;

use miyu_config::Value;
use miyu_policy::persona;

use crate::refusal::Refusal;
use crate::toml_changes::Wanted;

/// `persona.set` 的 `seed`：一个颜色，或者删掉。
#[derive(Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(super) enum SeedParams {
    /// `#rrggbb`。
    Color(String),
    /// `{"unset": true}`。
    Unset {
        /// 只能是 `true`。
        unset: bool,
    },
}

/// 主题色在 `persona.toml` 里的键。
const SEED: &str = "appearance.seed";

/// 查 `seed`，改成 `appearance.seed` 那一项：颜色不是 `#rrggbb`、`unset` 不是 `true`、`changes` 里也改了这一项的 `bad_params`。
pub(super) fn seed(params: Option<SeedParams>, wanted: &mut Vec<Wanted>) -> Result<(), Refusal> {
    let Some(params) = params else {
        return Ok(());
    };
    let color = match params {
        SeedParams::Color(text) => Some(persona::color(&text).ok_or(Refusal::BAD_PARAMS)?),
        SeedParams::Unset { unset: true } => None,
        SeedParams::Unset { unset: false } => return Err(Refusal::BAD_PARAMS),
    };
    if wanted.iter().any(|want| want.key() == SEED) {
        return Err(Refusal::BAD_PARAMS);
    }
    wanted.push(Wanted::text(SEED, color.as_deref()));
    Ok(())
}

/// `persona.set` 的 `background_opacity`、`background_focus`：一个值，或者删掉（施工 P-7）。
#[derive(Debug, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub(super) enum NumberParams<T> {
    /// 值。
    Value(T),
    /// `{"unset": true}`。
    Unset {
        /// 只能是 `true`。
        unset: bool,
    },
}

/// 背景图露出多少、焦点在 `persona.toml` 里的键（施工 P-7）。
const OPACITY: &str = "appearance.background_opacity";
const FOCUS: &str = "appearance.background_focus";

/// 查 `background_opacity`、`background_focus`，改成那两项（施工 P-7）：不是 0 到 100、焦点不是两个数、`unset` 不是 `true`、
/// `changes` 里也改了同一项的 `bad_params`。
pub(super) fn backdrop(
    opacity: Option<NumberParams<i64>>,
    focus: Option<NumberParams<Vec<i64>>>,
    wanted: &mut Vec<Wanted>,
) -> Result<(), Refusal> {
    let mut put = |key: &str, value: Option<Value>| {
        if wanted.iter().any(|want| want.key() == key) {
            return Err(Refusal::BAD_PARAMS);
        }
        wanted.push(Wanted::value(key, value));
        Ok(())
    };
    if let Some(opacity) = opacity {
        let value = match opacity {
            NumberParams::Value(number) => {
                let percent = persona::percent(number).ok_or(Refusal::BAD_PARAMS)?;
                Some(Value::Int(i64::from(percent)))
            }
            NumberParams::Unset { unset: true } => None,
            NumberParams::Unset { unset: false } => return Err(Refusal::BAD_PARAMS),
        };
        put(OPACITY, value)?;
    }
    if let Some(focus) = focus {
        let value = match focus {
            NumberParams::Value(numbers) => {
                let [x, y] = numbers.as_slice() else {
                    return Err(Refusal::BAD_PARAMS);
                };
                let (x, y) = (persona::percent(*x), persona::percent(*y));
                let (Some(x), Some(y)) = (x, y) else {
                    return Err(Refusal::BAD_PARAMS);
                };
                Some(Value::List(vec![
                    Value::Int(i64::from(x)),
                    Value::Int(i64::from(y)),
                ]))
            }
            NumberParams::Unset { unset: true } => None,
            NumberParams::Unset { unset: false } => return Err(Refusal::BAD_PARAMS),
        };
        put(FOCUS, value)?;
    }
    Ok(())
}
