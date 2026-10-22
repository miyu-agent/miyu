//! 日志门禁：格名、值、插值、panic 原话各认一遍；字符串里的 `//`、括号、测试模块不误认。

use super::*;

fn problems(source: &str) -> Vec<String> {
    scan("crates/demo/src/lib.rs", source)
}

#[test]
fn ids_counts_and_kinds_are_fine() {
    let source = r#"
fn ok() {
    tracing::info!(target: "miyu::demo", session = %id.short(), took_ms = 3, kind = "pdf", "done");
    tracing::warn!(target: "miyu::demo", error = %error, "write failed");
}
"#;
    assert!(problems(source).is_empty(), "{:?}", problems(source));
}

#[test]
fn path_and_content_fields_are_refused_with_their_line() {
    let source = "fn bad() {\n    tracing::warn!(target: \"miyu::demo\", path = %p, \"x\");\n    tracing::info!(%cwd, \"y\");\n    tracing::info!(persona.name = n, \"z\");\n}\n";
    let found = problems(source);
    assert_eq!(found.len(), 3, "{found:?}");
    assert!(
        found[0].starts_with("crates/demo/src/lib.rs:2："),
        "{found:?}"
    );
    assert!(found[1].contains("`cwd`"), "{found:?}");
    assert!(found[2].contains("`persona.name`"), "{found:?}");
}

#[test]
fn display_and_format_in_values_are_refused() {
    let source = r#"fn bad() {
    tracing::warn!(target: "miyu::demo", where_to = %dir.display(), "x");
    tracing::warn!(target: "miyu::demo", got = %format!("{a}/{b}"), "y");
}
"#;
    let found = problems(source);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found[0].contains(".display()") && found[1].contains("format!("),
        "{found:?}"
    );
}

#[test]
fn messages_do_not_interpolate() {
    let source = r#"fn bad() {
    tracing::info!(target: "miyu::demo", "{what} failed");
    tracing::info!(target: "miyu::demo", "failed {}", what);
}
"#;
    assert_eq!(problems(source).len(), 2, "{:?}", problems(source));
}

#[test]
fn a_panicked_task_does_not_carry_its_error() {
    let source = r#"fn bad() {
    tracing::error!(target: "miyu::demo", error = %error, "index task panicked");
    tracing::error!(target: "miyu::demo", panic = error.is_panic(), "index task panicked");
}
"#;
    let found = problems(source);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].contains(":2："), "{found:?}");
}

#[test]
fn strings_comments_and_test_modules_are_not_misread() {
    let source = r#"fn fine() {
    // tracing::warn!(path = %p, "a comment");
    tracing::info!(target: "miyu::demo", kind = "https://x.y/(z)", "fetched");
}

#[cfg(test)]
mod tests {
    fn t() { tracing::info!(path = %p, "in a test"); }
}
"#;
    assert!(problems(source).is_empty(), "{:?}", problems(source));
    assert!(
        scan(
            "crates/demo/src/x/tests.rs",
            "tracing::info!(path = %p, \"t\");"
        )
        .is_empty()
    );
}

#[test]
fn an_allowed_field_says_why() {
    assert!(ALLOWED.iter().all(|(_, _, why)| !why.is_empty()));
    let source = "fn f() { tracing::debug!(target: \"miyu::http\", host = %h, \"sent\"); }\n";
    assert!(scan("crates/miyu-http/src/send.rs", source).is_empty());
    assert_eq!(scan("crates/miyu-net/src/lib.rs", source).len(), 1);
}

#[test]
fn a_span_name_is_not_a_message() {
    let source = "fn f() { tracing::error_span!(target: \"miyu::x\", \"session\", session = id.as_str()); }\n";
    assert!(problems(source).is_empty(), "{:?}", problems(source));
    let leaking = "fn f() { tracing::info_span!(target: \"miyu::x\", \"turn\", path = %p); }\n";
    assert_eq!(problems(leaking).len(), 1, "{:?}", problems(leaking));
}

#[test]
fn a_fixed_message_says_why() {
    assert!(FIXED_MESSAGES.iter().all(|(_, why)| !why.is_empty()));
    let source = "fn f() { tracing::warn!(target: \"miyu::x\", \"{what} failed\"); }\n";
    assert!(scan("crates/miyu-session/src/actor/model.rs", source).is_empty());
    assert_eq!(
        scan("crates/miyu-session/src/actor/turn.rs", source).len(),
        1
    );
}

#[test]
fn a_join_error_s_words_are_not_logged() {
    let source = r#"
fn f() {
    match done {
        Ok(Ok(x)) => x,
        Ok(Err(error)) => {
            tracing::warn!(target: "miyu::x", error = %error, "work failed");
        }
        Err(error) => {
            tracing::warn!(target: "miyu::x", error = %error, "work failed");
        }
    }
    match other {
        Ok(Err(error)) => tracing::warn!(target: "miyu::x", error = %error, "other failed"),
        Err(why) => tracing::warn!(target: "miyu::x", error = ?why, "other failed"),
    }
}
"#;
    let found = problems(source);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found[0].contains(":9：") && found[1].contains(":14："),
        "{found:?}"
    );
    let fixed = source
        .replace("Err(error) => {\n            tracing::warn!(target: \"miyu::x\", error = %error", "Err(error) => {\n            tracing::warn!(target: \"miyu::x\", panic = error.is_panic()")
        .replace("error = ?why", "panic = why.is_panic()");
    assert!(problems(&fixed).is_empty(), "{:?}", problems(&fixed));
    let plain = "fn f() {\n    match read {\n        Ok(x) => x,\n        Err(error) => {\n            tracing::warn!(target: \"miyu::x\", error = %error, \"read failed\");\n        }\n    }\n}\n";
    assert!(problems(plain).is_empty(), "{:?}", problems(plain));
}
