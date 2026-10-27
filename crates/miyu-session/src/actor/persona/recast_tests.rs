//! 人中途换的人格、预设（施工 P-13，`recast.rs`）：换预设的照新的重新筛工具面、钉上新的编号，记忆照开会话时的；换人格的照新
//! 人格拼（换成无人格的没有人设），交回要挪记忆的那一份；新人格找不着的照旧；子会话不换。

use super::recast::Wanted;
use super::test_support::{names, setup};
use super::*;

/// 照全是默认值的配置、要换成的 `wanted` 看一遍。
fn look_wanting(refresh: &Refresh, wanted: Wanted) -> Seen {
    look(
        refresh,
        &Values::default(),
        &refresh.tools.edition(),
        (Default::default(), None, wanted, false),
    )
}

#[test]
fn a_wanted_preset_refilters_the_face_and_keeps_memory() {
    let (_scratch, root, refresh) = setup("recast-preset", "");
    let dir = root.path().join("home/alice/presets");
    std::fs::write(dir.join("q.toml"), "[tools]\nshell = false\n").expect("写得进");
    let wanted = Wanted {
        persona: None,
        preset: Some("q".to_string()),
    };
    let Seen::Swapped(snapshot, _, _, _, recast) = look_wanting(&refresh, wanted) else {
        panic!("要换");
    };
    assert_eq!(names(&snapshot), ["read", "remember"]);
    assert_eq!(
        snapshot.preset.as_ref().map(|pin| pin.id.as_str()),
        Some("q")
    );
    assert_eq!(snapshot.memory_scope(), refresh.snapshot.memory_scope());
    assert!(recast.is_none(), "人格没换，记忆不挪");
    let missing = Wanted {
        persona: None,
        preset: Some("gone".to_string()),
    };
    assert!(matches!(
        look_wanting(&refresh, missing),
        Seen::Unreadable(_)
    ));
}

#[test]
fn a_wanted_persona_rebuilds_the_snapshot_and_asks_to_move_memory() {
    let (_scratch, _root, refresh) = setup("recast-persona", "");
    assert!(refresh.snapshot.system.starts_with("You are Miyu."));
    let wanted = Wanted {
        persona: Some(None),
        preset: None,
    };
    let Seen::Swapped(snapshot, _, _, _, recast) = look_wanting(&refresh, wanted) else {
        panic!("要换");
    };
    assert_eq!(snapshot.persona, None);
    assert!(
        !snapshot.system.contains("You are Miyu."),
        "{}",
        snapshot.system
    );
    assert_eq!(snapshot.preset, refresh.snapshot.preset, "预设照旧");
    assert!(recast.is_some(), "换了人格要挪记忆");
    let missing = Wanted {
        persona: Some(Some("nobody".to_string())),
        preset: None,
    };
    assert!(
        matches!(look_wanting(&refresh, missing), Seen::Kept(_)),
        "找不着的照旧"
    );
}

#[test]
fn a_persona_with_the_same_words_still_swaps() {
    let (_scratch, root, refresh) = setup("recast-twin", "");
    let dir = root.path().join("home/alice/personas/twin/prompts");
    std::fs::create_dir_all(&dir).expect("建得了");
    std::fs::write(dir.join("persona.md"), "You are Miyu.\n").expect("写得进");
    let wanted = Wanted {
        persona: Some(Some("twin".to_string())),
        preset: None,
    };
    let Seen::Swapped(snapshot, ..) = look_wanting(&refresh, wanted) else {
        panic!("字一样也换：记忆要挪");
    };
    assert_eq!(snapshot.persona.as_deref(), Some("twin"));
}
