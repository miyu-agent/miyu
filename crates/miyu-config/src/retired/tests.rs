//! 退役的键（施工 FL-1）：对得上那几样的认，别的不认。

use super::retired;

fn segments(key: &str) -> Vec<String> {
    key.split('.').map(str::to_string).collect()
}

#[test]
fn the_retired_keys_are_known_and_others_are_not() {
    assert!(retired(&segments("providers.relay.compat.strict_tools")));
    assert!(retired(&segments("loading.tools.weather")));
    assert!(retired(&segments("loading.mcp.github")));
    assert!(!retired(&segments("providers.relay.compat.stream_usage")));
    assert!(!retired(&segments("loading.tools")));
    assert!(!retired(&segments("ui.language")));
}
