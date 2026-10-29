//! `persona.set` 里外观的几格（施工 P-6、P-7，从 `write.rs` 挪出来：那一份到了 500 行）：主题色 `seed`、背景图露出多少
//! `background_opacity`、焦点 `background_focus`、模糊多少 `background_blur`（P-7 补），各改成 `persona.toml` 的 `[appearance]` 里那一项。

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

/// `persona.set` 的 `background_opacity`、`background_focus`、`background_blur`：一个值，或者删掉（施工 P-7）。
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

/// 背景图露出多少、焦点、模糊多少在 `persona.toml` 里的键（施工 P-7、P-7 补）。
const OPACITY: &str = "appearance.background_opacity";
const FOCUS: &str = "appearance.background_focus";
const BLUR: &str = "appearance.background_blur";

/// `persona.set` 里背景图的三格（施工 P-7、P-7 补）：没写的是不改。
pub(super) struct Backdrop {
    pub(super) opacity: Option<NumberParams<i64>>,
    pub(super) focus: Option<NumberParams<Vec<i64>>>,
    pub(super) blur: Option<NumberParams<i64>>,
}

/// 查 `background_opacity`、`background_focus`、`background_blur`，改成那三项（施工 P-7、P-7 补）：露出多少不是 0 到 100、
/// 焦点不是两个数、模糊不是 1 到 100、`unset` 不是 `true`、`changes` 里也改了同一项的 `bad_params`。
pub(super) fn backdrop(backdrop: Backdrop, wanted: &mut Vec<Wanted>) -> Result<(), Refusal> {
    let Backdrop {
        opacity,
        focus,
        blur,
    } = backdrop;
    let mut put = |key: &str, value: Option<Value>| {
        if wanted.iter().any(|want| want.key() == key) {
            return Err(Refusal::BAD_PARAMS);
        }
        wanted.push(Wanted::value(key, value));
        Ok(())
    };
    if let Some(opacity) = opacity {
        put(OPACITY, number(opacity, persona::percent)?)?;
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
                Some(Value::List(
                    vec![Value::Int(i64::from(x)), Value::Int(i64::from(y))].into(),
                ))
            }
            NumberParams::Unset { unset: true } => None,
            NumberParams::Unset { unset: false } => return Err(Refusal::BAD_PARAMS),
        };
        put(FOCUS, value)?;
    }
    if let Some(blur) = blur {
        put(BLUR, number(blur, persona::blur)?)?;
    }
    Ok(())
}

/// 一个数照 `check` 查过交回（`None` 是删掉）；查不过的、`unset` 不是 `true` 的 `bad_params`。
fn number(
    params: NumberParams<i64>,
    check: fn(i64) -> Option<u8>,
) -> Result<Option<Value>, Refusal> {
    match params {
        NumberParams::Value(number) => {
            let checked = check(number).ok_or(Refusal::BAD_PARAMS)?;
            Ok(Some(Value::Int(i64::from(checked))))
        }
        NumberParams::Unset { unset: true } => Ok(None),
        NumberParams::Unset { unset: false } => Err(Refusal::BAD_PARAMS),
    }
}
