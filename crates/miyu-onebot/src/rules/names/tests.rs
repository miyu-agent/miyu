//! 给人看的名字（施工 S-6 上）：有几种语言的文件带几种，跟在名字后面的参数照写了的那一份；空白的名字、没写的工具不带；读不懂的那一份记一条问题、别的照读；
//! 没有 `human/` 的什么都不带、不算错。

use super::*;

/// 一个用完就删的临时数据目录。
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir =
            std::env::temp_dir().join(format!("miyu-onebot-names-{}-{name}", std::process::id()));
        std::fs::create_dir_all(dir.join(HUMAN)).expect("建得了");
        Scratch(dir)
    }

    fn file(&self, language: &str, text: &str) {
        std::fs::write(self.0.join(HUMAN).join(format!("{language}.json")), text).expect("写得进");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::fs::remove_dir_all(&self.0).is_err() {
            // 删不掉就留在临时目录里，不影响测试。
        }
    }
}

#[test]
fn every_language_file_names_the_tools_it_writes() {
    let data = Scratch::new("languages");
    data.file(
        "en",
        r#"{"tools":{"a":{"name":"A","subject":"who"},"b":{"name":"  "},"c":{"name":"C"}}}"#,
    );
    data.file("zh", r#"{"tools":{"a":{"name":"甲"}}}"#);
    data.file("ja", "{");
    let mut problems = Vec::new();
    let names = load(&data.0, &["a", "b"], &mut problems);
    assert_eq!(
        names,
        Names::from([(
            "a".to_string(),
            Face {
                names: BTreeMap::from([
                    ("en".to_string(), "A".to_string()),
                    ("zh".to_string(), "甲".to_string()),
                ]),
                subject: Some("who".to_string()),
            }
        )]),
        "空白的 b、没要的 c 不带，读不懂的 ja 不带"
    );
    assert_eq!(
        problems
            .iter()
            .map(|problem| (problem.code, problem.file.as_str()))
            .collect::<Vec<_>>(),
        [(Code::BadFormat, "human/ja.json")]
    );
    let mut none = Vec::new();
    assert!(load(&data.0.join("nowhere"), &["a"], &mut none).is_empty());
    assert!(none.is_empty(), "没有的不算错：{none:?}");
}
