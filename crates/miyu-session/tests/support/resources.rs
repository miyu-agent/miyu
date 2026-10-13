//! 资源目录拷一份（施工 K-6 下）：测试要改其中一份数据（例如自动想起的门槛表），不碰仓库里出厂的那一份。

use std::path::{Path, PathBuf};

use miyu_store::resources::ResourceRoot;

use super::Home;
/// 出厂的资源目录拷一份到场地里，换成这一份（施工 K-6 下：测试要改其中一份数据，例如自动想起的门槛表）。交回拷到了哪。
pub fn copy_resources(home: &mut Home) -> PathBuf {
    fn copy(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).expect("建得了");
        for entry in std::fs::read_dir(from).expect("读得了") {
            let entry = entry.expect("读得了");
            let target = to.join(entry.file_name());
            match entry.file_type().expect("读得了").is_dir() {
                true => copy(&entry.path(), &target),
                false => {
                    std::fs::copy(entry.path(), &target).expect("拷得了");
                }
            }
        }
    }
    let to = home.scratch.0.join("resources");
    copy(home.resources.path(), &to);
    home.resources = ResourceRoot::at(to.clone());
    to
}
