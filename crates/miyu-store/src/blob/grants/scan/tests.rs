//! 在包的状态目录里找哈希（施工 BG-1补）：文本里的、二进制里的、跨着两块的找得到；写法不对的不认；链接不跟；太大的、
//! 太多的、太深的不扫、数得出；状态目录本身是链接的不扫。

use std::fs;

use super::*;
use crate::test_support::Scratch;

fn hash(text: &str) -> ContentHash {
    ContentHash::of(text.as_bytes())
}

/// 临时目录里的 `var/lib/memes/`，建好。
fn state(scratch: &Scratch) -> std::path::PathBuf {
    let dir = scratch.path().join("var").join("lib").join("memes");
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn found(dir: &Path) -> BTreeSet<ContentHash> {
    let scanned = scan(dir, &Limits::default()).unwrap();
    assert!(scanned.complete(), "{scanned:?}");
    scanned.found
}

#[test]
fn hashes_are_found_in_text_and_in_binary_files() {
    let scratch = Scratch::new();
    let dir = state(&scratch);
    fs::write(
        dir.join("index.json"),
        format!(
            r#"{{"a":"{}","b":["{}"]}}"#,
            hash("one").as_str(),
            hash("two").as_str()
        ),
    )
    .unwrap();
    // 二进制的：前后是不合 UTF-8 的字节，两个哈希紧挨着。
    let mut binary = vec![0u8, 0xff, 0xfe, 0x00];
    binary.extend_from_slice(hash("three").as_str().as_bytes());
    binary.extend_from_slice(hash("four").as_str().as_bytes());
    binary.extend_from_slice(&[0x80, 0x00]);
    fs::create_dir_all(dir.join("db").join("deep")).unwrap();
    fs::write(dir.join("db").join("deep").join("memes.sqlite"), binary).unwrap();
    assert_eq!(
        found(&dir),
        BTreeSet::from([hash("one"), hash("two"), hash("three"), hash("four")])
    );
}

#[test]
fn a_hash_across_two_chunks_is_found() {
    let scratch = Scratch::new();
    let dir = state(&scratch);
    // 哈希跨在每一种切法上：头一次读满 `CHUNK + LENGTH` 个字节，前面垫的长短从「整个在第一块里」挪到「整个在第二块里」。
    for (n, pad) in (CHUNK - 2..=CHUNK + LENGTH + 2).enumerate() {
        let mut bytes = vec![b'x'; pad];
        bytes.extend_from_slice(hash(&format!("split {n}")).as_str().as_bytes());
        bytes.extend_from_slice(b"tail");
        fs::write(dir.join(format!("{n}")), bytes).unwrap();
    }
    let wanted: BTreeSet<ContentHash> = (0..=LENGTH + 4)
        .map(|n| hash(&format!("split {n}")))
        .collect();
    assert_eq!(found(&dir), wanted);
}

#[test]
fn what_is_not_written_like_a_hash_is_not_taken() {
    let scratch = Scratch::new();
    let dir = state(&scratch);
    let good = hash("good");
    let hex = good.hex();
    fs::write(
        dir.join("notes"),
        format!(
            "sha256:{} sha256:{} sha512:{hex} sha256:{}g sha256: {hex} SHA256:{hex} sha256:sha256:{hex}",
            hex.to_uppercase(),
            &hex[..63],
            &hex[..62],
        ),
    )
    .unwrap();
    assert_eq!(found(&dir), BTreeSet::from([good]), "只有最后那一个合写法");
}

#[test]
fn a_missing_directory_is_empty() {
    let scratch = Scratch::new();
    let scanned = scan(
        &scratch.path().join("var").join("lib").join("memes"),
        &Limits::default(),
    )
    .unwrap();
    assert!(scanned.found.is_empty() && scanned.complete() && scanned.files == 0);
    state(&scratch);
    let scanned = scan(
        &scratch.path().join("var").join("lib").join("other"),
        &Limits::default(),
    )
    .unwrap();
    assert!(scanned.found.is_empty() && scanned.complete());
}

#[cfg(unix)]
#[test]
fn links_are_not_followed() {
    use std::os::unix::fs::symlink;

    let scratch = Scratch::new();
    let dir = state(&scratch);
    let outside = scratch.path().join("elsewhere");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("secret"), hash("outside").as_str()).unwrap();
    fs::write(dir.join("own"), hash("own").as_str()).unwrap();
    symlink(outside.join("secret"), dir.join("file-link")).unwrap();
    symlink(&outside, dir.join("dir-link")).unwrap();
    assert_eq!(found(&dir), BTreeSet::from([hash("own")]), "链接外面的不认");

    // 状态目录本身是链接的：整个不扫。
    let linked = scratch.path().join("var").join("lib").join("linked");
    symlink(&outside, &linked).unwrap();
    assert!(scan(&linked, &Limits::default()).is_err());
}

#[test]
fn a_file_over_the_limit_is_skipped_and_counted() {
    let scratch = Scratch::new();
    let dir = state(&scratch);
    let small = hash("small").as_str().to_string();
    fs::write(dir.join("small"), &small).unwrap();
    fs::write(
        dir.join("large"),
        format!("{}{}", hash("large").as_str(), "x".repeat(100)),
    )
    .unwrap();
    let limits = Limits {
        file_bytes: small.len() as u64,
        ..Limits::default()
    };
    let scanned = scan(&dir, &limits).unwrap();
    assert_eq!(scanned.found, BTreeSet::from([hash("small")]));
    assert_eq!((scanned.files, scanned.too_large), (2, 1));
    assert!(!scanned.complete());
}

#[test]
fn too_many_files_and_too_deep_directories_stop_the_scan() {
    let scratch = Scratch::new();
    let dir = state(&scratch);
    for n in 0..5 {
        fs::write(dir.join(format!("{n}")), hash(&format!("{n}")).as_str()).unwrap();
    }
    let limits = Limits {
        files: 3,
        ..Limits::default()
    };
    let scanned = scan(&dir, &limits).unwrap();
    assert_eq!((scanned.files, scanned.found.len()), (3, 3));
    assert!(scanned.cut_short && !scanned.complete());

    let deep = dir.join("1-level").join("2-level");
    fs::create_dir_all(&deep).unwrap();
    fs::write(dir.join("1-level").join("in"), hash("in").as_str()).unwrap();
    fs::write(deep.join("out"), hash("out").as_str()).unwrap();
    let limits = Limits {
        depth: 1,
        ..Limits::default()
    };
    let scanned = scan(&dir, &limits).unwrap();
    assert!(scanned.found.contains(&hash("in")) && !scanned.found.contains(&hash("out")));
    assert_eq!(scanned.too_deep, 1);
    assert!(!scanned.complete());
}
