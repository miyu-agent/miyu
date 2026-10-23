//! 预设能关的几块事实（施工 C-11）：都开着的和以前一样；只关时间、只关工作目录的照拆开的写法，类别照旧是 `env`；都关了、没有
//! 拆开写法的不注入环境；权限、会话编号、模型各自关得掉；拆开的写法要了不该要的字段造的时候就报错。

use super::*;

/// 替身的模板加上模型那一份和拆开的两份写法，开关照 `gates`。
fn gated(gates: Gates) -> FactTemplates {
    templates()
        .with_model(Some(r#"<m n="{name}"/>"#))
        .unwrap()
        .with_gates(
            gates,
            Some(r#"<e t="{time}" z="{timezone}"/>"#),
            Some(r#"<e d="{cwd}"/>"#),
        )
        .unwrap()
}

fn identity() -> Identity {
    Identity {
        provider: "DeepSeek".to_string(),
        model: "flash".to_string(),
        effort: "default".to_string(),
        preset: "full".to_string(),
    }
}

/// 一个新会话第一个边界上注入的几块：类别和原文。
fn first(templates: &FactTemplates) -> Vec<(String, String)> {
    templates
        .boundary(
            &Log::new().history,
            now(),
            &environment("~/src/miyu"),
            &permission(Level::Workspace, false),
            (&session(), Some(&identity())),
        )
        .into_iter()
        .map(|fact| (fact.kind.as_str().to_string(), fact.text))
        .collect()
}

fn kinds(facts: &[(String, String)]) -> Vec<&str> {
    facts.iter().map(|(kind, _)| kind.as_str()).collect()
}

fn env_of(facts: &[(String, String)]) -> Option<&str> {
    facts
        .iter()
        .find(|(kind, _)| kind == "env")
        .map(|(_, text)| text.as_str())
}

#[test]
fn all_on_is_what_it_was() {
    let before = with_gates_absent();
    let after = first(&gated(Gates::default()));
    assert_eq!(after, before, "都开着的逐字节一样");
    assert_eq!(kinds(&after), ["env", "permission", "session", "model"]);
}

/// 没调过 `with_gates` 的（以前的快照）。
fn with_gates_absent() -> Vec<(String, String)> {
    first(&templates().with_model(Some(r#"<m n="{name}"/>"#)).unwrap())
}

#[test]
fn time_and_workdir_split_the_env_block() {
    let no_cwd = first(&gated(Gates {
        workdir: false,
        ..Gates::default()
    }));
    assert_eq!(
        env_of(&no_cwd),
        Some(r#"<e t="Fri 2026-09-25 16:00–16:10" z="UTC+09:00"/>"#)
    );
    let no_time = first(&gated(Gates {
        time: false,
        ..Gates::default()
    }));
    assert_eq!(env_of(&no_time), Some(r#"<e d="~/src/miyu"/>"#));
    let neither = first(&gated(Gates {
        time: false,
        workdir: false,
        ..Gates::default()
    }));
    assert_eq!(kinds(&neither), ["permission", "session", "model"]);
    // 没有拆开的写法的：只开一样的时候不注入，不把关掉的那一样写进去。
    let unsplit = templates()
        .with_gates(
            Gates {
                workdir: false,
                ..Gates::default()
            },
            None,
            None,
        )
        .unwrap();
    assert_eq!(env_of(&first(&unsplit)), None);
}

#[test]
fn permission_session_and_model_switch_off_alone() {
    for (gates, gone) in [
        (
            Gates {
                permission: false,
                ..Gates::default()
            },
            "permission",
        ),
        (
            Gates {
                session: false,
                ..Gates::default()
            },
            "session",
        ),
        (
            Gates {
                model: false,
                ..Gates::default()
            },
            "model",
        ),
    ] {
        let facts = first(&gated(gates));
        assert!(!kinds(&facts).contains(&gone), "{gone}: {facts:?}");
        assert_eq!(facts.len(), 3, "别的照旧：{facts:?}");
    }
}

#[test]
fn a_split_template_cannot_ask_for_the_other_half() {
    let leaky = templates().with_gates(Gates::default(), Some(r#"<e d="{cwd}"/>"#), None);
    assert!(leaky.is_err(), "只有时间的那一份要不了工作目录");
    let leaky = templates().with_gates(Gates::default(), None, Some(r#"<e t="{time}"/>"#));
    assert!(leaky.is_err(), "只有工作目录的那一份要不了时间");
}
