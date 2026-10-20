//! 全局开关（施工 X-27）：预设明写的先，没写的照全局开关，都没写的照 `unlisted`；只认三类功能；不进指纹。

use super::*;

/// 一份预设：`unlisted` 照 `on`，`[features]` 写了 `features`，`[software]` 写了 `software`，带上全局开关 `global`。
fn file(
    unlisted: Unlisted,
    features: &[(&str, bool)],
    software: &[(&str, bool)],
    global: &[(&str, bool)],
) -> PresetFile {
    let map = |pairs: &[(&str, bool)]| -> BTreeMap<String, bool> {
        pairs.iter().map(|(k, v)| (k.to_string(), *v)).collect()
    };
    PresetFile {
        unlisted: Some(unlisted),
        features: map(features),
        software: map(software),
        ..PresetFile::default()
    }
    .following(map(global))
}

#[test]
fn the_preset_beats_the_global_switch_which_beats_unlisted() {
    let preset = file(
        Unlisted::On,
        &[("skill-weather", true)],
        &[("kit", false)],
        &[
            ("skill-weather", false),
            ("script-echo", false),
            ("mcp-github", false),
            ("skill-kit-notes", true),
        ],
    );
    assert_eq!(preset.decided("skill-weather", ""), (true, By::Preset));
    assert_eq!(
        preset.decided("skill-kit-notes", "kit"),
        (false, By::Preset),
        "包写在 [software] 里的也算明写"
    );
    assert_eq!(preset.decided("script-echo", "~home"), (false, By::Global));
    assert_eq!(preset.decided("mcp-github", "~home"), (false, By::Global));
    assert_eq!(preset.decided("mcp-other", "~home"), (true, By::Default));
    assert!(!preset.opens_in("script-echo", "~home"));
}

/// 全局开关只管默认开的预设（2026-10-20 项目主人定）：`unlisted = "off"` 的，全局开关打开的照旧关着，明写的照预设。
#[test]
fn an_off_preset_ignores_the_global_switch() {
    let preset = file(
        Unlisted::Off,
        &[("script-echo", true)],
        &[],
        &[("skill-weather", true), ("script-echo", false)],
    );
    assert_eq!(preset.decided("skill-weather", ""), (false, By::Default));
    assert_eq!(preset.decided("skill-notes", ""), (false, By::Default));
    assert_eq!(preset.decided("script-echo", "~home"), (true, By::Preset));
}

#[test]
fn only_skills_scripts_and_mcp_servers_follow_the_switch() {
    let preset = file(Unlisted::On, &[], &[], &[("memory", false), ("web", false)]);
    assert_eq!(preset.decided("memory", "memory"), (true, By::Default));
    assert!(switchable("skill-a") && switchable("script-a") && switchable("mcp-a"));
    assert!(!switchable("memory") && !switchable("command-a"));
}

#[test]
fn switches_stay_out_of_the_digest() {
    let plain = file(Unlisted::On, &[], &[], &[]);
    let switched = file(Unlisted::On, &[], &[], &[("skill-weather", false)]);
    assert_eq!(plain.digest(), switched.digest());
}
