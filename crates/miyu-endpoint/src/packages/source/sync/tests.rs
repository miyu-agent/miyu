//! 拿索引的地址先后（施工 G-3 上）：镜像在前、软件源在后，补上结尾的 `/`，环境变量取不到、空的跳过。

use super::*;

#[test]
fn mirrors_come_before_the_source() {
    let settings = SourceSettings {
        source: Address::Literal("https://miyu-agent.github.io/packages".to_string()),
        mirrors: vec![
            Address::Literal("https://mirror.example.invalid/miyu/".to_string()),
            Address::Env("MIYU_TEST_NO_SUCH_MIRROR_VARIABLE".to_string()),
            Address::Literal("  ".to_string()),
        ],
        hold: None,
    };
    assert_eq!(
        bases(&settings),
        [
            "https://mirror.example.invalid/miyu/",
            "https://miyu-agent.github.io/packages/",
        ]
    );
}
