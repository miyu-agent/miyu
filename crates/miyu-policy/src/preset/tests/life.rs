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
