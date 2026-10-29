//! 内置语义模型这个小程序包（施工 R-5 三补，设计 `30-插件框架.md` 第六节）：出厂的人格记忆推荐它、不依赖它，出厂不装。包本身
//! 在 `miyu-agent/miyu-embed` 仓库（施工 S-7 下），清单读得成小程序包的测试也在那边。

use miyu_config::package::read;

use crate::support::resources;

#[test]
fn memory_recommends_the_embed_package_that_is_not_shipped() {
    let memory = resources()
        .path()
        .join("packages")
        .join(miyu_memory::PACKAGE)
        .join("package.toml");
    let memory = read(&std::fs::read_to_string(&memory).expect("出厂有")).expect("写法对");
    assert_eq!(memory.recommends, ["miyu-embed"]);
    assert!(memory.depends.is_empty(), "推荐，不是缺了不起");
    assert!(
        !resources()
            .path()
            .join("packages")
            .join("miyu-embed")
            .exists(),
        "出厂不装"
    );
}
