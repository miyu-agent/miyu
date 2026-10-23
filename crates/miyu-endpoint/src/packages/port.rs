//! 软件包的端口（施工 F-10 上，`miyu_tool::PackagesPort`）：`packages` 这件工具经它看、在软件源里搜（施工 G-6），`package` 经它
//! 装、卸、从软件源装、升级（施工 F-10 下、G-6，
//! 同 `package.install`、`package.remove`，不查连接是不是本机的：权限策略照会话的级别问过人了）。照协议那几样回应算（`package.list`、
//! `package.info`、`package.install` 带 `preview` 的），名字、说明、写错的那一句照英文：她看的字是英文。拿着核心的弱引用：
//! 会话活在核心里面，拿强的会绕成环。

use std::path::PathBuf;
use std::sync::{Arc, Weak};

use serde_json::{Value, json};

use miyu_kernel::id::AccountId;
use miyu_tool::{Looking, PackageConflict, PackageRefusal, PackagesPort};

use super::local::InfoParams;
use super::source;
use super::source::fetch::SourceParams;
use super::source::search::SearchParams;
use super::source::upgrade::UpgradeParams;
use miyu_policy::preset::Unlisted;

use super::TARGET;
use crate::Core;
use crate::hello::Peer;
use crate::refusal::{Locale, Refusal};

/// 交给会话的那一份。
pub(crate) fn port(core: &Arc<Core>, owner: &AccountId) -> Arc<dyn PackagesPort> {
    Arc::new(Port {
        core: Arc::downgrade(core),
        owner: owner.clone(),
    })
}

/// 核心那一头，和这个会话的属主（施工 Y-5 下下：她替属主装包，属主不是管理员的装不了带进程的包）。
struct Port {
    core: Weak<Core>,
    owner: AccountId,
}

impl Port {
    /// 核心还在；正在停的拒 `shutting_down`。
    fn core(&self) -> Result<Arc<Core>, PackageRefusal> {
        self.core.upgrade().ok_or_else(|| PackageRefusal {
            reason: "shutting_down".to_string(),
            problem: None,
            line: None,
            conflicts: Vec::new(),
        })
    }
}

impl Port {
    /// 改机器上的软件包之前（装、升级、卸、看一眼、启用扩展，施工 Y-5 下下）：核心还在，会话属主是管理员；成员 `not_permitted`。
    /// 只读的（列、看、搜）照旧用 [`Port::core`]。
    fn managing(&self) -> Result<Arc<Core>, PackageRefusal> {
        let core = self.core()?;
        super::may_manage::may_manage(self.peer(&core).admin).map_err(refused)?;
        Ok(core)
    }

    /// 照英文说（名字、说明、清单哪里不对），是不是管理员照会话的属主算（施工 Y-5 下下）。
    fn peer(&self, core: &Core) -> Peer {
        Peer {
            locale: Locale::En,
            language: "en",
            input: false,
            admin: crate::login::admins::is_admin(core, &self.owner),
        }
    }
}

/// 协议上的拒绝换成端口的：原因代码，写错的清单带上哪里不对、第几行，撞了名的带上每一处（施工 F-8 六补修）。
fn refused(refusal: Refusal) -> PackageRefusal {
    let data = refusal.data.unwrap_or_default();
    let field = |one: &Value, key: &str| one[key].as_str().unwrap_or_default().to_string();
    PackageRefusal {
        reason: refusal.reason.to_string(),
        problem: data
            .get("problem")
            .and_then(Value::as_str)
            .map(str::to_string),
        line: data.get("line").and_then(Value::as_u64),
        conflicts: data
            .get("conflicts")
            .and_then(Value::as_array)
            .map(|all| {
                all.iter()
                    .map(|one| PackageConflict {
                        kind: field(one, "kind"),
                        name: field(one, "name"),
                        // 撞了人格、预设的一层的（施工 P-11 下）照那一层的写法填：`home`、`system`、`shipped`。
                        package: one
                            .get("package")
                            .or_else(|| one.get("layer"))
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

impl PackagesPort for Port {
    fn list(&self) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
            let listed = super::list(&core, self.peer(&core)).map_err(refused)?;
            Ok(listed["packages"].clone())
        })
    }

    fn info(&self, package: String) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
            let params: InfoParams = serde_json::from_value(json!({ "package": package }))
                .map_err(|_| refused(Refusal::BAD_PARAMS))?;
            let mut info = super::local::info(&core, params).await.map_err(refused)?;
            if let Some(entry) = super::entry(&core, &package, self.peer(&core)) {
                for key in ["name", "summary", "kind"] {
                    if let Some(value) = entry.get(key) {
                        info[key] = value.clone();
                    }
                }
            }
            Ok(info)
        })
    }

    fn inspect(&self, path: PathBuf) -> Looking<'_> {
        Box::pin(async move {
            let core = self.managing()?;
            super::preview::installing(&core, self.peer(&core), &path)
                .await
                .map_err(refused)
        })
    }

    fn install(&self, path: PathBuf, preset: Option<String>) -> Looking<'_> {
        Box::pin(async move {
            let core = self.managing()?;
            let params = serde_json::from_value(json!({ "path": path }))
                .map_err(|_| refused(Refusal::BAD_PARAMS))?;
            let entry = super::manage::install(&core, self.peer(&core), params)
                .await
                .map_err(refused)?;
            if let (Some(preset), Some(package)) = (preset, entry["package"].as_str()) {
                open_in(&core, (&self.owner, self.peer(&core)), &preset, package).await;
            }
            Ok(entry)
        })
    }

    fn enabling(&self, package: String) -> Looking<'_> {
        Box::pin(async move {
            let core = self.managing()?;
            super::preview::enabling(&core, self.peer(&core), &package).map_err(refused)
        })
    }

    fn enable(&self, package: String) -> Looking<'_> {
        Box::pin(async move {
            let core = self.managing()?;
            let (manifest, _) = super::switch::switchable(&core, &package).map_err(refused)?;
            // 要的能力一并批：问人的那一次就是批（工作区一级），完全放开的照级别直接做。
            let approve: Vec<Value> = crate::extensions::described(&core, &manifest, "en")
                .and_then(|listed| listed.as_array().cloned())
                .unwrap_or_default()
                .into_iter()
                .map(|one| one["id"].clone())
                .collect();
            let params = serde_json::from_value(json!({ "package": package, "approve": approve }))
                .map_err(|_| refused(Refusal::BAD_PARAMS))?;
            super::switch::enable(&core, self.peer(&core), params)
                .await
                .map_err(refused)
        })
    }

    fn removing(&self, package: String) -> Looking<'_> {
        Box::pin(async move { self.remove_or_look(package, true).await })
    }

    fn remove(&self, package: String) -> Looking<'_> {
        Box::pin(async move { self.remove_or_look(package, false).await })
    }

    fn search(&self, query: String) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
            let found = source::search::search(&core, self.peer(&core), SearchParams::of(query))
                .await
                .map_err(refused)?;
            Ok(found["packages"].clone())
        })
    }

    fn fetching(&self, package: String) -> Looking<'_> {
        Box::pin(async move {
            let core = self.managing()?;
            source::fetch::install(
                &core,
                self.peer(&core),
                None,
                SourceParams::looking(&package),
            )
            .await
            .map_err(refused)
        })
    }

    fn fetch(&self, package: String, preset: Option<String>) -> Looking<'_> {
        Box::pin(async move {
            let core = self.managing()?;
            let entry =
                source::fetch::install(&core, self.peer(&core), None, SourceParams::of(&package))
                    .await
                    .map_err(refused)?;
            if let (Some(preset), Some(package)) = (preset, entry["package"].as_str()) {
                open_in(&core, (&self.owner, self.peer(&core)), &preset, package).await;
            }
            Ok(entry)
        })
    }

    fn upgrading(&self, only: Option<String>) -> Looking<'_> {
        Box::pin(async move {
            let core = self.managing()?;
            source::upgrade::upgrade(&core, self.peer(&core), None, UpgradeParams::of(only, true))
                .await
                .map_err(refused)
        })
    }

    fn upgrade(&self, only: Option<String>) -> Looking<'_> {
        Box::pin(async move {
            let core = self.managing()?;
            source::upgrade::upgrade(
                &core,
                self.peer(&core),
                None,
                UpgradeParams::of(only, false),
            )
            .await
            .map_err(refused)
        })
    }
}

/// 在会话用的预设 `preset` 里打开她装的包 `package`（施工 F-10 下下，16 第三节）：预设不开没列出来的软件、也没写这个包的，往
/// 家目录那一层的同名覆盖里写 `software.<编号> = true`；开着的、写过的（人关过的照人的）不动。写不成的记一行，装照样算成。
/// 预设照会话属主 `owner` 的找、写（施工 Y-5 下下：原来照机主，成员的会话改的是机主的预设）。
async fn open_in(core: &Arc<Core>, (owner, peer): (&AccountId, Peer), preset: &str, package: &str) {
    let Ok(found) = crate::presets::presets(core, owner).find(preset) else {
        return;
    };
    if found.file.unlisted() == Unlisted::On || found.file.software.contains_key(package) {
        return;
    }
    let change = json!({ "preset": preset, "changes": [{ "key": format!("software.{package}"), "value": true }] });
    let Ok(params) = serde_json::from_value(change) else {
        return;
    };
    if let Err(refusal) = crate::presets::write::set(core, owner, peer, params).await {
        tracing::warn!(target: TARGET, preset, package, reason = refusal.reason, "package not opened in the preset");
    }
}

impl Port {
    /// 卸包 `package`，`preview` 的只看一眼（施工 F-10 下）：同 `package.remove`。
    async fn remove_or_look(
        &self,
        package: String,
        preview: bool,
    ) -> Result<Value, PackageRefusal> {
        let core = self.managing()?;
        let params = serde_json::from_value(json!({ "package": package, "preview": preview }))
            .map_err(|_| refused(Refusal::BAD_PARAMS))?;
        super::manage::remove(&core, params).await.map_err(refused)
    }
}

#[cfg(test)]
mod tests;
