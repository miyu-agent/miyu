//! RFC 4648 第十节的七个测试值，加上用到 `+`、`/` 的两个字节；换成库以后和原来自己拼的逐字一样（施工 V-38）。

use super::*;

/// 原来自己拼的写法（施工 V-38 以前）：拿来对照。
fn by_hand(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let byte = |i: usize| u32::from(chunk.get(i).copied().unwrap_or(0));
        let group = byte(0) << 16 | byte(1) << 8 | byte(2);
        for i in 0..4 {
            if i <= chunk.len() {
                let index = (group >> (18 - 6 * i)) & 0x3f;
                out.push(char::from(ALPHABET[index as usize]));
            } else {
                out.push('=');
            }
        }
    }
    out
}

#[test]
fn the_rfc_test_vectors() {
    for (input, output) in [
        ("", ""),
        ("f", "Zg=="),
        ("fo", "Zm8="),
        ("foo", "Zm9v"),
        ("foob", "Zm9vYg=="),
        ("fooba", "Zm9vYmE="),
        ("foobar", "Zm9vYmFy"),
    ] {
        assert_eq!(encode(input.as_bytes()), output, "{input:?}");
        assert_eq!(encoded_len(input.len() as u64), output.len() as u64);
    }
}

#[test]
fn the_last_two_letters_of_the_alphabet() {
    assert_eq!(encode(&[0xfb, 0xff]), "+/8=");
    assert_eq!(encode(&[0x00, 0x10, 0x83]), "ABCD");
}

#[test]
fn the_library_writes_what_the_hand_written_one_did() {
    let bytes: Vec<u8> = (0..200_003u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    for len in [
        0,
        1,
        2,
        3,
        4,
        5,
        63,
        64,
        65,
        CHUNK - 1,
        CHUNK,
        CHUNK + 1,
        bytes.len(),
    ] {
        assert_eq!(encode(&bytes[..len]), by_hand(&bytes[..len]), "{len}");
    }
}

#[test]
fn chunks_of_whole_groups_join_into_the_whole() {
    let bytes: Vec<u8> = (0..(CHUNK * 3 + 7) as u32)
        .map(|i| (i.wrapping_mul(40_503) >> 7) as u8)
        .collect();
    assert_eq!(CHUNK % 3, 0);
    let joined: String = bytes.chunks(CHUNK).map(encode).collect();
    assert_eq!(joined, encode(&bytes));
    assert_eq!(encoded_len(CHUNK as u64), 64 * 1024);
}
