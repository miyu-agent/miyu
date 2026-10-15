//! 软件包的端口（施工 F-10 上，`miyu_tool::PackagesPort`）：`packages` 这件工具经它看，`package` 经它装、卸（施工 F-10 下，
//! 同 `package.install`、`package.remove`，不查连接是不是本机的：权限策略照会话的级别问过人了）。照协议那几样回应算（`package.list`、
//! `package.info`、`package.install` 带 `preview` 的），名字、说明、写错的那一句照英文：她看的字是英文。拿着核心的弱引用：
//! 会话活在核心里面，拿强的会绕成环。

use std::path::PathBuf;
use std::sync::{Arc, Weak};

use serde_json::{Value, json};

use miyu_tool::{Looking, PackageConflict, PackageRefusal, PackagesPort};

use super::local::InfoParams;
use miyu_policy::preset::Unlisted;

use super::TARGET;
use crate::Core;
use crate::hello::Peer;
use crate::refusal::{Locale, Refusal};

/// 交给会话的那一份。
pub(crate) fn port(core: &Arc<Core>) -> Arc<dyn PackagesPort> {
    Arc::new(Port(Arc::downgrade(core)))
}

/// 核心那一头。
struct Port(Weak<Core>);

impl Port {
    /// 核心还在；正在停的拒 `shutting_down`。
    fn core(&self) -> Result<Arc<Core>, PackageRefusal> {
        self.0.upgrade().ok_or_else(|| PackageRefusal {
            reason: "shutting_down".to_string(),
            problem: None,
            line: None,
            conflicts: Vec::new(),
        })
    }
}

/// 照英文说：名字、说明、清单哪里不对。
fn english() -> Peer {
    Peer {
        locale: Locale::En,
        language: "en",
        input: false,
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
                        package: field(one, "package"),
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
            let listed = super::list(&core, english()).map_err(refused)?;
            Ok(listed["packages"].clone())
        })
    }

    fn info(&self, package: String) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
            let params: InfoParams = serde_json::from_value(json!({ "package": package }))
                .map_err(|_| refused(Refusal::BAD_PARAMS))?;
            let mut info = super::local::info(&core, params).await.map_err(refused)?;
            if let Some(entry) = super::entry(&core, &package, english()) {
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
            let core = self.core()?;
            super::preview::installing(&core, english(), &path)
                .await
                .map_err(refused)
        })
    }

    fn install(&self, path: PathBuf, preset: Option<String>) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
            let params = serde_json::from_value(json!({ "path": path }))
                .map_err(|_| refused(Refusal::BAD_PARAMS))?;
            let entry = super::manage::install(&core, english(), params)
                .await
                .map_err(refused)?;
            if let (Some(preset), Some(package)) = (preset, entry["package"].as_str()) {
                open_in(&core, &preset, package).await;
            }
            Ok(entry)
        })
    }

    fn enabling(&self, package: String) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
            super::preview::enabling(&core, english(), &package).map_err(refused)
        })
    }

    fn enable(&self, package: String) -> Looking<'_> {
        Box::pin(async move {
            let core = self.core()?;
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
            super::switch::enable(&core, english(), params)
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
}

/// 在会话用的预设 `preset` 里打开她装的包 `package`（施工 F-10 下下，16 第三节）：预设不开没列出来的软件、也没写这个包的，往
/// 家目录那一层的同名覆盖里写 `software.<编号> = true`；开着的、写过的（人关过的照人的）不动。写不成的记一行，装照样算成。
async fn open_in(core: &Arc<Core>, preset: &str, package: &str) {
    let Ok(found) = crate::presets::presets(core).find(preset) else {
        return;
    };
    if found.file.unlisted() == Unlisted::On || found.file.software.contains_key(package) {
        return;
    }
    let change = json!({ "preset": preset, "changes": [{ "key": format!("software.{package}"), "value": true }] });
    let Ok(params) = serde_json::from_value(change) else {
        return;
    };
    if let Err(refusal) = crate::presets::write::set(core, english(), params).await {
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
        let core = self.core()?;
        let params = serde_json::from_value(json!({ "package": package, "preview": preview }))
            .map_err(|_| refused(Refusal::BAD_PARAMS))?;
        super::manage::remove(&core, params).await.map_err(refused)
    }
}
