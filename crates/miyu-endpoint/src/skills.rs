//! `skill.list`（施工 X-26，`docs/blueprint/skills.md`「技能」，网页设置页「技能」那一页）：列出核心这一刻用得上的技能。
//!
//! 找法同造会话（`miyu_store::skills::find`）：管理员家目录里放的、装了的软件包带的，同名的照技能的先后规矩只留核心实际用的
//! 那一份（自己放的赢，包之间出厂的先于家目录、同一层照编号），照名字排。一项：
//!
//! - `name`；`display_name`、`summary` 照连接的语言挑（同 `package.list`，这种语言没写的照英文、中文、日文的先后），一种都没写的
//!   不带；
//! - `from`：`home`（管理员自己放的）、`shipped`（出厂的包带的）、`package`（管理员装的包带的）；随包来的另带 `package` 编号；
//! - `path`：`SKILL.md` 在哪，给人看的写法（家目录下的写成 `~/…`）。
//!
//! 碰磁盘，在阻塞线程里找。

use serde_json::{Value, json};

use miyu_store::packages::{Found, Layer};
use miyu_store::skills::Skill;

use crate::Core;
use crate::config::project::shown;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;

/// `skill.list`：`{"skills": [...]}`，照名字排。
///
/// # Errors
///
/// 找的线程崩了：内部出错。
pub(crate) async fn list(core: &Core, peer: Peer) -> Result<Value, Refusal> {
    let (packages, root, admin) = (core.packages(), core.root.clone(), core.admin.clone());
    let found = tokio::task::spawn_blocking(move || {
        let found = miyu_store::skills::find(&packages, &root, &admin).0;
        (found, packages)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let (found, packages) = found;
    let skills: Vec<Value> = found
        .iter()
        .map(|skill| one(core, &packages, skill, peer.language))
        .collect();
    Ok(json!({ "skills": skills }))
}

/// 一项。
fn one(core: &Core, packages: &[Found], skill: &Skill, language: &str) -> Value {
    let mut one = json!({
        "name": skill.head.name,
        "from": "home",
        "path": shown(&skill.file, core.home.as_deref()),
    });
    if let Some(id) = &skill.package {
        let layer = packages
            .iter()
            .find(|found| &found.id == id)
            .map(|found| found.layer);
        one["from"] = json!(match layer {
            Some(Layer::Shipped) => "shipped",
            _ => "package",
        });
        one["package"] = json!(id);
    }
    if let Some(name) = pick(&skill.head.display_name, language) {
        one["display_name"] = json!(name);
    }
    if let Some(summary) = pick(&skill.head.summary, language) {
        one["summary"] = json!(summary);
    }
    one
}
