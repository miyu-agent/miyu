use serde_json::json;

use super::{absolute, expand};

#[test]
fn home_relative_directories_are_written_out() {
    let home = std::path::Path::new("/home/me");
    assert_eq!(expand("~", Some("/home/me")), "/home/me");
    assert_eq!(
        expand("~/src/miyu", Some("/home/me")),
        home.join("src/miyu").to_string_lossy()
    );
    assert_eq!(expand("~/src", None), "~/src", "读不出家目录的照原样");
    assert_eq!(expand("/srv/x", Some("/home/me")), "/srv/x");
}

#[test]
fn only_absolute_directories_are_taken() {
    let root = if cfg!(windows) { "C:\\work" } else { "/work" };
    let dotted = format!("{root}{}.", std::path::MAIN_SEPARATOR);
    assert_eq!(absolute(&json!({"cwd": dotted})).as_deref(), Some(root));
    assert_eq!(absolute(&json!({"cwd": "work"})), None);
    assert_eq!(absolute(&json!({})), None);
}
