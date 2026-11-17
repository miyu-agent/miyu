//! `script.list`（施工 X-27，`docs/blueprint/skills.md`「脚本」，网页设置页「脚本」那一页）：列出这一刻登记上的脚本，照工具名排。
//! 一项：
//!
//! - `tool`：工具名；`display_name`、`description` 是给人看的那一槽（脚本开头的 `Display name`、`显示名称`、`描述`），照连接的
//!   语言挑（同 `skill.list`），一种都没写的不带；
//! - `from`、`package`、`path`：同 `skill.list`，`path` 是脚本文件；
//! - `timeout`：一次等多少秒；
//! - `on`：技能、脚本、MCP 三页的全局开关这时开不开（`features.script-<工具名>`），没写的算开。

use serde_json::{Value, json};

use miyu_policy::script::feature_id;
use miyu_session::switches::on;
use miyu_store::scripts::Script;

use crate::Core;
use crate::config::project::shown;
use crate::hello::Peer;
use crate::personas::pick;

/// `script.list`：`{"scripts": [...], "dir"}`；`dir` 是你放脚本的文件夹（施工 X-27 补）。列的是登记进工具目录的那一份。
pub(crate) async fn list(core: &Core, peer: Peer) -> Value {
    let values = core.config().resolved().values();
    let switches = miyu_session::switches::of(&values);
    let packages = core.packages_of();
    // 登记上的脚本照工具名排（`Core::scripts`）。
    let scripts: Vec<Script> = core.scripts().to_vec();
    let listed: Vec<Value> = scripts
        .iter()
        .map(|script: &Script| {
            let name = &script.head.name;
            let mut one = json!({
                "tool": name,
                "path": shown(&script.file, core.home.as_deref()),
                "timeout": script.head.timeout,
                "on": on(&switches, &feature_id(name)),
            });
            crate::skills::from(&mut one, &packages, script.package.as_deref());
            if let Some(display) = pick(&script.head.display_name, peer.language) {
                one["display_name"] = json!(display);
            }
            if let Some(summary) = pick(&script.head.summary, peer.language) {
                one["description"] = json!(summary);
            }
            one
        })
        .collect();
    let dir = miyu_store::scripts::own_dir(&core.root);
    json!({ "scripts": listed, "dir": shown(&dir, core.home.as_deref()) })
}
