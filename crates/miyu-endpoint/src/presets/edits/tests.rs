//! 文件改动的版本（施工 OV-1 下）：账号的号是他那一层和系统区的最后一下；改了的文件照先后、不重，只给他和系统区的；多的说还有几个。

use super::*;

fn account(name: &str) -> AccountId {
    AccountId::parse(name).expect("合写法")
}

#[test]
fn versions_and_changed_files_follow_the_layers() {
    let edits = Edits::default();
    let (alice, bob) = (account("alice"), account("bob"));
    assert_eq!(edits.version(&alice), 0);
    edits.record(
        Some(alice.clone()),
        "home/alice/prompts/core/a.txt".to_string(),
    );
    edits.record(Some(bob.clone()), "home/bob/tools/read.toml".to_string());
    edits.record(None, "system/presets/full.toml".to_string());
    edits.record(
        Some(alice.clone()),
        "home/alice/prompts/core/a.txt".to_string(),
    );
    assert_eq!(edits.version(&alice), 4);
    assert_eq!(edits.version(&bob), 3, "系统区的那一下也算");
    assert_eq!(
        edits.since(&alice, 0, 20),
        (
            vec![
                "home/alice/prompts/core/a.txt".to_string(),
                "system/presets/full.toml".to_string()
            ],
            0
        ),
        "不重、不带别人的"
    );
    assert_eq!(
        edits.since(&alice, 3, 20).0,
        ["home/alice/prompts/core/a.txt"]
    );
    assert_eq!(
        edits.since(&bob, 0, 1),
        (vec!["home/bob/tools/read.toml".to_string()], 1),
        "多的说还有几个"
    );
}

#[test]
fn old_changes_out_of_the_table_still_count() {
    let edits = Edits::default();
    let alice = account("alice");
    for n in 0..(KEPT + 5) {
        edits.record(Some(alice.clone()), format!("home/alice/tools/t{n}.toml"));
    }
    let (files, more) = edits.since(&alice, 0, 100);
    assert_eq!(files.len(), KEPT);
    assert_eq!(more, 1, "挪出表的至少一个");
}
