//! 邀请码（施工 Y-1 上）：能用的认、用一次就没了、过期的不认、撤掉的没了；写的时候删掉过期没用的、用掉超过 30 天的；写读一字
//! 不差。

use super::*;
use crate::test_support::Scratch;

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("合写法")
}

fn invite(id: &str, hash: &str, expires: &str) -> Invite {
    Invite {
        id: id.to_string(),
        hash: hash.to_string(),
        by: "admin".to_string(),
        created: at("2026-10-22T00:00:00.000Z"),
        expires: at(expires),
        used: None,
    }
}

#[test]
fn an_invite_is_usable_once_until_it_expires() {
    let now = at("2026-10-22T01:00:00.000Z");
    let mut invites = Invites::default();
    invites.add(
        invite("0000000a", "sha256:a", "2026-10-29T00:00:00.000Z"),
        now,
    );
    assert_eq!(invites.usable("sha256:a", now), Some("0000000a"));
    assert_eq!(invites.usable("sha256:b", now), None, "不认识的不认");
    let late = at("2026-10-29T00:00:00.000Z");
    assert_eq!(
        invites.usable("sha256:a", late),
        None,
        "到了过期的那一刻就不认"
    );
    assert_eq!(
        invites.take("sha256:a", "alice", now).as_deref(),
        Some("0000000a")
    );
    assert_eq!(invites.usable("sha256:a", now), None, "用掉的不认");
    assert_eq!(invites.take("sha256:a", "bob", now), None, "用不了第二次");
    assert_eq!(
        invites.invites[0].used,
        Some(Used {
            account: "alice".to_string(),
            at: now
        })
    );
    assert!(!invites.revoke("0000000a"), "用掉的撤不了");
}

#[test]
fn revoking_and_pruning_drop_what_is_no_longer_needed() {
    let now = at("2026-10-22T01:00:00.000Z");
    let mut invites = Invites::default();
    invites.add(
        invite("0000000a", "sha256:a", "2026-10-29T00:00:00.000Z"),
        now,
    );
    invites.add(
        invite("0000000b", "sha256:b", "2026-10-22T00:30:00.000Z"),
        now,
    );
    assert!(invites.revoke("0000000a"));
    assert!(!invites.revoke("0000000a"), "撤过的没了");
    let mut old = invite("0000000c", "sha256:c", "2026-10-29T00:00:00.000Z");
    old.used = Some(Used {
        account: "alice".to_string(),
        at: at("2026-09-01T00:00:00.000Z"),
    });
    invites.invites.push(old);
    let mut recent = invite("0000000d", "sha256:d", "2026-10-29T00:00:00.000Z");
    recent.used = Some(Used {
        account: "bob".to_string(),
        at: at("2026-10-20T00:00:00.000Z"),
    });
    invites.invites.push(recent);
    invites.prune(now);
    let ids: Vec<&str> = invites
        .invites
        .iter()
        .map(|each| each.id.as_str())
        .collect();
    assert_eq!(
        ids,
        ["0000000d"],
        "过期没用的、用掉超过 30 天的删掉，最近用掉的留着"
    );
}

#[test]
fn at_most_64_are_kept() {
    let now = at("2026-10-22T01:00:00.000Z");
    let mut invites = Invites::default();
    for n in 0..70 {
        invites.add(
            invite(
                &format!("{n:08x}"),
                &format!("sha256:{n}"),
                "2026-10-29T00:00:00.000Z",
            ),
            now,
        );
    }
    assert_eq!(invites.invites.len(), 64);
    assert_eq!(invites.invites[0].id, format!("{:08x}", 6), "删最早发的");
}

#[test]
fn a_written_file_reads_back() {
    let temp = Scratch::new();
    let path = temp.path().join("system").join(FILE);
    let empty = read(&path).expect("没有的是空的");
    assert!(empty.invites.invites.is_empty());
    let mut invites = empty.invites;
    invites.add(
        invite("0000000a", "sha256:a", "2026-10-29T00:00:00.000Z"),
        at("2026-10-22T01:00:00.000Z"),
    );
    write(&path, &invites, None).expect("写得进");
    assert_eq!(read(&path).expect("读得回").invites, invites);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).expect("在").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
