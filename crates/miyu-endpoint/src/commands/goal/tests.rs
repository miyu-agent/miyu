//! `/goal` 后面的字怎么读（施工 X-8 下）。

use super::*;

fn limit(n: u32) -> Option<NonZeroU32> {
    NonZeroU32::new(n)
}

#[test]
fn subcommands_only_without_more_text() {
    assert_eq!(parse(""), Ok(Asked::Show));
    assert_eq!(parse("pause"), Ok(Asked::Pause));
    assert_eq!(parse("resume"), Ok(Asked::Resume));
    assert_eq!(parse("clear"), Ok(Asked::Clear));
    assert_eq!(
        parse("clear the cache dir"),
        Ok(Asked::Set {
            text: "clear the cache dir",
            limit: None
        }),
        "带字的整句当目标"
    );
    assert_eq!(
        parse("把测试修绿"),
        Ok(Asked::Set {
            text: "把测试修绿",
            limit: None
        })
    );
}

#[test]
fn rounds_come_first_and_zero_means_no_limit() {
    assert_eq!(
        parse("--rounds 20 把测试修绿"),
        Ok(Asked::Set {
            text: "把测试修绿",
            limit: limit(20)
        })
    );
    assert_eq!(
        parse("--rounds 0 把测试修绿"),
        Ok(Asked::Set {
            text: "把测试修绿",
            limit: None
        })
    );
    assert_eq!(
        parse("edit --rounds 5 再写文档"),
        Ok(Asked::Edit {
            text: "再写文档",
            limit: Some(limit(5))
        })
    );
    assert_eq!(
        parse("edit 再写文档"),
        Ok(Asked::Edit {
            text: "再写文档",
            limit: None
        }),
        "没写上限的照原来的"
    );
    assert_eq!(
        parse("--roundsx 20 字"),
        Ok(Asked::Set {
            text: "--roundsx 20 字",
            limit: None
        }),
        "不是这个开关的当原文"
    );
}

#[test]
fn wrong_ones_are_bad_params() {
    for asked in [
        "edit",
        "--rounds 20",
        "--rounds x 字",
        "--rounds -1 字",
        "edit --rounds 3",
    ] {
        assert_eq!(parse(asked), Err(Refusal::BAD_PARAMS), "{asked}");
    }
}
