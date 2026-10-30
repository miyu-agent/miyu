//! 要说的话从哪来（施工 AP-2）：标准输入用真的管道喂，终端用真的伪终端（只在 Unix 上）。

use std::io::{PipeReader, PipeWriter, Write};
use std::sync::mpsc;
use std::time::Duration;

use super::{Unread, message};

/// 命令行上的几个词。
fn words(words: &[&str]) -> Vec<String> {
    words.iter().map(ToString::to_string).collect()
}

/// 一根写好了 `bytes`、写的一头关上了的管道。
fn piped(bytes: &[u8]) -> PipeReader {
    let (reader, mut writer) = std::io::pipe().expect("开得了管道");
    writer.write_all(bytes).expect("写得进");
    drop(writer);
    reader
}

/// 照 `words` 和一根写着 `bytes` 的管道凑。
fn from_pipe(said: &[&str], bytes: &[u8]) -> Result<String, Unread> {
    message(&words(said), piped(bytes), false)
}

#[test]
fn without_words_the_whole_pipe_is_the_message() {
    assert_eq!(
        from_pipe(&[], "看看这个\n".as_bytes()),
        Ok("看看这个\n".to_string())
    );
    assert_eq!(
        from_pipe(&["-"], b"x"),
        Ok("x".to_string()),
        "只有 `-` 的一样"
    );
}

#[test]
fn a_dash_puts_the_pipe_after_the_words_with_a_blank_line() {
    assert_eq!(
        from_pipe(&["为什么", "失败", "-"], b"error: x\n"),
        Ok("为什么 失败\n\nerror: x\n".to_string())
    );
    assert_eq!(
        from_pipe(&["-", "看看"], b"log"),
        Ok("看看\n\nlog".to_string()),
        "`-` 写在哪都接在后面"
    );
    assert_eq!(
        from_pipe(&["看看", "-"], b" \n"),
        Ok("看看".to_string()),
        "读到空白的照写的"
    );
}

#[test]
fn words_without_a_dash_never_read_the_pipe() {
    // 写的一头一直开着：要是去读，会一直等下去。在另一个线程里凑，最多等十秒。
    let (reader, writer): (PipeReader, PipeWriter) = std::io::pipe().expect("开得了管道");
    let (done, finished) = mpsc::channel();
    std::thread::spawn(move || {
        let _sent = done.send(message(&words(&["在吗"]), reader, false));
    });
    let got = finished.recv_timeout(Duration::from_secs(10));
    drop(writer);
    assert_eq!(got, Ok(Ok("在吗".to_string())), "没读管道");
}

#[test]
fn not_utf8_and_empty_are_refused() {
    assert_eq!(from_pipe(&[], &[0xc3, 0x28]), Err(Unread::NotUtf8));
    assert_eq!(from_pipe(&["看看", "-"], &[0xff]), Err(Unread::NotUtf8));
    for (said, bytes) in [
        (&[][..], &b""[..]),
        (&[], b" \n\t"),
        (&["-"], b""),
        (&[""], b"ignored"),
        (&["  "], b""),
    ] {
        assert_eq!(
            from_pipe(said, bytes),
            Err(Unread::Missing),
            "{said:?} {bytes:?}"
        );
    }
}

/// 标准输入是终端、又没写消息：不等人敲，当没有消息。写了 `-` 的照读（明写的）。
#[cfg(unix)]
#[test]
fn a_terminal_without_words_is_a_missing_message() {
    use std::io::IsTerminal;

    use rustix::pty::{OpenptFlags, grantpt, openpt, ptsname, unlockpt};
    let main = openpt(OpenptFlags::RDWR | OpenptFlags::NOCTTY).expect("开得起伪终端");
    grantpt(&main).expect("放得开");
    unlockpt(&main).expect("解得开");
    let name = ptsname(&main, Vec::new()).expect("问得到名字");
    let slave = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(name.to_str().expect("是 UTF-8"))
        .expect("开得起从端");
    assert!(slave.is_terminal(), "从端是终端");
    let terminal = slave.is_terminal();
    // 终端里先敲好一行再按 Ctrl+D：要是去读，读到的是这一行，不会一直等着人敲。
    let mut main = std::fs::File::from(main);
    main.write_all("在吗\n\u{4}".as_bytes()).expect("敲得进");
    assert_eq!(message(&[], slave, terminal), Err(Unread::Missing));
}
