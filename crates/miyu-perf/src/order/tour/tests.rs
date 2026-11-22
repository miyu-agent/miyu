//! 逛一圈的步骤表：每一步都有按键、有等的字，命令先等它出现在输入框里再回车，说的那一句在 `/new` 以后。

use super::*;

fn keyed() -> Vec<(String, &'static [u8], &'static str)> {
    steps()
        .into_iter()
        .filter_map(|step| match step.act {
            Act::Keys { keys, until, .. } => Some((step.name, keys, until)),
            _ => None,
        })
        .collect()
}

#[test]
fn every_step_presses_something_and_waits_for_something() {
    for (name, keys, until) in keyed() {
        assert!(!name.is_empty());
        assert!(!keys.is_empty(), "{name}");
        // 屏幕上的字照去掉空格比（`screen::shows`）：只有空格的等于什么都不等。
        assert!(until.chars().any(|c| !c.is_whitespace()), "{name}");
    }
}

#[test]
fn a_command_is_seen_in_the_input_before_enter() {
    let keyed = keyed();
    for (at, (name, keys, until)) in keyed.iter().enumerate() {
        if keys.first() != Some(&b'/') || keys.len() == 1 {
            continue;
        }
        assert_eq!(keys, &until.as_bytes(), "{name}：打的命令要等它本身");
        assert_eq!(
            keyed.get(at + 1).map(|next| next.1),
            Some(ENTER),
            "{name} 后面要回车"
        );
    }
}

#[test]
fn the_list_covers_what_the_work_order_asks_for() {
    let typed: Vec<&[u8]> = keyed().into_iter().map(|(_, keys, _)| keys).collect();
    for wanted in [
        "/help",
        "/status",
        "/context",
        "/usage",
        "/tool-list",
        "/config",
        "/model",
        "/persona",
        "/preset",
        "/theme",
        "/files",
        "/jump",
        "/sessions",
        "/compact",
        "/new",
        "/undo",
    ] {
        assert!(typed.contains(&wanted.as_bytes()), "少了 {wanted}");
    }
    for key in [PAGE_UP, PAGE_DOWN, b"/".as_slice(), BACKSPACE] {
        assert!(typed.contains(&key), "{key:?}");
    }
}

#[test]
fn the_new_session_says_one_thing() {
    let steps = steps();
    let said: Vec<usize> = (0..steps.len())
        .filter(|&at| steps[at].act == Act::Say)
        .collect();
    assert_eq!(said.len(), 1);
    let before = &steps[said[0] - 2].act;
    assert_eq!(
        before,
        &Act::Keys {
            keys: b"/new",
            until: "/new",
            or: None
        }
    );
}

/// 关页面的几步、撤回那一步另有一种也算的字，别的步骤只认一种。
#[test]
fn only_the_unsure_redraws_accept_a_second_text() {
    for step in steps() {
        if let Act::Keys { until, or, .. } = step.act {
            match (until, or) {
                (BACK, Some(BACK_TOO)) | ("/restore 恢复", Some("tools: go.")) | (_, None) => {}
                other => panic!("{}：{other:?}", step.name),
            }
            if until == BACK {
                assert_eq!(or, Some(BACK_TOO), "{}", step.name);
            }
        }
    }
}

/// 设置页（施工 LW-19）：每个分类都进一次、退回分类页，最后才关掉设置页；进去以后等的字和分类页上的说明不一样。
#[test]
fn the_settings_page_enters_every_category() {
    let keyed = keyed();
    let opened = keyed
        .iter()
        .position(|(_, keys, _)| *keys == b"/config")
        .expect("有 /config");
    let closed = keyed[opened..]
        .iter()
        .position(|(_, _, until)| *until == BACK)
        .map(|at| at + opened)
        .expect("关掉设置页");
    let inside = &keyed[opened..closed];
    let mut entered = vec!["通用", "外观"];
    entered.extend(CATEGORIES.iter().map(|(category, _, _)| *category));
    for category in entered {
        let at = inside
            .iter()
            .position(|(name, keys, _)| *keys == ENTER && name == &format!("/config 进{category}"))
            .unwrap_or_else(|| panic!("没进{category}"));
        let back = inside[at..]
            .iter()
            .position(|(_, keys, until)| *keys == ESC && *until == MENU);
        assert!(back.is_some(), "{category} 没退回分类");
    }
    for (category, described, waited) in CATEGORIES {
        assert_ne!(
            described.replace(' ', ""),
            waited.replace(' ', ""),
            "{category}"
        );
    }
}

/// `/theme`（施工 LW-19）：打开以后先往下选一项，再关掉。
#[test]
fn the_theme_picker_moves_once_before_closing() {
    let keyed = keyed();
    let at = keyed
        .iter()
        .position(|(_, keys, _)| *keys == b"/theme")
        .expect("有 /theme");
    assert_eq!(keyed[at + 1].1, ENTER);
    assert_eq!((keyed[at + 2].1, keyed[at + 2].2), (DOWN, THEME_NEXT));
    assert_eq!((keyed[at + 3].1, keyed[at + 3].2), (ESC, BACK));
}
