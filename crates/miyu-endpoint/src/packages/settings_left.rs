//! 卸包留下的设置项（施工 PK-5补，`docs/blueprint/packages.md`「装卸」第 7 条）：照 pacman，不带 `purge` 卸包时 `etc/config.toml`
//! 里它的设置项原样留着，重装同编号照原样生效。
//!
//! - [`left`]：这时哪些前缀是已卸的包留下的（读配置时照它跳过，`config/left.rs`）。
//! - `purge`：对着没装、但还留着东西的编号 `package.remove --purge`：只删留下的三样（配置里它的设置项、配置目录
//!   `etc/<编号>/`、`etc/leftover/<编号>.toml`），`preview` 的照常先说一遍。不带 `purge` 的、什么都没留的照旧 `unknown_package`。

use std::collections::BTreeSet;
use std::sync::Arc;

use serde_json::{Map, Value, json};

use miyu_store::packages::settings_left;
use miyu_store::packages::{Found, Layer, Packages};
use miyu_store::root::DataRoot;

use super::TARGET;
use super::manage::blocking;
use crate::Core;
use crate::config::Left;
use crate::refusal::Refusal;

/// 这时已卸的包留下的设置项的前缀：卸掉的出厂、系统加装的包照 `places` 里的清单（`read_removed`，声明了设置项的），你装的
/// 包照 `etc/leftover/` 记的；`found` 里有的编号（这时装着的）不算。核心起来时交给配置服务（`Config::load_leaving`），装卸
/// 以后跟着换（`Core::reload_packages`）。
pub fn left(places: &Packages, root: &DataRoot, found: &[Found]) -> Left {
    let installed: BTreeSet<&str> = found.iter().map(|one| one.id.as_str()).collect();
    let shipped: Vec<(String, String)> = places
        .read_removed()
        .iter()
        .filter(|one| !installed.contains(one.id.as_str()))
        .filter_map(|one| {
            let manifest = one.read.as_ref().ok()?;
            (!manifest.settings.is_empty())
                .then(|| (manifest.prefix_of(&one.id).to_string(), one.id.clone()))
        })
        .collect();
    let marked: Vec<(String, String)> = settings_left::read(root)
        .into_iter()
        .filter(|(id, _)| !installed.contains(id.as_str()))
        .map(|(id, prefix)| (prefix, id))
        .collect();
    Left::new(shipped.into_iter().chain(marked))
}

/// 清单 `found` 里下面一层（系统加装的、出厂的）有编号是 `id` 的一份（施工 PK-5补；SD-1 起被你装的盖住，记成
/// `Issue::Covered`，`package.list` 里是 `shadowed`）：卸掉你装的那一份以后它顶上来，配置里的键照它的清单认，不用记号。
pub fn lower_has(found: &[Found], id: &str) -> bool {
    found
        .iter()
        .any(|one| one.id == id && one.layer != Layer::Home)
}

/// 没装的包 `id` 留下的东西：配置里它的键、有东西的配置目录（看一眼照它写）、配置目录在不在、有没有记号。
struct Leftovers {
    keys: Vec<String>,
    shown: Option<std::path::PathBuf>,
    there: bool,
    marked: bool,
}

impl Leftovers {
    async fn of(core: &Core, id: &str) -> Result<Leftovers, Refusal> {
        let keys = crate::config::forget::left_by(core, id);
        let (config, root, target) = (
            core.root.package_config(id),
            core.root.clone(),
            id.to_string(),
        );
        blocking(move || {
            Ok(Leftovers {
                keys,
                shown: super::preview::holds_something(&config).then_some(config.clone()),
                there: std::fs::symlink_metadata(&config).is_ok(),
                marked: settings_left::has(&root, &target),
            })
        })
        .await
    }

    fn nothing(&self) -> bool {
        self.keys.is_empty() && !self.there && !self.marked
    }
}

/// `package.remove` 对着没装的编号 `id`、带了 `purge`（施工 PK-5补）：只删留下的三样；`preview` 的只说会删什么（`{package, settings,
/// config}`，同装着的包那一份的写法）。什么都没留的 `unknown_package`。
pub(crate) async fn purge(core: &Arc<Core>, id: &str, preview: bool) -> Result<Value, Refusal> {
    if !miyu_store::personas::valid(id) {
        return Err(Refusal::BAD_PARAMS);
    }
    let _one_at_a_time = core.packaging.lock().await;
    let left = Leftovers::of(core, id).await?;
    if left.nothing() {
        return Err(Refusal::UNKNOWN_PACKAGE);
    }
    if preview {
        let mut answer = Map::new();
        answer.insert("package".into(), json!(id));
        answer.insert("settings".into(), json!(left.keys));
        let entry = super::preview::config_entry(left.shown.as_deref(), left.keys.len(), true);
        if let Some(entry) = entry {
            answer.insert("config".into(), entry);
        }
        return Ok(Value::Object(answer));
    }
    crate::config::forget::forget(core, &left.keys);
    let (config, root, target) = (
        core.root.package_config(id),
        core.root.clone(),
        id.to_string(),
    );
    blocking(move || {
        super::remove::remove_dir_if_there(&config)?;
        settings_left::unmark(&root, &target)
    })
    .await?;
    refresh(core);
    tracing::info!(target: TARGET, package = id, "package leftovers purged");
    Ok(json!({"package": id, "removed": true}))
}

/// 记号变了以后照这时的清单重算留下的前缀、配置照手里的字重新认（清单不变）。
fn refresh(core: &Core) {
    let left = left(&super::packages(core), &core.root, &core.system_packages());
    let items = core.config().items().to_vec();
    crate::config::refit::refit(core, items, left);
}
