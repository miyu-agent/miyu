//! 跳过的脚本、技能（施工 X-12 下，`docs/blueprint/skills.md`「脚本」第 2 条）：照这一刻装了的包和管理员家目录再找一遍，
//! 开头写错的、读不了的、说明加参数超了预算的、同名被压掉的，各一条警告，`message` 带上为什么（英文一句：给作者看的，同
//! 编译器的报错）；找到了、却因为撞了别的工具的名字没登记上的脚本另报一条。

use std::path::Path;

use serde_json::{Value, json};

use miyu_store::human::Human;
use miyu_store::skills::Skipped;

use super::shown;
use crate::Core;

/// 几条警告：`kind` 是 `script`、`skill`。
pub(super) async fn skipped(core: &Core, words: &Human) -> Vec<Value> {
    let (packages, root, admin) = (core.packages(), core.root.clone(), core.admin.clone());
    let found = tokio::task::spawn_blocking(move || {
        let scripts = miyu_store::scripts::find(&packages, &root, &admin);
        let skills = miyu_store::skills::find(&packages, &root, &admin).1;
        (scripts, skills)
    })
    .await;
    let Ok(((scripts, skipped_scripts), skipped_skills)) = found else {
        return Vec::new();
    };
    let warn = |kind: &str, code: &str, path: &Path, detail: &str| {
        let message = crate::packages::sentence(words, code, detail).unwrap_or_default();
        json!({"kind": kind, "file": shown(core, path), "code": code, "level": "warning", "message": message})
    };
    let one =
        |kind: &str, code: &str, skipped: &Skipped| warn(kind, code, &skipped.path, &skipped.why);
    let registered = core.scripts();
    let mut problems: Vec<Value> = skipped_scripts
        .iter()
        .map(|skipped| one("script", "script_skipped", skipped))
        .collect();
    problems.extend(
        scripts
            .iter()
            .filter(|script| !registered.iter().any(|on| on.head.name == script.head.name))
            .map(|script| warn("script", "script_clashed", &script.file, &script.head.name)),
    );
    problems.extend(
        skipped_skills
            .iter()
            .map(|skipped| one("skill", "skill_skipped", skipped)),
    );
    problems
}
