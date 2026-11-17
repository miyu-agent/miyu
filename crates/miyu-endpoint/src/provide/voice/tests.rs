//! 交回的语音（施工 SD-1）：写法、大小、哪一轮该兜底放。

use super::*;

fn sent(json: &str) -> Vec<Sent> {
    serde_json::from_str(json).expect("合写法")
}

fn hash(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

fn image(c: char) -> String {
    format!(
        r#"{{"blob":"{}","media_type":"image/png","width":1,"height":1}}"#,
        hash(c)
    )
}

fn voice(c: char, media_type: &str) -> String {
    format!(
        r#"{{"blob":"{}","media_type":"{media_type}","voice":true,"duration_ms":1200}}"#,
        hash(c)
    )
}

#[test]
fn at_most_one_audio_voice_beside_the_images() {
    let one = format!("[{},{}]", image('a'), voice('b', "audio/mpeg"));
    assert!(shaped(&sent(&one), 4));
    let two = format!("[{},{}]", voice('a', "audio/mpeg"), voice('b', "audio/wav"));
    assert!(!shaped(&sent(&two), 4), "语音至多一段");
    let not_audio = format!("[{}]", voice('a', "image/png"));
    assert!(!shaped(&sent(&not_audio), 4), "要是 audio/ 开头的");
    let five: Vec<String> = ['a', 'b', 'c', 'd', 'e'].into_iter().map(image).collect();
    assert!(
        !shaped(&sent(&format!("[{}]", five.join(","))), 4),
        "图照旧至多 4 张"
    );
    let four_and_a_voice = format!("[{},{}]", five[..4].join(","), voice('f', "audio/ogg"));
    assert!(shaped(&sent(&four_and_a_voice), 4), "语音不占图的名额");
}

#[test]
fn a_voice_larger_than_the_limit_does_not_fit() {
    let (root, _dir) = crate::test_support::temp_root("voice-size");
    let blobs = Blobs::new(root.blobs());
    let small = blobs.put(&[0_u8; 16]).expect("存得进");
    assert!(fits(&blobs, &[small]));
    let big = blobs
        .put(&vec![
            0_u8;
            usize::try_from(MOST_BYTES).expect("放得下") + 1
        ])
        .expect("存得进");
    assert!(!fits(&blobs, &[big]));
    assert_eq!(megabytes(), "10");
}

#[test]
fn only_a_turn_nobody_opened_is_unattended() {
    let by = |json: &str| serde_json::from_str::<By>(json).expect("合写法");
    assert!(unattended(None), "没有触发的那句");
    assert!(unattended(Some(&by(
        r#"{"kind":"schedule","schedule":"t1","loop":"life"}"#
    ))));
    for spoken in [
        r#"{"kind":"person","head":"web"}"#,
        r#"{"kind":"person"}"#,
        r#"{"kind":"harness","name":"codex"}"#,
        r#"{"kind":"session","id":"01a0f233-cfec-7023-8ed5-2a037a1d5ec8"}"#,
    ] {
        assert!(!unattended(Some(&by(spoken))), "{spoken}");
    }
}
