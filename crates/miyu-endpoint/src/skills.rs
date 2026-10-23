//! `skill.list`（施工 X-26，`docs/blueprint/skills.md`「技能」，网页设置页「技能」那一页）：列出核心这一刻用得上的技能。
//!
//! 找法同造会话（`miyu_store::skills::find`）：管理员家目录里放的、装了的软件包带的，同名的照技能的先后规矩只留核心实际用的
//! 那一份（自己放的赢，包之间出厂的先于家目录、同一层照编号），照名字排。一项：
//!
//! - `name`；`display_name`、`summary` 照连接的语言挑（同 `package.list`，这种语言没写的照英文、中文、日文的先后），一种都没写的
//!   不带；
//! - `from`：`home`（管理员自己放的）、`shipped`（出厂的包带的）、`package`（管理员装的包带的）；随包来的另带 `package` 编号；
//! - `path`：`SKILL.md` 在哪，给人看的写法（家目录下的写成 `~/…`）；
//! - `on`：技能、脚本、MCP 三页的全局开关这时开不开（施工 X-27，`features.skill-<名字>`），没写的算开。
//!
//! 碰磁盘，在阻塞线程里找。

mod remove;

use miyu_kernel::id::AccountId;
use serde_json::{Value, json};

use miyu_policy::skill::feature_id;
use miyu_session::switches::on;
use miyu_store::packages::{Found, Layer};
use miyu_store::skills::Skill;

use crate::Core;
use crate::config::project::shown;
use crate::hello::Peer;
use crate::personas::pick;
use crate::refusal::Refusal;
pub(crate) use remove::remove;

/// `skill.list`：`{"skills": [...], "dir"}`，照名字排；`dir` 是调用者 `account` 自己放技能的文件夹（施工 X-27 补；施工 Y-2 中上
/// 起照连接的账号：自己的、系统区的、包里的，原来是管理员的）。
///
/// # Errors
///
/// 找的线程崩了：内部出错。
pub(crate) async fn list(core: &Core, account: &AccountId, peer: Peer) -> Result<Value, Refusal> {
    let (packages, root, admin) = (core.packages(), core.root.clone(), account.clone());
    let found = tokio::task::spawn_blocking(move || {
        let found = miyu_store::skills::find(&packages, &root, &admin).0;
        (found, packages)
    })
    .await
    .map_err(|_| Refusal::INTERNAL)?;
    let (found, packages) = found;
    let switches = crate::presets::switches(core);
    let skills: Vec<Value> = found
        .iter()
        .map(|skill| {
            let mut one = one(core, &packages, skill, peer.language);
            one["on"] = json!(on(&switches, &feature_id(&skill.head.name)));
            one
        })
        .collect();
    let dir = miyu_store::skills::own_dir(&core.root, account);
    Ok(json!({ "skills": skills, "dir": shown(&dir, core.home.as_deref()) }))
}

/// 一项。
fn one(core: &Core, packages: &[Found], skill: &Skill, language: &str) -> Value {
    let mut one = json!({
        "name": skill.head.name,
        "path": shown(&skill.file, core.home.as_deref()),
    });
    from(&mut one, packages, skill.package.as_deref());
    if let Some(name) = pick(&skill.head.display_name, language) {
        one["display_name"] = json!(name);
    }
    if let Some(summary) = pick(&skill.head.summary, language) {
        one["summary"] = json!(summary);
    }
    one
}

/// 填上 `from`（技能、脚本共用，施工 X-27 的 `script.list` 也照它）：自己放的（没有包 `package`）是 `home`；随包来的照装了的包
/// `packages` 里那个包在哪一层，出厂的是 `shipped`、别的是 `package`，另带 `package` 编号。
pub(crate) fn from(one: &mut Value, packages: &[Found], package: Option<&str>) {
    let Some(id) = package else {
        one["from"] = json!("home");
        return;
    };
    let layer = packages
        .iter()
        .find(|found| found.id == id)
        .map(|found| found.layer);
    one["from"] = json!(match layer {
        Some(Layer::Shipped) => "shipped",
        _ => "package",
    });
    one["package"] = json!(id);
}
