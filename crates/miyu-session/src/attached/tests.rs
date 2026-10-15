//! 附件造成块的测试（施工 C-9）：字在前、附件照先后；内容存进这个会话的 blob，块里是哈希；图片带名字和宽高，两种都带原来
//! 的路径；没带附件的只有字，不碰磁盘；存不下的整句不发。

use miyu_kernel::id::{FileName, MediaType, SourcePath};

use super::*;

/// 一个用完就删的临时目录。
struct Scratch(std::path::PathBuf);

impl Scratch {
    fn new(name: &str) -> Scratch {
        let dir = std::env::temp_dir().join(format!("miyu-attached-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        Scratch(dir)
    }
}

impl Drop for Scratch {
    #[expect(
        clippy::let_underscore_must_use,
        reason = "删不掉就留在临时目录里，不影响测试"
    )]
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn attached(name: &str, bytes: &[u8], kind: Kind) -> Attached {
    Attached {
        name: FileName::parse(name).unwrap(),
        path: Some(SourcePath::parse(&format!("/work/{name}")).unwrap()),
        bytes: bytes.to_vec(),
        kind,
    }
}

#[tokio::test]
async fn the_words_come_first_and_each_attachment_is_stored() {
    let scratch = Scratch::new("stored");
    let blobs = Blobs::new(scratch.0.join("blobs"));
    let png = MediaType::parse("image/png").unwrap();
    let text = MediaType::parse("text/plain").unwrap();
    let picture = attached(
        "dot.png",
        b"picture bytes",
        Kind::Image {
            media_type: png.clone(),
            width: 2,
            height: 1,
        },
    );
    let note = attached(
        "note.txt",
        b"hello",
        Kind::File {
            media_type: text.clone(),
        },
    );
    let blocks = blocks(&blobs, "See these.", vec![picture, note])
        .await
        .unwrap();
    let [Block::Text(said), Block::Image(image), Block::File(file)] = blocks.as_slice() else {
        panic!("字、图、文件：{blocks:?}");
    };
    assert_eq!(said.text, "See these.");
    assert_eq!(blobs.get(&image.blob).unwrap(), b"picture bytes");
    assert_eq!(image.name.as_ref().map(FileName::as_str), Some("dot.png"));
    assert_eq!((image.width, image.height, &image.media_type), (2, 1, &png));
    assert_eq!(
        image.path.as_ref().map(SourcePath::as_str),
        Some("/work/dot.png")
    );
    assert_eq!(blobs.get(&file.blob).unwrap(), b"hello");
    assert_eq!((file.name.as_str(), &file.media_type), ("note.txt", &text));
    assert_eq!(
        file.path.as_ref().map(SourcePath::as_str),
        Some("/work/note.txt")
    );
}

#[tokio::test]
async fn without_attachments_only_the_words_and_no_disk() {
    // blob 的目录是个不存在、也建不起来的地方：碰了磁盘就会出错。
    let scratch = Scratch::new("none");
    std::fs::write(scratch.0.join("file"), b"").unwrap();
    let blobs = Blobs::new(scratch.0.join("file").join("blobs"));
    let blocks = blocks(&blobs, "Hi.", Vec::new()).await.unwrap();
    assert_eq!(
        blocks,
        [Block::Text(Text {
            text: "Hi.".to_string()
        })]
    );
    let note = attached(
        "note.txt",
        b"hello",
        Kind::File {
            media_type: MediaType::parse("text/plain").unwrap(),
        },
    );
    assert!(blocks_err(&blobs, note).await, "存不下的整句不发");
}

/// 带一个附件、存不下：交回的是出错。
async fn blocks_err(blobs: &Blobs, one: Attached) -> bool {
    blocks(blobs, "Hi.", vec![one]).await.is_err()
}
