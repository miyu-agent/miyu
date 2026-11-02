//! 配置、密钥给人看的写法和新建配置第一行的 `#:schema` 都照数据根的布局（设计 55）：写死的几个字和 [`DataRoot`] 算出来的
//! 位置对得上。

use std::path::{Component, Path, PathBuf};

use miyu_store::root::DataRoot;

use super::super::{SECRETS, SYSTEM, SYSTEM_SCHEMA, secrets_path, system_at};

/// 生成的 Schema 的文件名（`miyu-core` 的 `settings::FILES`；端点不依赖核心，这里照抄）。
const SCHEMA: &str = "config.schema.json";

/// 去掉 `..`、`.`：只是字面上的，不碰磁盘。
fn plain(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

#[test]
fn the_shown_names_are_where_the_files_are() {
    let root = DataRoot::at(std::env::temp_dir().join("miyu-config-places"));
    let (system, shown) = system_at(&root);
    assert_eq!(shown, SYSTEM);
    assert_eq!(system, root.path().join(SYSTEM));
    assert_eq!(secrets_path(&root), root.path().join(SECRETS));
}

#[test]
fn the_schema_line_points_at_the_generated_reference() {
    let root = DataRoot::at(std::env::temp_dir().join("miyu-config-places"));
    let beside = root.config_file();
    let schema = plain(&beside.parent().expect("在目录里").join(SYSTEM_SCHEMA));
    assert_eq!(schema, root.config_reference().join(SCHEMA));
}
