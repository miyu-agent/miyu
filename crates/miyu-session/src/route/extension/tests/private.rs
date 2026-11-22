//! 私有数据只认这个扩展驱动的家族：别家的不交过去，交上来的记成这个家族的。

use super::*;

#[tokio::test]
async fn only_this_drivers_private_data_is_handed_over() {
    for (family, kept) in [("pi/ag", true), ("anthropic", false), ("other/ag", false)] {
        let fake = scripted(vec![Plan::Answer(hello(), Duration::ZERO, usage())]);
        let (ended, _, _) = run(
            Arc::clone(&fake),
            Duration::from_secs(5),
            request(Some(family)),
        )
        .await;
        assert_eq!(error(&ended), None);
        let params = fake.params.lock().expect("锁")[0].clone();
        let reasoning = &params["request"]["messages"][0]["blocks"][0];
        assert_eq!(reasoning["text"], "thought", "{family}：块还在");
        assert_eq!(reasoning.get("private").is_some(), kept, "{family}");
    }
}

#[tokio::test]
async fn private_deltas_are_owned_by_the_extension_family() {
    let batches = vec![json!([
        {"start": {"index": 0, "kind": "reasoning"}},
        {"private": {"index": 0, "data": {"sig": "s1"}}},
        {"end": {"index": 0}},
        "truncated"
    ])];
    let fake = scripted(vec![Plan::Answer(batches, Duration::ZERO, usage())]);
    let (_, deltas, _) = run(fake, Duration::from_secs(5), request(None)).await;
    let Some(Delta::Private { private, .. }) = deltas.get(1) else {
        panic!("{deltas:?}")
    };
    assert_eq!(private.driver.as_str(), "pi/ag");
    assert_eq!(private.data.get(), r#"{"sig":"s1"}"#);
    assert_eq!(deltas.last(), Some(&Delta::Truncated));
}
