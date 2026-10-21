//! 提示词的命令名（施工 X-39）：恰好一个 `:`，提示词名照工具名清理（空白、斜杠、`:` 换成 `_`）、截到 64 个字符；清理以后撞名的
//! 不登记、进 `skipped`。

use super::*;

fn prompt(name: &str) -> Prompt {
    Prompt {
        name: name.to_string(),
        title: None,
        description: None,
        arguments: Vec::new(),
    }
}

#[test]
fn names_are_cleaned_into_one_colon() {
    assert_eq!(command_name("github", "pr_review"), "github:pr_review");
    assert_eq!(command_name("github", "a b/c:d"), "github:a_b_c_d");
    let long = command_name("github", &"x".repeat(100));
    assert_eq!(long.chars().count(), 64);
    assert_eq!(long.matches(':').count(), 1);
}

#[test]
fn a_name_taken_after_cleaning_is_skipped() {
    let wait = Duration::from_secs(1);
    let (offered, skipped) = offered(
        "github",
        vec![prompt("pr review"), prompt("pr_review"), prompt("issues")],
        wait,
    );
    let commands: Vec<&str> = offered.iter().map(|one| one.command.as_str()).collect();
    assert_eq!(commands, ["github:pr_review", "github:issues"]);
    assert_eq!(offered[0].prompt.name, "pr review", "取的时候照原名");
    assert_eq!(
        skipped,
        [Skipped::prompt("pr_review".to_string(), "name_taken")]
    );
}
