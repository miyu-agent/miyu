//! 只查 `sandbox.private.add`：相对路径写法不对，家目录和上级太宽，别的放过。

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
fn home_and_above_are_refused_by_name() {
    let home = Path::new("/home/alice");
    let add = miyu_tool::private::ADD;
    assert_eq!(
        problem(add, &list(&["~/notes", "/srv/x"]), Some(home)),
        None
    );
    assert_eq!(
        problem(add, &list(&["~/notes", "~/"]), Some(home)),
        Some((Code::PrivateTooBroad, "~/".to_string()))
    );
    assert_eq!(
        problem(add, &list(&["/home"]), Some(home)).map(|(code, _)| code),
        Some(Code::PrivateTooBroad)
    );
    assert_eq!(
        problem(add, &list(&["notes"]), Some(home)),
        Some((Code::BadFormat, "notes".to_string()))
    );
    assert_eq!(
        problem("ui.language", &list(&["~/"]), Some(home)),
        None,
        "别的项不管"
    );
    assert_eq!(
        problem(add, &list(&["~/"]), None),
        None,
        "家目录不知道的只查写法"
    );
}
