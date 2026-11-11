//! 用户态总线（施工 BUS-2，设计 `67-总线.md` 第四节）：包照名字找、照名字调，不写死包名。
//!
//! - `bus.call {name, method, params?, package?}`：照名字从装着的包里找。`provides` 的一家直接调，写了 `package` 的要正是它；
//!   `plugs` 的要写挑哪家（`package_required`），那家要接着这个名字。方法写一段的短名，核心拼成 `<名字>.<短名>`，要写进了那家
//!   的 `[export]`（`not_exported`）。余下照 `package.call` 转（[`super::calls::forward`]：时限、blob 两个方向、错误）。在后台答。
//! - `bus.list {name?}`：总线上的名字 `[{"name", "kind", "package", "ready", "methods"}]`，照名字、包排；写了 `name` 只列这一个。
//!   `ready` 照登记算（B8）：程序连着，这个名字底下导出的方法都登记了。`methods`（施工 BUS-3补）是这一家在这个名字底下导出了、
//!   这时也登记上了的方法的短名，照字母排：接进同一个名字的几家各实现了哪几处，调的一方照它认；程序没连着的是空的。
//!
//! 能力照 `package.call`：扩展要声明了 `packages.call`、人批过（`connection/rights.rs`）；人的连接都调得到。

use serde::Deserialize;
use serde_json::{Value, json};

use miyu_config::package::{Export, under};

use crate::Core;
use crate::hello::Caller;
use crate::refusal::Refusal;

/// `bus.call` 的参数。
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CallParams {
    name: String,
    method: String,
    #[serde(default)]
    params: Option<Value>,
    #[serde(default)]
    package: Option<String>,
}

/// `bus.list` 的参数。
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ListParams {
    #[serde(default)]
    name: Option<String>,
}

/// 一个名字是哪一种。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Provides,
    Plugs,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Provides => "provides",
            Kind::Plugs => "plugs",
        }
    }
}

/// 总线上的一项：名字、哪一种、哪个包、那个包的导出。
struct Entry {
    name: String,
    kind: Kind,
    package: String,
    export: Export,
}

/// 装着、读得成的包在总线上声明的名字，照名字、包排。
fn entries(core: &Core) -> Vec<Entry> {
    let mut all: Vec<Entry> = core
        .system_packages()
        .iter()
        .filter_map(|found| Some((found.id.clone(), found.read.as_ref().ok()?.export.clone())))
        .flat_map(|(package, export)| {
            let named = |kind, names: &[String]| {
                names
                    .iter()
                    .map(|name| (name.clone(), kind))
                    .collect::<Vec<_>>()
            };
            let mut names = named(Kind::Provides, &export.provides);
            names.extend(named(Kind::Plugs, &export.plugs));
            names.into_iter().map(move |(name, kind)| Entry {
                name,
                kind,
                package: package.clone(),
                export: export.clone(),
            })
        })
        .collect();
    all.sort_by(|a, b| (&a.name, &a.package).cmp(&(&b.name, &b.package)));
    all
}

/// `entry` 可不可用（B8）：那个包的程序连着，名字底下导出的方法都登记了。
fn ready(core: &Core, entry: &Entry) -> bool {
    let Some(registered) = core.backstage.registered(&entry.package) else {
        return false;
    };
    entry
        .export
        .methods
        .iter()
        .filter(|method| under(method, &entry.name))
        .all(|method| registered.contains(method))
}

/// `entry` 那一家在这个名字底下导出了、这时也登记上了的方法的短名（施工 BUS-3补），照字母排；程序没连着的是空的。`bus.call`
/// 调得通的正是这几个。
fn methods(core: &Core, entry: &Entry) -> Vec<String> {
    let Some(registered) = core.backstage.registered(&entry.package) else {
        return Vec::new();
    };
    let mut short: Vec<String> = entry
        .export
        .methods
        .iter()
        .filter(|method| under(method, &entry.name) && registered.contains(*method))
        .filter_map(|method| method.rsplit_once('.').map(|(_, short)| short.to_string()))
        .collect();
    short.sort();
    short.dedup();
    short
}

/// `bus.list`：总线上的名字。
pub(crate) fn list(core: &Core, params: ListParams) -> Value {
    json!({"names": listed(core, params.name.as_deref())})
}

/// 总线上的名字（写了 `name` 的只列这一个），`bus.list` 的回应和 `bus` 流的推送共用。
pub(crate) fn listed(core: &Core, name: Option<&str>) -> Vec<Value> {
    entries(core)
        .iter()
        .filter(|entry| name.is_none_or(|name| entry.name == name))
        .map(|entry| {
            json!({
                "name": entry.name,
                "kind": entry.kind.as_str(),
                "package": entry.package,
                "ready": ready(core, entry),
                "methods": methods(core, entry),
            })
        })
        .collect()
}

/// `bus.call`：照名字找到包，转过去。
pub(crate) async fn call(
    core: &Core,
    caller: &Caller,
    params: CallParams,
) -> Result<Value, Refusal> {
    if params.method.is_empty() || params.method.contains('.') {
        return Err(Refusal::BAD_PARAMS);
    }
    let all = entries(core);
    let named: Vec<&Entry> = all
        .iter()
        .filter(|entry| entry.name == params.name)
        .collect();
    let entry = match (
        named.first().map(|entry| entry.kind),
        params.package.as_deref(),
    ) {
        (None, _) => return Err(Refusal::no_provider(&params.name)),
        (Some(Kind::Plugs), None) => return Err(Refusal::package_required(&params.name)),
        (Some(_), Some(package)) => named.iter().find(|entry| entry.package == package),
        (Some(Kind::Provides), None) => named.first(),
    }
    .ok_or_else(|| Refusal::no_provider(&params.name))?;
    let method = format!("{}.{}", params.name, params.method);
    if !entry.export.exports(&method) {
        return Err(Refusal::not_exported(&method));
    }
    let from = caller.package.as_deref();
    super::calls::forward(core, from, &entry.package, &method, params.params).await
}
