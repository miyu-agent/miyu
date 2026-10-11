//! 判官看的记录照哪一份字渲染（施工 O-24）：群会话照快照里钉下的，不管核心这时在哪个时区；没有的照这时的。转发那几行的字
//! （施工 O-34）跟着快照走。

use super::*;

fn chat(offset: i32) -> GroupChat {
    GroupChat {
        offset,
        no_text: "[no text content]\n".to_string(),
        recent: None,
        voice: None,
    }
}

#[test]
fn a_group_reads_with_its_pinned_time_zone() {
    assert_eq!(
        pick(Some((chat(540), None)), || Ok((chat(-300), None))),
        Ok((chat(540), None))
    );
    assert_eq!(
        pick(None, || Ok((chat(-300), None))),
        Ok((chat(-300), None))
    );
}

/// 施工 O-34：转发那几行的字跟着快照走：O-34 以前造的群会话没有就不写，不拿这时的补上。
#[test]
fn a_group_made_before_forwards_reads_without_them() {
    let now = ForwardTexts {
        label: "forward:\n".to_string(),
        numbered: "forward #{number}:\n".to_string(),
        more: "({count} more not shown)\n".to_string(),
    };
    assert_eq!(
        pick(Some((chat(540), None)), || Ok((
            chat(540),
            Some(now.clone())
        ))),
        Ok((chat(540), None))
    );
    assert_eq!(
        pick(None, || Ok((chat(540), Some(now.clone())))),
        Ok((chat(540), Some(now)))
    );
}
