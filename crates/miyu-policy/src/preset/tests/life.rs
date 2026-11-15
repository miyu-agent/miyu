//! 只给人生会话的包（施工 LF-4 下，`life.md`「结束」，`presets.md` 讲 `needs` 的那一条）：不是没结束的人生会话的，清单写了
//! `needs = ["life"]` 的包整包扣下：功能不开、工具不留、不算进没开的那几个；是的照预设。指纹、快照记的都不变；接在
//! 「显示不了图片」后面，两样一起扣。

use super::*;

/// 装了的几个功能加上人生那一个（包 `base-life`，只给人生会话）。
fn with_life() -> Features {
    let mut list: Vec<crate::features::Feature> = super::installed().iter().cloned().collect();
    list.push(crate::features::Feature {
        id: "life".to_string(),
        package: "base-life".to_string(),
        tools: vec!["end_life".to_string()],
        service: None,
        script: None,
    });
    Features::new(list).needing_life(["base-life".to_string()])
}

#[test]
fn only_a_life_session_gets_the_packages_that_need_life() {
    let features = with_life();
    let file = read("[features]\nmemory = false\n").unwrap();
    let plain = Chosen::new("full".to_string(), file.clone(), &features);
    let living = plain.clone().living(&features, true);
    let other = plain.clone().living(&features, false);
    assert!(
        living.file.opens_in("life", "base-life"),
        "人生会话照预设开"
    );
    assert!(living.file.keeps("life", "base-life", "end_life"));
    assert_eq!(living.off, ["memory"]);
    assert!(!other.file.opens_in("life", "base-life"), "别的会话扣下");
    assert!(
        !other.file.keeps("life", "base-life", "end_life"),
        "工具不留"
    );
    assert_eq!(other.off, ["memory"], "扣下的不算进没开的");
    assert!(other.file.opens_in("files", "basesystem"), "别的包照旧");
    assert_eq!(other.pin(), plain.pin(), "快照记的不变");
    assert_eq!(living.pin(), plain.pin());
}

#[test]
fn a_preset_that_turns_life_off_names_it_only_in_a_life_session() {
    let features = with_life();
    let file = read("[preset]\nunlisted = \"off\"\n\n[software]\nbasesystem = true\n").unwrap();
    let chosen = Chosen::new("lean".to_string(), file, &features);
    assert!(
        chosen
            .clone()
            .living(&features, true)
            .off
            .contains(&"life".to_string()),
        "人生会话里没开的照算"
    );
    assert!(
        !chosen
            .living(&features, false)
            .off
            .contains(&"life".to_string()),
        "别的会话里换了预设也没有，不算"
    );
}

#[test]
fn life_and_images_are_withheld_together() {
    let features = with_life().needing_images(["onebot".to_string()]);
    let file = read("").unwrap();
    let blind = Chosen::new("full".to_string(), file, &features)
        .situated(&features, false, "local")
        .living(&features, false);
    assert!(!blind.file.opens_in("qq", "onebot"), "要图的照旧扣下");
    assert!(!blind.file.opens_in("life", "base-life"), "人生的也扣下");
    assert!(blind.off.is_empty(), "{:?}", blind.off);
}

/// 人生那个包的两个功能（施工 LF-11）：`life`（`end_life`）、`change`（`note_change`）。
fn with_change() -> Features {
    let mut list: Vec<crate::features::Feature> = super::installed().iter().cloned().collect();
    for (id, tool) in [("life", "end_life"), ("change", "note_change")] {
        list.push(crate::features::Feature {
            id: id.to_string(),
            package: "base-life".to_string(),
            tools: vec![tool.to_string()],
            service: None,
            script: None,
        });
    }
    Features::new(list).needing_life(["base-life".to_string()])
}

/// 施工 LF-11 验收第 3 条：`note_change` 同 `end_life` 只在没结束的人生会话里有（结束了的、普通会话 `living` 是假，整包扣下）；
/// 预设里单关功能 `change` 的人生会话里没有它、`end_life` 还在，`change` 算进没开的。
#[test]
fn note_change_lives_with_end_life_and_can_be_turned_off_alone() {
    let features = with_change();
    let all = Chosen::new("full".to_string(), read("").unwrap(), &features);
    assert!(
        all.clone()
            .living(&features, true)
            .file
            .keeps("change", "base-life", "note_change")
    );
    let ended = all.living(&features, false);
    assert!(
        !ended.file.keeps("change", "base-life", "note_change"),
        "结束了的、普通会话没有"
    );
    assert!(!ended.file.keeps("life", "base-life", "end_life"));
    assert!(ended.off.is_empty(), "{:?}", ended.off);
    let file = read("[features]\nchange = false\n").unwrap();
    let off = Chosen::new("life".to_string(), file, &features).living(&features, true);
    assert!(
        !off.file.keeps("change", "base-life", "note_change"),
        "单关了"
    );
    assert!(
        off.file.keeps("life", "base-life", "end_life"),
        "end_life 还在"
    );
    assert_eq!(off.off, ["change"]);
}
