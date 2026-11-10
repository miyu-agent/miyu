//! 读不了的会话照原因分开拒（施工 YC-2）：读写出错的不再说成损坏。

use std::io;
use std::path::PathBuf;

use miyu_kernel::id::ContentHash;
use miyu_session::LoadError;
use miyu_store::blob::BlobError;
use miyu_store::log::OpenError;

use crate::refusal::Refusal;

const HASH: &str = "sha256:97f5f58cebf9e368ddcc668976ce5da07ceb80c7be52ac6d4edcf2ac8a639894";

fn hash() -> ContentHash {
    ContentHash::parse(HASH).expect("合写法")
}

fn broken() -> OpenError {
    OpenError::Broken {
        segment: PathBuf::from("000000000001.jsonl"),
        line: 93,
        why: "not JSON".to_string(),
    }
}

fn too_many_files() -> io::Error {
    io::Error::other("Too many open files")
}

#[test]
fn a_log_that_cannot_be_read_right_now_is_unavailable_not_broken() {
    assert_eq!(
        Refusal::of_log(&OpenError::Io(too_many_files())),
        Refusal::UNAVAILABLE
    );
    assert_eq!(Refusal::of_log(&broken()), Refusal::BROKEN);
    assert_eq!(
        Refusal::of_log(&OpenError::Missing(PathBuf::from("x"))),
        Refusal::NOT_FOUND
    );
}

#[test]
fn loading_tells_a_passing_error_from_a_broken_session() {
    assert_eq!(
        Refusal::of_load(&LoadError::Log(OpenError::Io(too_many_files()))),
        Refusal::UNAVAILABLE
    );
    assert_eq!(
        Refusal::of_load(&LoadError::Blob(BlobError::Io(too_many_files()))),
        Refusal::UNAVAILABLE
    );
    assert_eq!(Refusal::of_load(&LoadError::Log(broken())), Refusal::BROKEN);
    assert_eq!(Refusal::of_load(&LoadError::NotCreated), Refusal::BROKEN);
    assert_eq!(
        Refusal::of_load(&LoadError::Blob(BlobError::Missing(hash()))),
        Refusal::BROKEN
    );
    assert_eq!(
        Refusal::of_load(&LoadError::Blob(BlobError::Corrupt(hash()))),
        Refusal::BROKEN
    );
    assert_eq!(
        Refusal::of_load(&LoadError::Log(OpenError::Missing(PathBuf::from("x")))),
        Refusal::NOT_FOUND
    );
}
