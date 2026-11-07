//! 供应商的驱动写成扩展的那一种（施工 MD-1，设计 66 第四节第 5 条，core1 11-07 照推荐定 A）：`providers.<id>.driver` 的选项是内置的三种，
//! 核心起来时接上装着、声明了 `[[driver]]` 的包的 `<包编号>/<驱动名>`（N-5 搜索来源的做法）。这一格另开一个窄口子：写法是
//! `<包编号>/<驱动名>`、这时不在选项里的（没装、写错了驱动名），不丢，照原样留下，记一条警告 `unknown_driver`；运行时这家供应商当
//! 用不了，绝不退回内置驱动：主人给扩展配的 key、地址不能拿去走别的协议。别的格、别的写法照旧 `not_an_option`。读、`config.set`
//! 写都走 [`check`]，读回、`miyu check`、头的下拉框（列不到的显示原值）对得上。

use std::sync::{Mutex, PoisonError};

use crate::problem::Code;
use crate::{Item, Value};

/// 那一格：一家供应商的驱动。
pub const PROVIDER_DRIVER: &str = "providers.<id>.driver";

/// `text` 是不是扩展驱动的写法：`<包编号>/<驱动名>`，包编号照包名的规则（[`crate::secret::valid_name`]），驱动名照清单 `[[driver]]`
/// 的 `name`（命令名的写法）。
pub fn extension(text: &str) -> bool {
    text.split_once('/').is_some_and(|(package, name)| {
        crate::secret::valid_name(package) && crate::package::driver_name(name)
    })
}

/// 「一家供应商的驱动」接上装着的扩展驱动（`extra` 是 `<包编号>/<驱动名>`，接在内置的几种后面，同 N-5 的搜索来源）。只在核心起来、
/// 装卸时调；不是这一格的照原样。
pub fn with_installed(item: &Item, extra: &[String]) -> Item {
    let mut item = item.clone();
    if let (true, crate::Kind::Option(shipped)) = (item.key == PROVIDER_DRIVER, item.kind)
        && !extra.is_empty()
    {
        let all: Vec<&str> = shipped
            .iter()
            .copied()
            .chain(extra.iter().map(String::as_str))
            .collect();
        item.kind = crate::Kind::Option(kept(&all));
    }
    item
}

/// 选项表要活到进程结束（[`crate::Kind`] 拿的是 `&'static`）：照内容找已经留着的那一份，没有才另留一份。装了又卸、卸了又装，
/// 同样的列表只留一次，不会每装卸一回多攒一份（core1 11-07 提的，施工 MD-2）。留下的份数是见过的不同列表数，很小。
fn kept(all: &[&str]) -> &'static [&'static str] {
    static KEPT: Mutex<Vec<&'static [&'static str]>> = Mutex::new(Vec::new());
    let mut kept = KEPT.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(found) = kept.iter().find(|found| **found == all) {
        return found;
    }
    let leaked: Vec<&'static str> = all
        .iter()
        .map(|text| &*Box::leak(text.to_string().into_boxed_str()))
        .collect();
    let leaked: &'static [&'static str] = Box::leak(leaked.into_boxed_slice());
    kept.push(leaked);
    leaked
}

/// 照 `item` 查 `value`：合的 `Ok(None)`；是那个口子的，`Ok(Some(Code::UnknownDriver))`，值照留、另记警告；不合的照原因码。
///
/// # Errors
///
/// 不合的那一种原因码，同 [`crate::Kind::check`]。
pub fn check(item: &Item, value: &Value) -> Result<Option<Code>, Code> {
    match (item.kind.check(value), value) {
        (Ok(()), _) => Ok(None),
        (Err(Code::NotAnOption), Value::Text(text))
            if item.key == PROVIDER_DRIVER && extension(text) =>
        {
            Ok(Some(Code::UnknownDriver))
        }
        (Err(code), _) => Err(code),
    }
}

#[cfg(test)]
mod tests;
