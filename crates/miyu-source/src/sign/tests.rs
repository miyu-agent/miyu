//! 签名（施工 G-2 上）：新建的钥匙签得了、验得过；改一个字节、换一把钥匙、钥匙不认识、签名文件写坏的都不过；编进来的公钥
//! 都读得成。

use super::*;

#[test]
fn a_signed_index_verifies_with_its_key_only() {
    let made = generate().expect("生成得了");
    let public = PublicKey::from_base64(&made.public).expect("读得成");
    assert_eq!(public.id, made.id);
    assert_eq!(made.id.len(), 16);
    let bytes = br#"{"version":1,"generated":"2026-10-19T03:00:00Z","packages":[]}"#;
    let signed = sign(bytes, &made.secret).expect("签得了");
    assert_eq!(signed.key, made.id);
    let file = serde_json::to_vec(&signed).expect("写得成");
    assert_eq!(
        verify(bytes, &file, std::slice::from_ref(&public)),
        Ok(made.id.clone())
    );

    let mut changed = bytes.to_vec();
    changed[30] ^= 1;
    assert_eq!(
        verify(&changed, &file, std::slice::from_ref(&public)),
        Err(Error::Mismatch),
        "改了一个字节"
    );
    let other = generate().expect("生成得了");
    let other_public = PublicKey::from_base64(&other.public).expect("读得成");
    assert_eq!(
        verify(bytes, &file, std::slice::from_ref(&other_public)),
        Err(Error::UnknownKey(made.id.clone())),
        "只认得别的钥匙"
    );
    let forged = Signature {
        key: other.id.clone(),
        signature: signed.signature.clone(),
    };
    assert_eq!(
        verify(
            bytes,
            &serde_json::to_vec(&forged).expect("写得成"),
            &[public, other_public]
        ),
        Err(Error::Mismatch),
        "冒用别的钥匙的编号"
    );
}

#[test]
fn broken_inputs_are_errors() {
    assert_eq!(sign(b"x", "not base64!"), Err(Error::BadSecret));
    assert_eq!(sign(b"x", "AAAA"), Err(Error::BadSecret));
    assert!(matches!(
        verify(b"x", b"not json", &[]),
        Err(Error::BadSignatureFile(_))
    ));
    assert!(PublicKey::from_base64("AAAA").is_none(), "不是 32 字节");
}

#[test]
fn the_official_keys_read_and_match_their_ids() {
    for (id, text) in OFFICIAL {
        let key = PublicKey::from_base64(text).expect("编进来的公钥读得成");
        assert_eq!(key.id, *id);
    }
    assert_eq!(official().len(), OFFICIAL.len());
}

#[test]
fn the_official_source_has_a_key() {
    assert!(
        !official().is_empty(),
        "官方软件源的公钥编进来了（施工 G-7）"
    );
}
