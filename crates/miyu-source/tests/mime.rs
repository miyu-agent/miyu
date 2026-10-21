//! 包文件的类型（施工 G-14，`37-软件源.md` 第二节第 6 条）：仓库里的 shared-mime-info 定义只给 zstd 压的 tar 加一条
//! `*.miyu`，不另开类型；后缀和 [`miyu_source::archive::EXTENSION`] 对得上（改了后缀忘了改它的会红）。

use std::path::Path;

/// shared-mime-info 的名字空间。
const NAMESPACE: &str = "http://www.freedesktop.org/standards/shared-mime-info";

#[test]
fn package_files_are_known_as_zstd_compressed_tar() {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging/mime/miyu.xml");
    let text = std::fs::read_to_string(&file).expect("仓库里有定义文件");
    let document = roxmltree::Document::parse(&text).expect("是合写法的 XML");
    let root = document.root_element();
    assert!(root.has_tag_name((NAMESPACE, "mime-info")), "{root:?}");
    let types: Vec<_> = root.children().filter(|node| node.is_element()).collect();
    assert_eq!(types.len(), 1, "只挂一个类型，不另开");
    assert!(types[0].has_tag_name((NAMESPACE, "mime-type")));
    assert_eq!(
        types[0].attribute("type"),
        Some("application/x-zstd-compressed-tar")
    );
    let globs: Vec<_> = types[0]
        .children()
        .filter(|node| node.is_element())
        .collect();
    assert_eq!(globs.len(), 1, "只加一条 glob");
    assert!(globs[0].has_tag_name((NAMESPACE, "glob")));
    let pattern = format!("*.{}", miyu_source::archive::EXTENSION);
    assert_eq!(globs[0].attribute("pattern"), Some(pattern.as_str()));
    assert_eq!(globs[0].attribute("weight"), Some("80"));
}
