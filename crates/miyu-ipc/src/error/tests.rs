//! 起不来、连不上的说法照实说是套接字还是命名管道（施工 WN-1）。

use std::path::Path;

use super::{not_private, occupied};

#[test]
fn a_pipe_is_called_a_named_pipe() {
    let pipe = Path::new(r"\\.\pipe\miyu-x");
    for said in [occupied(pipe, true), not_private(pipe, true)] {
        assert!(said.contains("命名管道"), "{said}");
        assert!(!said.contains("套接字"), "{said}");
        assert!(!said.contains("别人也能进"), "{said}");
    }
    let socket = Path::new("/run/user/1000/miyu-x/core.sock");
    assert!(occupied(socket, false).contains("套接字"));
    assert!(not_private(socket, false).contains("别人也能进"));
}

/// 连不上、起不来的八句都交说法（施工 EN-1），三种语言都取得出字；英文不再是中文原话。
#[test]
fn start_and_connect_errors_have_words() {
    use std::io;

    use miyu_store::human::Human;
    use miyu_store::resources::ResourceRoot;

    use super::{ConnectError, StartError, not_private_said};

    let resources = ResourceRoot::at(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources"));
    let place = Path::new("/run/user/1000/miyu-x");
    let refused = StartError::Refused {
        code: None,
        reason: "boom".to_string(),
    };
    let saids = [
        ConnectError::NotRunning.said(),
        Some(not_private_said(place, false)),
        Some(not_private_said(place, true)),
        refused.said(),
        StartError::Silent.said(),
        StartError::Timeout.said(),
        StartError::Busy.said(),
        StartError::Io(io::Error::other("no such file")).said(),
    ];
    for language in ["zh", "en", "ja"] {
        let words = Human::load(&resources, language).expect("读得出");
        for said in &saids {
            let said = said.as_ref().expect("有说法");
            let text = words.say(said);
            assert!(text.is_some(), "{language} 没有 {said:?}");
        }
    }
    let english = Human::load(&resources, "en").expect("读得出");
    let say =
        |said: Option<miyu_kernel::event::Said>| english.say(&said.expect("有")).expect("有字");
    assert_eq!(
        say(ConnectError::NotRunning.said()),
        "The core isn't running"
    );
    assert_eq!(say(refused.said()), "The core couldn't start: boom");
    assert_eq!(
        say(StartError::Silent.said()),
        format!(
            "The core didn't start and gave no reason; see {} in the data root",
            miyu_store::root::DataRoot::at(std::path::PathBuf::new())
                .core_log()
                .display()
        )
    );
    assert_eq!(ConnectError::Io(io::Error::other("x")).said(), None);
}
