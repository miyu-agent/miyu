//! 换出来的要是判过的（施工 5-18）：没带判过的照原样，带了的不在里面就拒。

use std::path::PathBuf;
use std::sync::Arc;

use super::*;
use crate::testkit;

/// 一个用完就删的目录，里面有 `work/a.txt`。
struct Place(PathBuf);

impl Place {
    fn new(name: &str) -> Place {
        let dir = std::env::temp_dir().join(format!("miyu-checked-{}-{name}", std::process::id()));
        std::fs::create_dir_all(dir.join("work")).expect("建得了");
        std::fs::write(dir.join("work/a.txt"), "a").expect("写得进");
        Place(std::fs::canonicalize(dir).expect("在"))
    }

    /// 在 `work/` 里的一次调用，判过的是 `checked`。
    fn call(&self, checked: Option<Vec<PathBuf>>) -> Call {
        Call {
            cwd: self.0.join("work").to_string_lossy().into_owned(),
            checked: checked.map(Arc::from),
            ..testkit::call("{}")
        }
    }
}

impl Drop for Place {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap_or(());
    }
}

#[test]
fn without_checked_positions_the_resolved_one_is_used() {
    let place = Place::new("none");
    let real = place.call(None).real("a.txt").expect("换得成");
    assert_eq!(real, place.0.join("work/a.txt"));
}

#[test]
fn a_checked_position_is_used_and_any_other_is_refused() {
    let place = Place::new("some");
    let a = place.0.join("work/a.txt");
    let call = place.call(Some(vec![a.clone()]));
    assert_eq!(call.real("a.txt").expect("判过的"), a);
    // 还不存在的照上级换，判过的也是这样换的。
    let new = place.0.join("work/new.txt");
    let call = place.call(Some(vec![new.clone()]));
    assert_eq!(call.real("new.txt").expect("判过的"), new);
    assert!(matches!(call.real("a.txt"), Err(Unchecked::Moved)));
    // 一条都没判过的，一律拒。
    assert!(matches!(
        place.call(Some(Vec::new())).real("a.txt"),
        Err(Unchecked::Moved)
    ));
}

#[test]
fn resolve_errors_come_through_unchanged() {
    let place = Place::new("error");
    let error = place
        .call(Some(Vec::new()))
        .real("~/x")
        .expect_err("没有家目录");
    assert!(matches!(error, Unchecked::Resolve(ResolveError::NoHome)));
}

#[test]
fn the_thing_itself_is_checked_and_nameless_ones_are_left_alone() {
    let place = Place::new("itself");
    let a = place.0.join("work/a.txt");
    let call = place.call(Some(vec![a.clone()]));
    assert_eq!(call.real_itself("a.txt").expect("判过的"), Some(a));
    assert!(matches!(call.real_itself("b.txt"), Err(Unchecked::Moved)));
    // `.` 没有名字可碰：工具不碰，用不着判过。
    assert!(matches!(
        place.call(Some(Vec::new())).real_itself("."),
        Ok(None)
    ));
}

/// 判完以后上级被换成了链接：换出来的落在别处，拒。
#[cfg(unix)]
#[test]
fn a_link_swapped_in_after_the_check_is_refused() {
    let place = Place::new("swapped");
    std::fs::create_dir_all(place.0.join("safe")).expect("建得了");
    std::fs::create_dir_all(place.0.join("data")).expect("建得了");
    std::fs::write(place.0.join("data/token"), "t").expect("写得进");
    std::os::unix::fs::symlink(place.0.join("safe"), place.0.join("work/link")).expect("链得上");
    let call = place.call(Some(vec![place.0.join("safe/token")]));
    std::fs::remove_file(place.0.join("work/link")).expect("删得掉");
    std::os::unix::fs::symlink(place.0.join("data"), place.0.join("work/link")).expect("链得上");
    assert!(matches!(call.real("link/token"), Err(Unchecked::Moved)));
    assert!(matches!(
        call.real_itself("link/token"),
        Err(Unchecked::Moved)
    ));
}
