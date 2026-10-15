//! 约束解码（施工 L-2）：档案标的照档案，手写的一格盖过档案；都没写的不打开。

use serde_json::json;

use super::*;

/// 档案里 `strict` 那一家标了约束解码，`plain` 没标；两家都照手写的地址走 OpenAI 兼容的写法。
fn strict_held() -> Held {
    Held::new(
        json!({"providers": {"strict": {"compat": {"strict_tools": true}}}}),
        false,
    )
}

const BOTH: &str = "[providers.strict]\ndriver = \"openai-chat\"\nbase_url = \"https://strict.invalid/v1\"\n\n[providers.plain]\ndriver = \"openai-chat\"\nbase_url = \"https://plain.invalid/v1\"\n";

fn strict(source: &str, id: &str) -> bool {
    let values = values(source);
    provider(&values, &strict_held().knowledge(), id)
        .expect("推得出")
        .compat
        .strict_tools
}

#[test]
fn the_profile_marks_a_strict_provider() {
    assert!(strict(BOTH, "strict"));
    assert!(!strict(BOTH, "plain"), "都没写的不打开");
}

#[test]
fn a_hand_written_flag_wins_over_the_profile() {
    let source = format!(
        "{BOTH}\n[providers.strict.compat]\nstrict_tools = false\n\n[providers.plain.compat]\nstrict_tools = true\n"
    );
    assert!(!strict(&source, "strict"), "手写的关掉档案的");
    assert!(strict(&source, "plain"), "手写的打开");
}
