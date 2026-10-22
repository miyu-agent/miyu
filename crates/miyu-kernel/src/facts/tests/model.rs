//! 模型那一块（施工 E-M）：模板造的时候就查；四个字段照换、照转义；边界上排在会话编号后面；同一个身份不再注入，换了模型、
//! 档位、预设的才追加；没给身份、以前的快照没有这份模板的，都没有这一块。

use super::*;

/// 替身的模板加上模型那一份。
fn with_model() -> FactTemplates {
    templates()
        .with_model(Some(
            r#"<m p="{provider}" n="{name}" e="{effort}" s="{preset}"/>"#,
        ))
        .unwrap()
}

fn identity(model: &str, effort: &str, preset: &str) -> Identity {
    Identity {
        provider: "DeepSeek".to_string(),
        model: model.to_string(),
        effort: effort.to_string(),
        preset: preset.to_string(),
    }
}

/// 这个边界上模型那一块注入什么；不注入的是 `None`。
fn told(templates: &FactTemplates, log: &Log, identity: &Identity) -> Option<String> {
    templates
        .boundary(
            &log.history,
            now(),
            &environment("~/src/miyu"),
            &permission(Level::Workspace, false),
            (&session(), Some(identity)),
        )
        .into_iter()
        .find(|fact| fact.kind.as_str() == "model")
        .map(|fact| fact.text)
}

/// 开一个回合，内核在里面记下模型那一块 `text`。
fn saw(log: &mut Log, text: &str) {
    log.start();
    log.inject(KERNEL, "model", text);
    log.end();
}

#[test]
fn the_model_template_is_checked_when_made() {
    let wrong = templates()
        .with_model(Some(r#"<m t="{time}"/>"#))
        .unwrap_err();
    assert!(wrong.why.contains("time"), "{wrong}");
    assert!(templates().with_model(Some("<m n=\"{name\"/>")).is_err());
    assert!(templates().with_model(None).is_ok());
}

#[test]
fn the_block_names_the_provider_the_model_the_effort_and_the_preset() {
    let block = with_model()
        .model(&identity("deepseek-v4", "high", "dev"))
        .unwrap();
    assert_eq!(block.kind.as_str(), "model");
    assert!(!block.after);
    assert_eq!(
        block.text,
        r#"<m p="DeepSeek" n="deepseek-v4" e="high" s="dev"/>"#
    );
}

#[test]
fn a_provider_name_cannot_write_another_tag() {
    let mut someone = identity("m", "default", "none");
    someone.provider = r#"a"/><x y="&"#.to_string();
    let block = with_model().model(&someone).unwrap();
    assert_eq!(
        block.text,
        format!(
            r#"<m p="{}" n="m" e="default" s="none"/>"#,
            escape(&someone.provider)
        )
    );
    assert!(!block.text.contains("<x"), "{}", block.text);
}

#[test]
fn it_comes_after_the_session_block_at_a_boundary() {
    let facts = with_model().boundary(
        &History::default(),
        now(),
        &environment("~/src/miyu"),
        &permission(Level::Workspace, false),
        (&session(), Some(&identity("m", "default", "none"))),
    );
    let kinds: Vec<&str> = facts.iter().map(|fact| fact.kind.as_str()).collect();
    assert_eq!(kinds, ["env", "permission", "session", "model"]);
}

#[test]
fn the_same_identity_is_not_told_again_and_a_change_is() {
    let first = identity("deepseek-v4", "default", "none");
    let mut log = Log::new();
    log.start();
    let text = told(&with_model(), &log, &first).unwrap();
    log.end();
    saw(&mut log, &text);
    log.start();
    assert_eq!(told(&with_model(), &log, &first), None);
    for changed in [
        identity("deepseek-v4-flash", "default", "none"),
        identity("deepseek-v4", "high", "none"),
        identity("deepseek-v4", "default", "dev"),
    ] {
        assert!(told(&with_model(), &log, &changed).is_some(), "{changed:?}");
    }
}

#[test]
fn without_an_identity_or_a_template_there_is_no_model_block() {
    let quiet = with_model().boundary(
        &History::default(),
        now(),
        &environment("~/src/miyu"),
        &permission(Level::Workspace, false),
        (&session(), None),
    );
    assert!(quiet.iter().all(|fact| fact.kind.as_str() != "model"));
    let log = Log::new();
    assert_eq!(
        told(&templates(), &log, &identity("m", "default", "none")),
        None
    );
}
