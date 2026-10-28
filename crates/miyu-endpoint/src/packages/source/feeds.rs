//! 几个软件源（施工 U23 下下，设计 `37-软件源.md` 第五节「别人的软件源」）：官方那一个（`packages.source`、镜像、编进来的
//! 公钥），加上配置里 `packages.sources.<名字>` 写了地址和公钥的。加了就是信任它：
//! 索引照它自己那把公钥验，验不过的不收。
//!
//! - 名字 `official` 留给官方那一个，谁都不许叫它，在配置检查里报（[`problems`]，原因码 `SourceReserved`）；只写了一半的、
//!   公钥读不成的也报，那个源不用。
//! - 每个源一份索引：官方的照旧在 `state/packages/.sync/`，别的在 `state/packages/.sources/<名字>/`。

use std::collections::BTreeMap;
use std::path::PathBuf;

use miyu_config::parse::{Entry, Parsed};
use miyu_config::problem::{Code, Problem};
use miyu_config::{Address, Layer, Setting, Value};
use miyu_source::sign::PublicKey;

use super::super::packages;
use crate::Core;
use crate::settings::SourceSettings;

/// 官方那一个的名字：保留，配置里谁都不许用。
pub(crate) const OFFICIAL: &str = "official";

/// 源是谁加的。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tier {
    /// 官方那一个。
    Official,
    /// 配置里加的。
    Added,
}

/// 一个软件源。
#[derive(Debug, Clone)]
pub(crate) struct Feed {
    /// 名字：官方的是 [`OFFICIAL`]，别的是配置里起的。
    pub(crate) name: String,
    /// 谁加的。
    pub(crate) tier: Tier,
    /// 照先后试的地址，结尾带 `/`：官方的是镜像再原址，别的只有一个。
    pub(crate) bases: Vec<String>,
    /// 认哪几把公钥。
    pub(crate) keys: Vec<PublicKey>,
    /// 本机存它的索引的目录。
    pub(crate) dir: PathBuf,
}

impl Feed {
    /// 包文件的镜像（照 `<镜像>files/<文件名>` 下）：只有官方的有。
    pub(crate) fn mirrors(&self) -> &[String] {
        match self.tier {
            Tier::Official => &self.bases[..self.bases.len().saturating_sub(1)],
            Tier::Added => &[],
        }
    }
}

/// 官方那一个：地址照 `packages.mirrors`、`packages.source`，公钥是编进来的（测试换成自己的）。
pub(crate) fn official(core: &Core) -> Feed {
    let settings = SourceSettings::from(&core.config().resolved().values());
    Feed {
        name: OFFICIAL.to_string(),
        tier: Tier::Official,
        bases: super::sync::bases(&settings),
        keys: core.trust.keys.clone(),
        dir: packages(core).state_root().join(".sync"),
    }
}

/// 用得上的源，照先后：官方的、配置里加的（照名字排）。写错的、不认的跳过。
pub(crate) fn feeds(core: &Core) -> Vec<Feed> {
    let system = written(&core.config().file().parsed);
    let mut all = vec![official(core)];
    let state = packages(core).state_root().join(".sources");
    all.extend(
        system
            .iter()
            .filter(|(name, _)| name.as_str() != OFFICIAL)
            .filter_map(|(name, one)| feed(name, one, state.join(name))),
    );
    all
}

/// 一层配置里写的一个源：地址、公钥，和写它的第一项（问题指到那里）。
#[derive(Debug, Default)]
struct Written {
    url: Option<Address>,
    key: Option<String>,
    first: Option<(String, Entry)>,
}

/// 一层读好的 `parsed` 里写了的 `packages.sources.<名字>.url`、`.key`，照名字。
fn written(parsed: &Parsed) -> BTreeMap<String, Written> {
    let mut found: BTreeMap<String, Written> = BTreeMap::new();
    for (key, entry) in &parsed.entries {
        let Some((name, field)) = key
            .strip_prefix("packages.sources.")
            .and_then(|rest| rest.rsplit_once('.'))
        else {
            continue;
        };
        let one = found.entry(name.to_string()).or_default();
        if one.first.is_none() {
            one.first = Some((key.clone(), entry.clone()));
        }
        match (field, &entry.value) {
            ("url", value) => one.url = <Option<Address> as Setting>::read(Some(value)),
            ("key", Value::Text(text)) => one.key = Some(text.to_string()),
            _ => {}
        }
    }
    found
}

/// 写全了、地址取得到、公钥读得成的才是一个源。
fn feed(name: &str, one: &Written, dir: PathBuf) -> Option<Feed> {
    let bases = super::sync::resolved(one.url.iter());
    let key = PublicKey::from_base64(one.key.as_deref()?)?;
    (!bases.is_empty()).then(|| Feed {
        name: name.to_string(),
        tier: Tier::Added,
        bases,
        keys: vec![key],
        dir,
    })
}

/// 配置检查报的（施工 U23 下下，`Config::missing` 接上）：`layer` 这一层读好的 `parsed` 里写的源，叫 `official` 的、只写了一半的、
/// 公钥读不成的。问题指到那个源写的第一行。
pub(crate) fn problems(parsed: &Parsed, layer: Layer) -> Vec<Problem> {
    let mut found = Vec::new();
    for (name, one) in written(parsed) {
        let code = if name == OFFICIAL {
            Code::SourceReserved
        } else if one.url.is_none() || one.key.is_none() {
            Code::SourceIncomplete
        } else if one
            .key
            .as_deref()
            .and_then(PublicKey::from_base64)
            .is_none()
        {
            Code::SourceKey
        } else {
            continue;
        };
        if let Some((key, entry)) = one.first {
            let mut problem = Problem::item(code, layer, &key, entry.at, &entry.raw);
            problem.name = Some(name);
            found.push(problem);
        }
    }
    found
}

#[cfg(test)]
mod tests;
