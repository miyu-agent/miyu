//! 到时整组杀（施工 SH-1 上）：120 秒在测试里换短的。

use std::path::Path;

use super::*;
use crate::common::Common;

fn shell() -> Shell {
    let resources = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../resources");
    let common = Common::load(&resources).expect("资源目录里的字读得出来");
    Shell::load(&resources, common).expect("shell 读得出来")
}

#[tokio::test]
async fn a_command_past_its_time_is_killed_with_what_it_printed() {
    let shell = shell();
    let script = if cfg!(windows) {
        "Write-Output first; Start-Sleep -Seconds 30"
    } else {
        "echo first; sleep 30"
    };
    let command = PersonCommand {
        command: script.to_string(),
        cwd: std::env::temp_dir(),
    };
    let ran = shell
        .person_within(command, Halt::new(), Duration::from_millis(1500))
        .await
        .expect("起得来");
    assert_eq!(ran.ended, PersonEnd::TimedOut);
    assert_eq!((ran.exit, ran.signal), (None, None));
    assert!(ran.text.contains("first"), "{:?}", ran.text);
    assert!(ran.duration_ms < 30_000, "整组杀了，没等它睡完");
}
