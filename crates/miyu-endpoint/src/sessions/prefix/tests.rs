//! 改了的文件里挑真进这个会话前缀的（施工 PX-1）：它用的人格的文件（图片不算）、它的预设、它工具面上工具的覆盖、提示词的覆盖；
//! 一个都挑不出来的照全部列；多的算进「还有」。

use super::*;

fn touched() -> Touches {
    Touches {
        persona: Some("miyu".to_string()),
        preset: Some("coding".to_string()),
        tools: vec!["read".to_string(), "shell".to_string()],
    }
}

#[test]
fn only_what_enters_its_prefix_is_picked() {
    let t = touched();
    for (path, wanted) in [
        ("personas/miyu/persona.toml", true),
        ("personas/miyu/prompts/persona.md", true),
        ("personas/miyu/avatar.webp", false),
        ("personas/miyu/background.JPG", false),
        ("personas/other/prompts/persona.md", false),
        ("personas/other/avatar.png", false),
        ("presets/coding.toml", true),
        ("presets/writing.toml", false),
        ("tools/read.json", true),
        ("tools/write.json", false),
        ("prompts/core/local-paths-rule.txt", true),
        ("commands/review.md", false),
    ] {
        assert_eq!(touches(path, &t), wanted, "{path}");
    }
    let none = Touches::default();
    assert!(
        !touches("personas/miyu/persona.toml", &none),
        "无人格的不认人格的文件"
    );
}

#[test]
fn nothing_picked_lists_everything_and_the_rest_is_counted() {
    let t = touched();
    let all = vec!["personas/other/avatar.png".to_string()];
    assert_eq!(
        relevant((all.clone(), 0), &t),
        (all, 0),
        "挑不出来的照全部列"
    );
    let many: Vec<String> = (0..MOST + 3)
        .map(|n| format!("prompts/core/{n}.txt"))
        .chain(["personas/other/a.png".to_string()])
        .collect();
    let (files, more) = relevant((many, 1), &t);
    assert_eq!(files.len(), MOST);
    assert_eq!(more, 3 + 1, "多的三个、挪出表的一个");
    assert!(files.iter().all(|path| path.starts_with("prompts/")));
}
