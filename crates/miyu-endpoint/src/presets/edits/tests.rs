//! 文件改动的版本（施工 OV-1 下）：号是最后一下；改了的文件照先后、不重；多的说还有几个。

use super::*;

#[test]
fn versions_and_changed_files_are_counted() {
    let edits = Edits::default();
    assert_eq!(edits.version(), 0);
    edits.record("prompts/core/a.txt".to_string());
    edits.record("presets/full.toml".to_string());
    edits.record("prompts/core/a.txt".to_string());
    assert_eq!(edits.version(), 3);
    assert_eq!(
        edits.since(0, 20),
        (
            vec![
                "prompts/core/a.txt".to_string(),
                "presets/full.toml".to_string()
            ],
            0
        ),
        "不重"
    );
    assert_eq!(edits.since(2, 20).0, ["prompts/core/a.txt"]);
    assert_eq!(
        edits.since(0, 1),
        (vec!["prompts/core/a.txt".to_string()], 1),
        "多的说还有几个"
    );
}

#[test]
fn old_changes_out_of_the_table_still_count() {
    let edits = Edits::default();
    for n in 0..(KEPT + 5) {
        edits.record(format!("tools/t{n}.toml"));
    }
    let (files, more) = edits.since(0, 100);
    assert_eq!(files.len(), KEPT);
    assert_eq!(more, 1, "挪出表的至少一个");
}
