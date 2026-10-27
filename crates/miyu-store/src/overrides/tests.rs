//! 工具说明覆盖的文件（施工 OV-1 上）：只认 `<工具名>.toml`，照名字排；没有目录的是空的。

use super::*;
use crate::test_support::Scratch;

#[test]
fn only_tool_named_toml_files_are_found_in_order() {
    let temp = Scratch::new();
    let dir = temp.path().join(TOOLS);
    std::fs::create_dir_all(dir.join("sub.toml")).expect("建得了");
    for name in ["shell.toml", "read.toml", "bad name.toml", "notes.txt"] {
        std::fs::write(dir.join(name), "description = \"x\"\n").expect("写得进");
    }
    let found = tools_in(&dir);
    let tools: Vec<&str> = found.iter().map(|one| one.tool.as_str()).collect();
    assert_eq!(tools, ["read", "shell"]);
    assert_eq!(found[0].text.as_deref(), Ok("description = \"x\"\n"));
    assert!(tools_in(&temp.path().join("nothing")).is_empty());
}
