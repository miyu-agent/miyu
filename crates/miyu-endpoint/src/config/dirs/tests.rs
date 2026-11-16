//! 只查别的技能目录：相对路径写法不对，`~`、`~/…`、绝对路径放过；别的项不管。

use std::borrow::Cow;

use super::*;

fn list(dirs: &[&str]) -> Value {
    Value::List(
        dirs.iter()
            .map(|dir| Value::Text(Cow::Owned((*dir).to_string())))
            .collect(),
    )
}

#[test]
fn a_relative_skills_dir_is_refused_by_name() {
    let elsewhere = if cfg!(windows) { r"D:\srv\x" } else { "/srv/x" };
    let dirs = external::DIRS;
    assert_eq!(problem(dirs, &list(&["~", "~/notes", elsewhere])), None);
    assert_eq!(
        problem(dirs, &list(&["~/notes", "notes"])),
        Some((Code::DirPath, "notes".to_string()))
    );
    assert_eq!(
        problem("ui.language", &list(&["notes"])),
        None,
        "别的项不管"
    );
}
