//! `persona.toml` 里 `[persona]` 以外的几张表（`[memory]`、`[knowledge]`、`[appearance]`）读得出、上一层盖下面的（施工 K-1 下从
//! `persona/tests.rs` 挪出来守 500 行上限）。写错了说第几行的在 `persona/tests.rs` 的 `a_wrong_toml_says_which_line`。

use crate::memory::MemoryScope;
use crate::persona::{PersonaFile, color, read_toml};

#[test]
fn the_knowledge_bases_narrow_and_an_upper_layer_wins() {
    // 施工 K-1 下（19 N11）：列了的只查那几个，空的列表一个都不查，没写的查全部。
    let read = |text: &str| read_toml(text).unwrap().knowledge;
    assert_eq!(
        read("[knowledge]\nbases = [\"notes\", \"world-2\"]\n"),
        Some(vec!["notes".to_string(), "world-2".to_string()])
    );
    assert_eq!(read("[knowledge]\nbases = []\n"), Some(Vec::new()));
    assert_eq!(read("[knowledge]\n"), None, "不写是没有，查全部");
    let lower = read_toml("[knowledge]\nbases = [\"notes\"]\n").unwrap();
    assert_eq!(
        PersonaFile::default().over(lower.clone()).knowledge,
        Some(vec!["notes".to_string()]),
        "上面没写的沿用下面的"
    );
    let upper = read_toml("[knowledge]\nbases = []\n").unwrap();
    assert_eq!(
        upper.over(lower).knowledge,
        Some(Vec::new()),
        "写了的整格换掉"
    );
}

#[test]
fn the_memory_scope_is_persona_or_session_and_an_upper_layer_wins() {
    let read = |text: &str| read_toml(text).unwrap().memory;
    assert_eq!(
        read("[memory]\nscope = \"persona\"\n"),
        Some(MemoryScope::Persona)
    );
    assert_eq!(
        read("[memory]\nscope = \"session\"\n"),
        Some(MemoryScope::Session)
    );
    assert_eq!(read("[memory]\n"), None, "不写是没有，照 persona 算");
    let lower = read_toml("[memory]\nscope = \"session\"\n").unwrap();
    assert_eq!(
        PersonaFile::default().over(lower.clone()).memory,
        Some(MemoryScope::Session),
        "上一层没写沿用下面的"
    );
    let upper = read_toml("[memory]\nscope = \"persona\"\n").unwrap();
    assert_eq!(upper.over(lower).memory, Some(MemoryScope::Persona));
}

/// 主题色（施工 P-6）：`#rrggbb` 大小写都认、读成小写；上一层写了的盖下面的，没写沿用下面的。
#[test]
fn the_seed_is_a_lowercase_color_and_an_upper_layer_wins() {
    let read = |text: &str| read_toml(text).unwrap().seed;
    assert_eq!(
        read("[appearance]\nseed = \"#3368C0\"\n").as_deref(),
        Some("#3368c0")
    );
    assert_eq!(read("[appearance]\n"), None, "不写是没有，头从头像取色");
    let lower = read_toml("[appearance]\nseed = \"#112233\"\n").unwrap();
    assert_eq!(
        PersonaFile::default().over(lower.clone()).seed.as_deref(),
        Some("#112233"),
        "上一层没写沿用下面的"
    );
    let upper = read_toml("[appearance]\nseed = \"#abcdef\"\n").unwrap();
    assert_eq!(upper.over(lower).seed.as_deref(), Some("#abcdef"));
    for wrong in ["3368c0", "#3368c", "#3368c00", "#3368cg", "red"] {
        assert_eq!(color(wrong), None, "{wrong}");
    }
}

/// 背景图模糊多少（施工 P-7 补）：1、100 两头都收，上一层写了的盖下面的、没写的沿用，都没写的是没有（不模糊）。
#[test]
fn background_blur_is_read_and_layered() {
    let lower = read_toml("[appearance]\nbackground_blur = 1\n").unwrap();
    assert_eq!(lower.background_blur, Some(1));
    let upper = read_toml("[appearance]\nbackground_blur = 100\n").unwrap();
    assert_eq!(
        upper.over(lower).background_blur,
        Some(100),
        "上一层的盖下面的"
    );
    let lower = read_toml("[appearance]\nbackground_blur = 30\n").unwrap();
    let upper = read_toml("[appearance]\nseed = \"#3368c0\"\n").unwrap();
    assert_eq!(
        upper.over(lower).background_blur,
        Some(30),
        "没写的沿用下面的"
    );
    assert_eq!(read_toml("").unwrap().background_blur, None, "没写的是没有");
}

/// 背景图露出多少、焦点（施工 P-7）：读成数，上一层写了的盖下面的、没写的沿用；0、100 两头都收。
#[test]
fn background_opacity_and_focus_are_read_and_layered() {
    let lower =
        read_toml("[appearance]\nbackground_opacity = 20\nbackground_focus = [30, 70]\n").unwrap();
    assert_eq!(lower.background_opacity, Some(20));
    assert_eq!(lower.background_focus, Some([30, 70]));
    let upper = read_toml("[appearance]\nbackground_opacity = 0\n").unwrap();
    let both = upper.over(lower);
    assert_eq!(both.background_opacity, Some(0), "上一层的盖下面的");
    assert_eq!(both.background_focus, Some([30, 70]), "没写的沿用下面的");
    let edges =
        read_toml("[appearance]\nbackground_opacity = 100\nbackground_focus = [0, 100]\n").unwrap();
    assert_eq!(
        (edges.background_opacity, edges.background_focus),
        (Some(100), Some([0, 100]))
    );
    assert_eq!(
        read_toml("").unwrap().background_focus,
        None,
        "没写的是没有"
    );
}
