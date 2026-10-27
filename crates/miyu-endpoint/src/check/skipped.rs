//! 跳过的脚本、技能（施工 X-12 下，`docs/blueprint/skills.md`「脚本」第 2 条）：照这一刻装了的包和管理员家目录再找一遍，
//! 开头写错的、读不了的各一条警告，`message` 带上为什么（英文一句：给作者看的，同编译器的报错）；同名被盖住的说被谁盖住
//! （`script_covered`、`skill_covered`，施工 X-13 中，设计 `31-软件包.md` 第五节第 3 条）；找到了、却因为撞了别的工具的名字
//! 没登记上的脚本另报一条，照核心上一次再找时记下的分清撞名（`script_clashed`）、造不成工具（`script_not_made`）
//! 和核心还没读到（`script_not_loaded`，施工 X-46）。斜杠命令模板同样报（施工 X-14 上补，`slash.md`）：`slash_skipped`、`slash_covered`，和核心内置的
//! 同名、用不上的 `slash_builtin`。链接指到数据根里不认的地方的（施工 X-37）：`script_link_outside`、`skill_link_outside`、
//! `slash_link_outside`，说指到了哪一片。有脚本状态目录、这一刻登记上的脚本里却没有它的（施工 X-44）：`script_state_left`，带
//! 这个目录，人自己删。

use std::path::Path;

use serde_json::{Value, json};

use miyu_store::human::Human;
use miyu_store::skills::Skipped;

use super::shown;
use crate::Core;
use crate::scripts::{Missed, Why};

/// 几条警告：`kind` 是 `script`、`skill`、`slash`、`persona`、`preset`。
pub(super) async fn skipped(core: &Core, words: &Human) -> Vec<Value> {
    let (packages, root, admin) = (
        core.system_packages(),
        core.root.clone(),
        core.admin.clone(),
    );
    let found = tokio::task::spawn_blocking(move || {
        let scripts = miyu_store::scripts::find(&packages, &root, &admin);
        let skills = miyu_store::skills::find(&packages, &root, &admin).1;
        let slash = miyu_store::slash::find(&packages, &root, &admin);
        let states = miyu_store::scripts::states(root.path());
        (scripts, skills, slash, states)
    })
    .await;
    let Ok(((scripts, skipped_scripts), skipped_skills, (slash, skipped_slash), states)) = found
    else {
        return Vec::new();
    };
    let warn = |kind: &str, code: &str, path: &Path, detail: &str| {
        let message = crate::packages::sentence(words, code, detail).unwrap_or_default();
        json!({"kind": kind, "file": shown(core, path), "code": code, "level": "warning", "message": message})
    };
    let one = |kind: &str, skipped: &Skipped| match (&skipped.by, &skipped.outside) {
        (Some(by), _) => warn(
            kind,
            &format!("{kind}_covered"),
            &skipped.path,
            &shown(core, by),
        ),
        (None, Some(outside)) => warn(
            kind,
            &format!("{kind}_link_outside"),
            &skipped.path,
            &shown(core, outside),
        ),
        (None, None) => warn(
            kind,
            &format!("{kind}_skipped"),
            &skipped.path,
            &skipped.why,
        ),
    };
    let (registered, missed) = (core.scripts(), core.missed_scripts());
    let mut problems: Vec<Value> = skipped_scripts
        .iter()
        .map(|skipped| one("script", skipped))
        .collect();
    problems.extend(
        scripts
            .iter()
            .filter(|script| !registered.iter().any(|on| on.head.name == script.head.name))
            .map(
                |script| match missed.iter().find(|one| one.file == script.file) {
                    Some(Missed {
                        why: Why::Clashed, ..
                    }) => warn("script", "script_clashed", &script.file, &script.head.name),
                    Some(Missed {
                        why: Why::NotMade(why),
                        ..
                    }) => warn("script", "script_not_made", &script.file, why),
                    None => warn(
                        "script",
                        "script_not_loaded",
                        &script.file,
                        &script.head.name,
                    ),
                },
            ),
    );
    problems.extend(
        states
            .iter()
            .filter(|state| {
                !registered.iter().any(|on| {
                    on.head.name == state.tool && crate::scripts::package(on) == state.source
                })
            })
            .map(|state| {
                let detail = format!("{}/{}", state.source, state.tool);
                warn("script", "script_state_left", &state.path, &detail)
            }),
    );
    problems.extend(skipped_skills.iter().map(|skipped| one("skill", skipped)));
    problems.extend(skipped_slash.iter().map(|skipped| one("slash", skipped)));
    problems.extend(
        slash
            .iter()
            .filter(|template| core.commands.find_present(core, &template.name).is_some())
            .map(|template| warn("slash", "slash_builtin", &template.file, &template.name)),
    );
    // 包里带的人格、预设被出厂的、排在前面的包盖住了（施工 P-11 上）。
    let (personas, presets) = (
        crate::personas::personas(core, &core.admin),
        crate::presets::presets(core, &core.admin),
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
    // 卸了的包留下的人格、预设的改动（施工 P-11 下）。
    problems.extend(crate::packages::left::warnings(core, words).await);
    // 扩展声明的命令、整页的命令名（施工 X-14 下、X-15 上）和内置的同名：报在清单上。
    for found in core.system_packages().iter() {
        let Ok(manifest) = &found.read else {
            continue;
        };
        problems.extend(
            manifest
                .slash_names()
                .filter(|name| core.commands.find_present(core, name).is_some())
                .map(|name| warn("slash", "slash_builtin", &found.path, name)),
        );
    }
    problems
}
