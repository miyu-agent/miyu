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
