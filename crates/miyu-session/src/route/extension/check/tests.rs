//! driver-check 的样本和比法（施工 MD-3）：样本各不相同、都带记号字、一对只差末尾一条；头一处不同在哪；稳定区到记号字为止要相同，
//! 找不到记号字的报出来不当过。

use serde_json::Value;

use super::*;

#[test]
fn samples_differ_and_all_carry_the_stable_mark() {
    let samples = samples("pi/ag");
    let names: Vec<&str> = samples.iter().map(|sample| sample.name).collect();
    assert_eq!(
        names,
        ["plain", "tools", "private", "image", "continuation"]
    );
    for (at, sample) in samples.iter().enumerate() {
        let value: Value = serde_json::from_str(&sample.params).expect("是 JSON");
        assert_eq!(value["driver"], "pi/ag");
        assert!(sample.params.contains(STABLE_MARK), "{}", sample.name);
        assert!(
            samples[..at]
                .iter()
                .all(|other| other.params != sample.params),
            "{} 和前面的不一样",
            sample.name
        );
    }
    let private = &samples[2].params;
    assert!(
        private.contains(r#""driver":"pi/ag","data":"sig-1""#),
        "私有数据记成这个驱动的：{private}"
    );
    let [short, long] = pair("pi/ag");
    let (short, long): (Value, Value) = (
        serde_json::from_str(&short.params).expect("JSON"),
        serde_json::from_str(&long.params).expect("JSON"),
    );
    let (short, long) = (
        short["request"]["messages"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
        long["request"]["messages"]
            .as_array()
            .cloned()
            .unwrap_or_default(),
    );
    assert_eq!(long.len(), short.len() + 1);
    assert_eq!(long[..short.len()], short[..], "只在末尾多一条");
}

#[test]
fn the_first_difference_is_found() {
    assert_eq!(first_difference(b"abc", b"abc"), None);
    assert_eq!(first_difference(b"abc", b"abd"), Some(2));
    assert_eq!(
        first_difference(b"ab", b"abc"),
        Some(2),
        "一份是另一份的开头"
    );
}

#[test]
fn the_stable_region_is_compared_up_to_the_mark() {
    let mark = STABLE_MARK;
    let short = format!(r#"{{"a":1,"m":"{mark}","x":[1]}}"#);
    let long = format!(r#"{{"a":1,"m":"{mark}","x":[1,2]}}"#);
    assert_eq!(stable_prefix(short.as_bytes(), long.as_bytes()), Ok(()));
    let stamped = format!(r#"{{"a":2,"m":"{mark}","x":[1,2]}}"#);
    assert_eq!(
        stable_prefix(short.as_bytes(), stamped.as_bytes()),
        Err(
            "adding a message at the end changed byte 5, before the end of the stable region"
                .to_string()
        )
    );
    // 扩展把记号字的一个字母转义了：照「没法查稳定区」报。
    let escaped = format!(r#"{{"a":1,"m":"miyu{}tableend7f3a"}}"#, "\\u0073");
    assert!(
        stable_prefix(short.as_bytes(), escaped.as_bytes())
            .is_err_and(|why| why.contains("the longer body has no")),
        "找不到记号字的报出来"
    );
}
