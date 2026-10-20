use std::os::unix::fs::symlink;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

/// 一个用完就删的临时目录，换成真实的位置（`plan` 交回的是真实的路径）。
struct Tree(PathBuf);

impl Tree {
    fn new() -> Tree {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("miyu-sandbox-reads-{}-{n}", std::process::id()));
        fs::create_dir_all(dir.join("data/home/admin/workspace")).expect("建得了目录");
        fs::create_dir_all(dir.join("other")).expect("建得了目录");
        fs::write(dir.join("note.txt"), b"note").expect("写得进");
        fs::write(dir.join("data/token"), b"secret").expect("写得进");
        symlink(dir.join("data"), dir.join("alias")).expect("建得了链接");
        Tree(fs::canonicalize(dir).expect("在"))
    }
}

impl Drop for Tree {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn nothing_hidden_reads_the_whole_disk() {
    assert_eq!(plan(&[]).expect("放得了"), vec![PathBuf::from("/")]);
    let gone = PathBuf::from("/no/such/place");
    assert_eq!(plan(&[gone]).expect("放得了"), vec![PathBuf::from("/")]);
}

#[test]
fn the_way_to_the_hidden_is_walked_around() {
    let tree = Tree::new();
    let data = tree.0.join("data");
    let reads = plan(std::slice::from_ref(&data)).expect("放得了");
    // 旁边的整个放行：根目录下的别的、这一级的别的。
    assert!(
        reads.contains(&PathBuf::from("/usr")),
        "根目录旁边的：{reads:?}"
    );
    assert!(reads.contains(&tree.0.join("note.txt")), "{reads:?}");
    assert!(reads.contains(&tree.0.join("other")), "{reads:?}");
    // 藏起来的、通向它的那条路、链接：都不放。
    for skipped in [&data, &tree.0, &PathBuf::from("/"), &tree.0.join("alias")] {
        assert!(!reads.contains(skipped), "{skipped:?}：{reads:?}");
    }
    assert!(
        !reads.iter().any(|path| path.starts_with(&data)),
        "藏起来的下面一样都不放：{reads:?}"
    );
}

#[test]
fn a_hidden_path_given_through_a_link_is_hidden_where_it_really_is() {
    let tree = Tree::new();
    let reads = plan(&[tree.0.join("alias")]).expect("放得了");
    assert!(!reads.contains(&tree.0.join("data")), "{reads:?}");
    assert!(
        !reads
            .iter()
            .any(|path| path.starts_with(tree.0.join("data")))
    );
    assert!(reads.contains(&tree.0.join("other")), "{reads:?}");
}

#[test]
fn a_writable_path_is_walked_around_what_it_hides() {
    let tree = Tree::new();
    let data = tree.0.join("data");
    // 施工 5-15：能写的里面藏着的，从能写的那一条起走。
    let writes = around(&tree.0, std::slice::from_ref(&data)).expect("放得了");
    assert!(
        writes.contains(&tree.0.join("note.txt")),
        "已有的文件单个放：{writes:?}"
    );
    assert!(
        writes.contains(&tree.0.join("other")),
        "旁边的目录整个放：{writes:?}"
    );
    for skipped in [&data, &tree.0, &tree.0.join("alias")] {
        assert!(!writes.contains(skipped), "{skipped:?}：{writes:?}");
    }
    assert!(
        !writes.iter().any(|path| path.starts_with(&data)),
        "{writes:?}"
    );
    // 里面没藏东西的、藏着的不在的：整个放。
    let other = tree.0.join("other");
    assert_eq!(
        around(&other, std::slice::from_ref(&data)).expect("放得了"),
        vec![other.clone()]
    );
    assert_eq!(
        around(&other, &[PathBuf::from("/no/such")]).expect("放得了"),
        vec![other]
    );
    // 能写的里面的再深一层（工作区在数据根里）：能写的照样整个放。
    let workspace = data.join("home/admin/workspace");
    assert_eq!(
        around(&workspace, std::slice::from_ref(&data)).expect("放得了"),
        vec![workspace]
    );
    // 一样深的照藏：什么都不放。
    assert!(
        around(&data, std::slice::from_ref(&data))
            .expect("放得了")
            .is_empty()
    );
}
