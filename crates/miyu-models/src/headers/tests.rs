//! 另配的头的模板（施工 8-14、8-14 补）：摘要怎么算、同一个种子同一个值，只认 `{session_digest}`。

use super::*;

/// 摘要是 26 位：前 12 位是时间照 opencode 的倒序编法，后 14 位是 SHA-256 的十六进制前 14 位。不是 UUIDv7 的种子照
/// 2026-01-01 编（对照 `python3 -c "import hashlib; print(format(~(1767225600000*4096) & (1<<48)-1, '012x') +
/// hashlib.sha256(b'ses-1').hexdigest()[:14])"`）。
#[test]
fn a_plain_seed_gets_the_fixed_time_and_its_hash() {
    assert_eq!(digest("ses-1"), "4892557fffff09df36791e54c5");
    assert_eq!(digest(PROBE_SEED), "4892557fffff3dae4165d21094");
    assert_eq!(digest("ses-1"), digest("ses-1"), "同一个种子同一个值");
    assert_ne!(digest("ses-1"), digest("ses-2"));
}

/// 会话编号是 UUIDv7 的，前 12 位照它记的毫秒数编：同一个会话重启以后还是这个值，Zen 认它是 opencode 的会话。
#[test]
fn a_uuid_v7_seed_gets_its_own_time() {
    let session = "01a134bc-a180-7274-a8cd-9f7767ba6044";
    assert_eq!(digest(session), "ecb435e7ffff57dc90b026e36b");
    assert_eq!(&digest(session)[..12], time_part(0x01a1_34bc_a180));
}

/// 长得像 UUID、版本不是 7 的，长度不对的、不是十六进制的，都照固定的时间；36 个字却几乎全是 `-` 的也不越界。
#[test]
fn other_shapes_get_the_fixed_time() {
    let fixed = time_part(FIXED_MS);
    for seed in [
        "01a134bc-a180-4274-a8cd-9f7767ba6044",
        "01a134bc-a180-7274-a8cd-9f7767ba604",
        "01a134bca1807274a8cd9f7767ba6044",
        "0za134bc-a180-7274-a8cd-9f7767ba6044",
        "------------------------------------",
        "compact",
    ] {
        assert_eq!(&digest(seed)[..12], fixed, "{seed}");
    }
}

/// 照 opencode 的 `Identifier.descending`：`~(ms × 4096)` 的低 48 位，补够 12 位。
#[test]
fn the_time_part_is_opencode_descending_id() {
    assert_eq!(time_part(1_767_225_600_000), "4892557fffff");
    assert_eq!(time_part(0), "ffffffffffff");
    assert_eq!(time_part(1), "ffffffffefff");
    assert_eq!(
        time_part(0x000F_FFFF_FFFF),
        "000000000fff",
        "高位截掉、不够 12 位的补 0"
    );
}

#[test]
fn filling_replaces_every_digest_and_leaves_plain_values_alone() {
    assert_eq!(
        fill("ses_{session_digest}", "ses-1"),
        "ses_4892557fffff09df36791e54c5"
    );
    assert_eq!(
        fill("{session_digest}/{session_digest}", "ses-1"),
        "4892557fffff09df36791e54c5/4892557fffff09df36791e54c5"
    );
    assert_eq!(fill("cli", "ses-1"), "cli");
}

#[test]
fn only_the_session_digest_can_be_written_in_braces() {
    assert_eq!(check("ses_{session_digest}"), Ok(()));
    assert_eq!(check("cli"), Ok(()));
    for bad in [
        "msg_{call_digest}",
        "{version}",
        "ses_{session_digest",
        "ses_session_digest}",
        "{}",
    ] {
        let error = check(bad).expect_err(bad);
        assert!(error.contains("{session_digest}"), "{error}");
    }
}
