//! 设置项命名的规矩（施工 UX-1，`docs/blueprint/config.md`「给人看的字」的「命名的规矩」，`miyu_config::naming`）：核心登记的项、包的
//! 「启用」、出厂的包的项，中英日三份字表里的名字都是软件术语、开关都正着说。

use std::path::Path;

use miyu_config::ConfigWords;
use miyu_core::settings::{Packaged, items};
use miyu_store::packages::Packages;
use miyu_store::resources::ResourceRoot;

#[test]
fn every_name_is_a_term_and_every_switch_says_on() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut found = Packages::shipped(&ResourceRoot::at(root.join("resources"))).read();
    let packaged = Packaged::as_released(&mut found);
    let listed: Vec<_> = items()
        .into_iter()
        .chain(packaged.items.iter().cloned())
        .chain(miyu_config::package::enabled::ITEMS.iter().cloned())
        .collect();
    for language in ["zh", "en", "ja"] {
        let file = root.join(format!("resources/core/human/{language}.json"));
        let text = std::fs::read_to_string(&file).expect("读得到");
        let json: serde_json::Value = serde_json::from_str(&text).expect("是 JSON");
        let words: ConfigWords =
            serde_json::from_value(json["config"].clone()).expect("配置那一格写法对");
        assert_eq!(
            miyu_config::naming::check(&listed, &words, language),
            Vec::<String>::new(),
            "{language}"
        );
    }
}
