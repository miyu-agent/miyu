//! 家目录那几处的环境变量照名字配（施工 Y-11 补修）：`USER_DIRS` 的每一处都有、对得上名字；不认识的名字没有。

use super::*;

#[test]
fn every_home_folder_has_its_own_variable() {
    for name in USER_DIRS {
        assert_eq!(
            var(name),
            Some(format!("MIYU_{}_DIR", name.to_uppercase()).as_str()),
            "{name}"
        );
    }
    assert_eq!(var("music"), None);
}
