//! 软件包列表里的一项（`package.list`、`package.changed` 的 `entry`、装卸开关的回应共用，施工 U23 下上从 `packages.rs` 挪出来：
//! 那一份到了 500 行）。

use serde_json::{Value, json};

use miyu_config::Words;
use miyu_kernel::id::AccountId;
use miyu_store::human::Human;
use miyu_store::packages::{Found, Issue, Packages};

use super::{depends, fill, mismatch, sentence, services, status};
use crate::Core;

/// 一项。`removed` 是卸掉了的出厂的包（带 `removed: true`）。依赖缺没缺照 `installed`（看的人看得到的那些）认，每人一份的扩展的
/// 状态照看的人 `viewer` 那一份（[`status::status`]）。
pub(super) fn listed(
    core: &Core,
    found: &Found,
    (places, installed, viewer): (&Packages, &[Found], Option<&AccountId>),
    words: &Human,
    language: &str,
    removed: bool,
) -> Value {
    let mut item = json!({"package": found.id, "layer": found.layer.as_str()});
    if removed {
        item["removed"] = json!(true);
    }
    match &found.read {
        Ok(manifest) => {
            fill::fill(&mut item, &found.id, manifest, language);
            item["from"] = json!(fill::from(found, places).as_str());
            services::fill(core, &found.id, manifest, &mut item);
            // 施工 F-6 上（`package-pages.md`）：状态、开关、后台页。
            item["status"] = json!(status::status(core, found, (manifest, viewer), removed));
            // 依赖的包缺了（施工 F-11）：不起，写明缺谁。
            let missing = depends::missing(&installed.iter().collect::<Vec<_>>(), manifest);
            if !removed && !missing.is_empty() {
                item["status"] = json!("depends_missing");
                item["missing"] = json!(missing);
            }
            let shipped = found.layer == miyu_store::packages::Layer::Shipped;
            if let Some(on) = status::enabled(core, &found.id, manifest, shipped, removed) {
                item["enabled"] = json!(on);
            }
            // 包自带的图标（施工 F-18 上）：合规矩的带版本，图用 `package.logo` 读；没写、不合的不带。
            if let Some(logo) = super::logo::of(found, manifest) {
                item["logo"] = json!(logo.version);
            }
            if status::page(found, manifest) {
                item["page"] = json!(true);
            }
            item["state"] = json!(places.state_dir(&found.id).to_string_lossy());
            // 装的时刻（施工 PG-1：预设页照装的先后排）：本地库只记装进家目录的，出厂的没有。
            if let Some(at) =
                miyu_store::packages::local::installed(&places.local_root(), &found.id)
            {
                item["installed_at"] = json!(at);
            }
            if let Some(range) = mismatch(manifest) {
                item["code"] = json!("protocol_mismatch");
                item["problem"] = json!(sentence(words, "protocol_mismatch", &range));
            }
        }
        Err(Issue::Wrong(problem)) => {
            item["code"] = json!(problem.code.as_str());
            item["problem"] = json!(
                sentence(words, problem.code.as_str(), &problem.detail)
                    .unwrap_or_else(|| problem.message.clone())
            );
            if let Some(line) = problem.line {
                item["line"] = json!(line);
            }
        }
        Err(Issue::Unreadable(error)) => {
            item["code"] = json!("unreadable");
            item["problem"] = json!(
                Words::sentence(words, "config/unreadable", &[("why", &error.to_string())])
                    .unwrap_or_else(|| error.to_string())
            );
        }
    }
    item
}
