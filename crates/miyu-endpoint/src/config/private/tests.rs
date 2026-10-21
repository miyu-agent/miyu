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

/// 家目录、它的上级、别处的一个绝对路径，照这个系统的写法（Windows 上 `/srv/x` 不算绝对路径，CI 撞见）。
fn places() -> (&'static Path, &'static str, &'static str) {
    if cfg!(windows) {
        (Path::new(r"C:\Users\alice"), r"C:\Users", r"D:\srv\x")
    } else {
        (Path::new("/home/alice"), "/home", "/srv/x")
    }
}

#[test]
fn home_and_above_are_refused_by_name() {
    let (home, above, elsewhere) = places();
    let add = miyu_tool::private::ADD;
    assert_eq!(
        problem(add, &list(&["~/notes", elsewhere]), Some(home)),
        None
    );
    assert_eq!(
        problem(add, &list(&["~/notes", "~/"]), Some(home)),
        Some((Code::PrivateTooBroad, "~/".to_string()))
    );
    assert_eq!(
        problem(add, &list(&[above]), Some(home)).map(|(code, _)| code),
        Some(Code::PrivateTooBroad)
    );
    assert_eq!(
        problem(add, &list(&["notes"]), Some(home)),
        Some((Code::PrivatePath, "notes".to_string()))
    );
    assert_eq!(
        problem(add, &list(&["~"]), Some(home)).map(|(code, _)| code),
        Some(Code::PrivateTooBroad),
        "只写 ~ 也是家目录"
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

#[test]
fn the_strength_is_drawn_only_on_linux() {
    assert_eq!(hidden_here("sandbox.strength"), !cfg!(target_os = "linux"));
    assert!(!hidden_here("sandbox.private.add"));
}

#[test]
fn only_shipped_dirs_can_be_removed() {
    let remove = miyu_tool::private::REMOVE;
    let shipped = miyu_fs::SHIPPED_PRIVATE[0];
    assert_eq!(problem(remove, &list(&[shipped]), None), None);
    assert_eq!(
        problem(remove, &list(&[shipped, "~/elsewhere"]), None),
        Some((Code::PrivateNotShipped, "~/elsewhere".to_string()))
    );
    assert_eq!(
        options(remove),
        Some(miyu_fs::SHIPPED_PRIVATE),
        "选项是出厂清单"
    );
    assert_eq!(options(miyu_tool::private::ADD), None);
}

/// 去掉的出厂项照反着画勾（施工 5-15 再补）：勾着的是不在值里的；别的项没有这一格。
#[test]
fn removed_dirs_tick_what_is_not_listed() {
    assert_eq!(ticks(miyu_tool::private::REMOVE), Some("unlisted"));
    assert_eq!(ticks(miyu_tool::private::ADD), None);
    assert_eq!(ticks("sandbox.strength"), None);
}
