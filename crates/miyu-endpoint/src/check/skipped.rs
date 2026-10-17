//! 跳过的脚本、技能（施工 X-12 下，`docs/blueprint/skills.md`「脚本」第 2 条）：照这一刻装了的包和管理员家目录再找一遍，
//! 开头写错的、读不了的各一条警告，`message` 带上为什么（英文一句：给作者看的，同编译器的报错）；同名被盖住的说被谁盖住
//! （`script_covered`、`skill_covered`，施工 X-13 中，设计 `31-软件包.md` 第五节第 3 条）；找到了、却因为撞了别的工具的名字
//! 没登记上的脚本另报一条。斜杠命令模板同样报（施工 X-14 上补，`slash.md`）：`slash_skipped`、`slash_covered`，和核心内置的
//! 同名、用不上的 `slash_builtin`。

use std::path::Path;

use serde_json::{Value, json};

use miyu_store::human::Human;
use miyu_store::skills::Skipped;

use super::shown;
use crate::Core;

/// 几条警告：`kind` 是 `script`、`skill`、`slash`、`persona`、`preset`。
pub(super) async fn skipped(core: &Core, words: &Human) -> Vec<Value> {
    let (packages, root, admin) = (core.packages(), core.root.clone(), core.admin.clone());
    let found = tokio::task::spawn_blocking(move || {
        let scripts = miyu_store::scripts::find(&packages, &root, &admin);
        let skills = miyu_store::skills::find(&packages, &root, &admin).1;
        let slash = miyu_store::slash::find(&packages, &root, &admin);
        (scripts, skills, slash)
    })
    .await;
    let Ok(((scripts, skipped_scripts), skipped_skills, (slash, skipped_slash))) = found else {
        return Vec::new();
    };
    let warn = |kind: &str, code: &str, path: &Path, detail: &str| {
        let message = crate::packages::sentence(words, code, detail).unwrap_or_default();
        json!({"kind": kind, "file": shown(core, path), "code": code, "level": "warning", "message": message})
    };
    let one = |kind: &str, skipped: &Skipped| match &skipped.by {
        Some(by) => warn(
            kind,
            &format!("{kind}_covered"),
            &skipped.path,
            &shown(core, by),
        ),
        None => warn(
            kind,
            &format!("{kind}_skipped"),
            &skipped.path,
            &skipped.why,
        ),
    };
    let registered = core.scripts();
    let mut problems: Vec<Value> = skipped_scripts
        .iter()
        .map(|skipped| one("script", skipped))
        .collect();
    problems.extend(
        scripts
            .iter()
            .filter(|script| !registered.iter().any(|on| on.head.name == script.head.name))
            .map(|script| warn("script", "script_clashed", &script.file, &script.head.name)),
    );
    problems.extend(skipped_skills.iter().map(|skipped| one("skill", skipped)));
    problems.extend(skipped_slash.iter().map(|skipped| one("slash", skipped)));
    problems.extend(
        slash
            .iter()
            .filter(|template| core.commands.find(&template.name).is_some())
            .map(|template| warn("slash", "slash_builtin", &template.file, &template.name)),
    );
    // 包里带的人格、预设被出厂的、排在前面的包盖住了（施工 P-11 上）。
    let (personas, presets) = (
        crate::personas::personas(core),
        crate::presets::presets(core),
    );
    let covered =
        tokio::task::spawn_blocking(move || (personas.covered(), presets.covered())).await;
    if let Ok((personas, presets)) = covered {
        for (kind, covered) in [("persona", personas), ("preset", presets)] {
            problems.extend(covered.iter().map(|one| {
                warn(
                    kind,
                    &format!("{kind}_covered"),
                    &one.path,
                    &shown(core, &one.by),
                )
            }));
        }
    }
    // 扩展声明的命令、整页的命令名（施工 X-14 下、X-15 上）和内置的同名：报在清单上。
    for found in core.packages().iter() {
        let Ok(manifest) = &found.read else {
            continue;
        };
        problems.extend(
            manifest
                .slash_names()
                .filter(|name| core.commands.find(name).is_some())
                .map(|name| warn("slash", "slash_builtin", &found.path, name)),
        );
    }
    problems
}
