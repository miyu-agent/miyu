//! 整理好的目录先写到旁边再换过去（审计 #031 第 3 条，施工 AU-P1）：同一个进程里两次写各用各的临时文件。

use super::temp_name;

#[test]
fn two_writes_in_one_process_use_different_temporary_files() {
    let (first, second) = (temp_name(), temp_name());
    assert_ne!(first, second);
    assert!(
        first.starts_with(super::FILE) && first.ends_with(".tmp"),
        "{first}"
    );
}
