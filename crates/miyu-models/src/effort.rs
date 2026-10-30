//! 思考强度（`docs/blueprint/models.md`「怎么走」第十一条，施工 8-18；8-18（补）去掉了会话那一层，SM-1 加回来，设计 51）：
//! 每个模型的一项配置，写在系统配置里；会话能另记一档，压在配置上。
//!
//! - 档位名（[`normalize`]、[`levels`]、[`offered`]）：照目录原样，`none`、`disabled` 读成 `off`；目录有开关、这一家的档案
//!   写了开关的，多一档 `off`（放在最前面），只有开关的是 `off`、`on`。档案没写开关的，目录的开关不算：驱动说不出来。
//! - 一次请求用哪一档（[`held`]）：会话记的那一档在真发的那个模型这时的档位里就照它；不然照配置的最终值
//!   （[`crate::facts::Facts::effort`]，已经照档位查过），没有就不带。
//! - 给头看的那一档，连同从哪来（[`shown`]、[`in_use`]）：会话的写 `session`，配置的照 `facts.effort` 的来源是哪一层配置
//!   文件，翻成 `subscribe`、`model.changed` 的 `effort.from`。
//! - `session.configure` 的 `effort` 照 [`levels_of`] 查：会话这时的引用有哪几档，池是成员的合起来。
//! - 空闲超时放大几倍（[`idle_factor`]）：`high` 2 倍、`xhigh` 3 倍、`max` 4 倍，别的照基数（`15-模型与供应商.md` 第五节）。
//! - 配置里写的不在档位里的（[`unknown`]）：报 `unknown_effort`，只报不丢，请求照没写发。档位由用它的一方交（要档案、目录）。

use miyu_config::key;
use miyu_config::merge::Resolved;
use miyu_config::parse::Parsed;
use miyu_config::problem::{Code, Problem};
use miyu_config::{Layer, Value};
use miyu_drivers::{EFFORT_OFF, EFFORT_ON};
use miyu_kernel::event::{EffortInUse, EffortSource};

use crate::Knowledge;
use crate::catalog::Reasoning;
use crate::facts::{Fact, Source, facts};
use crate::provider;
use crate::reference::{self, Resolved as Pointed};

/// 配置里模型默认的思考强度那一项（[`crate::settings::ModelSettings`] 的 `effort`）。
pub const ITEM: &str = "providers.<id>.models.<model>.effort";

/// 规整一档的名字：`none`、`disabled` 是关掉，读成 `off`；别的照原样。
pub fn normalize(name: &str) -> &str {
    match name {
        "none" | "disabled" => EFFORT_OFF,
        other => other,
    }
}

/// 一串档位名规整好：每个照 [`normalize`]，重复的只留第一个，先后不变。
pub fn levels(names: &[String]) -> Vec<String> {
    let mut levels: Vec<String> = Vec::with_capacity(names.len());
    for name in names {
        let name = normalize(name);
        if !levels.iter().any(|seen| seen == name) {
            levels.push(name.to_string());
        }
    }
    levels
}

/// 目录里一个模型的思考强度，这一家能给的几档：`switchable` 是这一家能照开关关思考（`openai-chat` 的档案写了开关
/// `compat.toggle`，`anthropic` 自带，[`crate::provider::Provider::switchable`]）。目录有开关、能开关
/// 的，没有 `off` 的在最前面加一档 `off`，一档都没有的是 `off`、`on`；别的照目录的档位。一档都没有的没有。
pub fn offered(reasoning: &Reasoning, switchable: bool) -> Option<Vec<String>> {
    let mut levels = reasoning.levels.clone();
    if reasoning.toggle && switchable {
        if levels.is_empty() {
            levels = vec![EFFORT_OFF.to_string(), EFFORT_ON.to_string()];
        } else if !levels.iter().any(|level| level == EFFORT_OFF) {
            levels.insert(0, EFFORT_OFF.to_string());
        }
    }
    (!levels.is_empty()).then_some(levels)
}

/// 会话记的那一档 `session`，在这个模型这时的档位 `levels` 里的（施工 SM-1，「怎么走」第十一条第 4 条）：一次请求照它，
/// 压着配置的默认；不在的（换了模型、目录变了）没有，照配置。
pub fn held<'a>(session: Option<&'a str>, levels: &[String]) -> Option<&'a str> {
    session.filter(|level| levels.iter().any(|known| known == level))
}

/// 给头看的那一档（施工 SM-1）：会话记的、这个模型有的，`from` 是 `session`；不然照配置的那一格（[`in_use`]）。
pub fn shown(
    session: Option<&str>,
    effort: &Fact<Option<String>>,
    levels: &[String],
) -> Option<EffortInUse> {
    match held(session, levels) {
        Some(level) => Some(EffortInUse {
            level: level.to_string(),
            from: EffortSource::Session,
        }),
        None => in_use(effort),
    }
}

/// 引用 `text`（模型或 `@池`）这时有哪几档，照最终值 `resolved`、手头的资料 `knowledge`（施工 SM-1：`session.configure` 的
/// `effort` 照它查）：模型的是它自己的；池的是认得出的成员各自的合起来（照先后，重复的只留第一个），用不上的成员照配置。
/// 解析不出、那一家用不了的一档都没有。
pub fn levels_of(resolved: &Resolved, knowledge: &Knowledge<'_>, text: &str) -> Vec<String> {
    let values = resolved.values();
    let of = |id: &str, model: &str| match provider::provider(&values, knowledge, id) {
        Ok(found) => facts(resolved, knowledge, &found, model)
            .0
            .levels()
            .to_vec(),
        Err(_) => Vec::new(),
    };
    let found = match reference::resolve(&values, knowledge, text) {
        Ok(Pointed::Model(target)) => of(&target.provider.id, &target.model),
        Ok(Pointed::Pool(pool)) => pool
            .members
            .iter()
            .flat_map(|member| of(&member.provider, &member.model))
            .collect(),
        Err(_) => Vec::new(),
    };
    levels(&found)
}

/// 给头看的那一档，连同从配置的哪一层来（「怎么走」第十一条第 4、7 条）：`effort` 是 [`crate::facts::facts`] 算出来的
/// 那一格（已经照档位查过，不在档位里的、没写的值是 `None`）。写在配置文件里的，`from` 是那一层；没写的（来源不是
/// [`Source::Config`]）什么都不带。
pub fn in_use(effort: &Fact<Option<String>>) -> Option<EffortInUse> {
    let level = effort.value.clone()?;
    let from = match &effort.source {
        Source::Config {
            layer: Layer::System,
            ..
        } => EffortSource::System,
        // 这一项清单里只许系统那一层（`config.md`「配置清单」），项目配置写了不算数：不会走到这里。
        Source::Config {
            layer: Layer::Project,
            ..
        } => EffortSource::Other("project".to_string()),
        _ => return None,
    };
    Some(EffortInUse { level, from })
}

/// 空闲超时照这一次的一档放大几倍（「怎么走」第十一条第 6 条）：`high` 2、`xhigh` 3、`max` 4，别的（连同 `off`、`on`、
/// 没有）1。
pub fn idle_factor(level: Option<&str>) -> u32 {
    match level {
        Some("high") => 2,
        Some("xhigh") => 3,
        Some("max") => 4,
        _ => 1,
    }
}

/// 一层配置 `parsed`（当成 `layer` 读的）里，模型默认的思考强度不在档位里的（「怎么走」第十一条第 2 条）：一处一条
/// `unknown_effort`（错误），`name` 是写的那一档，指到值那里。只看这一层算数的项。`levels` 说供应商 `<id>` 的模型
/// `<model>` 这时有哪几档；说不出来的（那一家用不了）不查。
pub fn unknown(
    parsed: &Parsed,
    layer: Layer,
    levels: &dyn Fn(&str, &str) -> Option<Vec<String>>,
) -> Vec<Problem> {
    let mut found = Vec::new();
    for (written, entry) in &parsed.entries {
        if entry.item != ITEM || !entry.counts {
            continue;
        }
        let (Value::Text(level), Some(segments)) = (&entry.value, key::split(written)) else {
            continue;
        };
        let [_, provider, _, model, _] = segments.as_slice() else {
            continue;
        };
        let Some(known) = levels(provider, model) else {
            continue;
        };
        if known.iter().any(|known| known == normalize(level)) {
            continue;
        }
        let mut problem = Problem::item(Code::UnknownEffort, layer, written, entry.at, &entry.raw);
        problem.name = Some(level.to_string());
        found.push(problem);
    }
    found
}

#[cfg(test)]
mod tests;
