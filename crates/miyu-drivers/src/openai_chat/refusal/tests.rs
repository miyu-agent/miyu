//! 认中转转来的 Google 拒绝说明（施工 AG-1）：开头那一句、政策链接两样都在才算。

use miyu_kernel::event::ErrorClass;

use super::{is_refusal, refused};

const SAID: &str = "The prompt could not be submitted. The prompt contains sensitive words that violate Google's [Generative AI Prohibited Use policy](https://policies.google.com/terms/generative-ai/use-policy). Try rephrasing the prompt.";

#[test]
fn both_the_opening_and_the_policy_link_are_needed() {
    assert!(is_refusal(SAID));
    assert!(is_refusal(&format!("\n  {SAID}")), "前面的空白不算");
    assert!(
        !is_refusal("The prompt could not be submitted. Please try again later."),
        "没有政策链接的不认"
    );
    assert!(
        !is_refusal("See https://policies.google.com/terms/generative-ai/use-policy for details."),
        "只有链接、开头不是那一句的不认"
    );
    assert!(
        !is_refusal(&format!("网关回的原话是：{SAID}")),
        "那一句不在开头的不认"
    );
    assert!(
        !is_refusal(
            "the prompt could not be submitted. policies.google.com/terms/generative-ai/use-policy"
        ),
        "大小写照原样"
    );
}

#[test]
fn it_becomes_a_content_policy_error_with_the_words() {
    let error = refused(&format!("  {SAID}\n"));
    assert_eq!(error.class, ErrorClass::ContentPolicy);
    assert_eq!(error.message, SAID);
    assert_eq!(error.status, None);
    let long = format!("{SAID}{}", "x".repeat(1000));
    assert_eq!(refused(&long).message.chars().count(), 500);
}
