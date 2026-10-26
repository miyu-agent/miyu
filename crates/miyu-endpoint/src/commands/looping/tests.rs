//! `/loop` 后面的字怎么读（施工 J-2）。

use super::*;

#[test]
fn off_alone_stops_the_loops() {
    assert_eq!(parse("off"), Ok(Asked::Off));
    assert_eq!(
        parse("off the record"),
        Ok(Asked::Dynamic {
            prompt: "off the record"
        }),
        "带字的整句是要说的话"
    );
}

#[test]
fn an_interval_first_or_after_every_makes_it_fixed() {
    assert_eq!(
        parse("5m 看看 CI"),
        Ok(Asked::Fixed {
            every: 300,
            prompt: "看看 CI"
        })
    );
    assert_eq!(
        parse("every 2h  整理收件箱 "),
        Ok(Asked::Fixed {
            every: 7_200,
            prompt: "整理收件箱"
        })
    );
    assert_eq!(
        parse("check the deploy every 20m"),
        Ok(Asked::Fixed {
            every: 1_200,
            prompt: "check the deploy"
        })
    );
    assert_eq!(
        parse("1d 早报"),
        Ok(Asked::Fixed {
            every: 86_400,
            prompt: "早报"
        })
    );
}

#[test]
fn intervals_under_a_minute_become_a_minute() {
    assert_eq!(
        parse("30s ping"),
        Ok(Asked::Fixed {
            every: 60,
            prompt: "ping"
        })
    );
    assert_eq!(
        parse("0m ping"),
        Ok(Asked::Fixed {
            every: 60,
            prompt: "ping"
        })
    );
}

#[test]
fn without_an_interval_it_is_dynamic() {
    assert_eq!(
        parse("盯着部署，好了告诉我"),
        Ok(Asked::Dynamic {
            prompt: "盯着部署，好了告诉我"
        })
    );
    assert_eq!(
        parse("every day is a gift"),
        Ok(Asked::Dynamic {
            prompt: "every day is a gift"
        }),
        "every 后面不是间隔的不算"
    );
    assert_eq!(
        parse("5 minutes then check"),
        Ok(Asked::Dynamic {
            prompt: "5 minutes then check"
        })
    );
}

#[test]
fn nothing_or_only_an_interval_is_bad() {
    assert_eq!(parse(""), Err(Refusal::BAD_PARAMS));
    assert_eq!(parse("5m"), Err(Refusal::BAD_PARAMS));
    assert_eq!(parse("every 5m"), Err(Refusal::BAD_PARAMS));
}

#[test]
fn odd_numbers_are_not_intervals() {
    assert_eq!(seconds("m"), None);
    assert_eq!(seconds("5x"), None);
    assert_eq!(seconds("-5m"), None);
    assert_eq!(seconds("99999999999999999999d"), None, "溢出的不算");
    assert_eq!(seconds("18446744073709551615d"), None, "乘溢出的不算");
}
