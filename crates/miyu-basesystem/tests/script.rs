//! 脚本当工具（施工 X-12 上，`docs/blueprint/skills.md`「脚本」第 3 条）：照 `#!` 起；参数从环境变量 `MIYU_ARGS_JSON`、标准输入
//! 都拿得到；在这一轮的工作目录里跑；有一个能写的缓存目录；退出码不是 0 的出错；到时整组杀掉、说一句；起不来的说清楚。
//! 交图（施工 X-35）：拷进 `MIYU_SCRIPT_IMAGES_DIR` 的照文件名排、至多 4 张，链接、超大的不交，到时被杀的不看，跑完目录删掉。
//! Python 的几条三个平台都跑（Windows 上照 `python` 找）；用 `sh` 的、要建链接的只在 Unix 上。

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_tool::Done;

use crate::support::Site;

fn text(done: &Done) -> String {
    done.blocks
        .iter()
        .map(|block| match block {
            Block::Text(text) => text.text.clone(),
            other => panic!("只该有字：{other:?}"),
        })
        .collect()
}

#[tokio::test]
async fn a_python_script_gets_its_arguments_both_ways() {
    let site = Site::new();
    site.file(
        "scripts/echo.py",
        b"#!/usr/bin/env python3\n# Description: Echo the arguments.\nimport os, sys\nprint('env=' + os.environ['MIYU_ARGS_JSON'])\nprint('stdin=' + sys.stdin.read())\nprint('cache=' + os.path.basename(os.environ['MIYU_SCRIPT_CACHE_DIR']))\nprint('cwd=' + os.path.basename(os.getcwd()))\n",
    );
    let done = site
        .script("scripts/echo.py", json!({"city": "东京"}))
        .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        text(&done),
        "env={\"city\":\"东京\"}\nstdin={\"city\":\"东京\"}\ncache=echo\ncwd=work\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_failing_script_is_an_error_with_its_exit_code() {
    let site = Site::new();
    site.file(
        "scripts/fail.sh",
        b"#!/bin/sh\n# Description: Fail.\necho partial\nexit 3\n",
    );
    let done = site.script("scripts/fail.sh", json!({})).await;
    assert!(done.error);
    assert_eq!(text(&done), "partial\nExit code 3\n");
}

#[cfg(unix)]
#[tokio::test]
async fn a_slow_script_is_stopped_at_its_timeout() {
    let site = Site::new();
    site.file(
        "scripts/slow.sh",
        b"#!/bin/sh\n# Description: Slow.\n# Timeout: 1\necho started\nsleep 30\n",
    );
    let started = std::time::Instant::now();
    let done = site.script("scripts/slow.sh", json!({})).await;
    assert!(done.error);
    assert!(
        started.elapsed() < std::time::Duration::from_secs(20),
        "整组杀掉了"
    );
    assert_eq!(
        text(&done),
        "started\nStopped after 1 s because the script took too long.\n"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_missing_interpreter_says_it_could_not_run() {
    let site = Site::new();
    site.file(
        "scripts/odd.sh",
        b"#!/no/such/interpreter\n# Description: Odd.\n",
    );
    let done = site.script("scripts/odd.sh", json!({})).await;
    assert!(done.error);
    assert!(
        text(&done).starts_with("Could not run odd: "),
        "{}",
        text(&done)
    );
}

/// 脚本拿到的环境变量和 `shell` 的一样（4-8 补）：核心的照传，像密钥的、`MIYU_*` 不给；自己那两个照设。
#[cfg(unix)]
#[tokio::test]
async fn a_script_gets_the_same_environment_as_shell() {
    if !crate::support::in_child(
        "script::a_script_gets_the_same_environment_as_shell",
        crate::shell::PROBES,
    ) {
        return;
    }
    let site = Site::new();
    let body = format!(
        "#!/bin/sh\n# Description: Probe.\n{}\necho \"${{MIYU_ARGS_JSON-unset}}\"\n",
        crate::shell::probe_script()
    );
    site.file("scripts/probe.sh", body.as_bytes());
    let done = site.script("scripts/probe.sh", json!({})).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "plain|unset|:probe|cat|unset\n{}\n");
}

/// 交图的脚本（施工 X-35）：往 `MIYU_SCRIPT_IMAGES_DIR` 里倒着写边长 `count` 到 1 的 PNG（`03.png` 先写，名字和写的先后
/// 不一样），另放一份文本和一张 5×5 的 BMP（量得出尺寸、不是那四种格式，不交），印出目录的路径。`extra` 是接在后面的几行 Python。
/// 超时 30 秒（施工 X-35 补）：Windows 上一整套测试并着跑时 Python 起来要好几秒，2 秒的还没印出目录就被杀了。
fn frames(extra: &str) -> String {
    timed(extra, 30)
}

/// 同 [`frames`]，超时 `timeout` 秒。
fn timed(extra: &str, timeout: u32) -> String {
    format!(
        "#!/usr/bin/env python3\n# Description: Make frames.\n# Parameters: {{\"type\": \"object\", \"properties\": {{\"count\": {{\"type\": \"integer\"}}}}}}\n# Timeout: {timeout}\nimport json, os, struct\nargs = json.loads(os.environ['MIYU_ARGS_JSON'])\nout = os.environ['MIYU_SCRIPT_IMAGES_DIR']\ndef png(side, pad=0):\n    return b'\\x89PNG\\r\\n\\x1a\\n' + struct.pack('>I', 13) + b'IHDR' + struct.pack('>II', side, side) + b'\\x08\\x06\\x00\\x00\\x00' + b'\\x00' * (4 + pad)\nfor side in range(args['count'], 0, -1):\n    with open(os.path.join(out, '%02d.png' % side), 'wb') as f:\n        f.write(png(side))\nwith open(os.path.join(out, 'notes.txt'), 'w') as f:\n    f.write('not an image')\nwith open(os.path.join(out, 'zz.bmp'), 'wb') as f:\n    f.write(b'BM' + b'\\x00' * 12 + struct.pack('<IiiHH', 40, 5, 5, 1, 24) + b'\\x00' * 24)\nprint('dir=' + out)\n{extra}"
    )
}

/// 交上来的几张图的边长。
fn sides(done: &Done) -> Vec<u32> {
    done.images
        .iter()
        .map(|picture| {
            assert_eq!(picture.width, picture.height);
            picture.width
        })
        .collect()
}

/// 脚本印的目录。
fn folder(done: &Done) -> std::path::PathBuf {
    let text = text(done);
    let dir = text
        .lines()
        .find_map(|line| line.strip_prefix("dir="))
        .unwrap_or_else(|| panic!("印了目录：{text}"));
    std::path::PathBuf::from(dir)
}

#[tokio::test]
async fn images_put_in_the_folder_are_handed_over_in_name_order() {
    let site = Site::new();
    site.file("scripts/frames.py", frames("").as_bytes());
    let done = site.script("scripts/frames.py", json!({"count": 2})).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(sides(&done), [1, 2], "照文件名排，文本不交");
    assert!(
        text(&done).starts_with("dir="),
        "字照脚本印的原样：{}",
        text(&done)
    );
    assert!(!folder(&done).exists(), "跑完目录删掉了");
}

#[tokio::test]
async fn at_most_four_images_are_handed_over() {
    let site = Site::new();
    site.file("scripts/frames.py", frames("").as_bytes());
    let done = site.script("scripts/frames.py", json!({"count": 6})).await;
    assert_eq!(sides(&done), [1, 2, 3, 4]);
}

#[tokio::test]
async fn a_failing_script_still_hands_over_its_images() {
    let site = Site::new();
    site.file(
        "scripts/frames.py",
        frames("raise SystemExit(3)\n").as_bytes(),
    );
    let done = site.script("scripts/frames.py", json!({"count": 1})).await;
    assert!(done.error);
    assert_eq!(sides(&done), [1]);
}

#[tokio::test]
async fn a_script_stopped_at_its_timeout_hands_over_nothing() {
    let site = Site::new();
    site.file(
        "scripts/frames.py",
        // 20 秒：Windows CI 上整套并着跑时 Python 印出目录要好几秒，8 秒的也到时被杀过；睡 120 秒，到时一定还在跑。
        timed(
            "import sys, time\nsys.stdout.flush()\ntime.sleep(120)\n",
            20,
        )
        .as_bytes(),
    );
    let done = site.script("scripts/frames.py", json!({"count": 1})).await;
    assert!(done.error);
    assert!(done.images.is_empty(), "到时被杀的不看");
    assert!(!folder(&done).exists(), "目录照样删掉");
}

/// 链接不跟、超过 5 MiB 的不读（施工 X-35，主会话审时提）：`01.png` 是指向 `02.png` 的链接，`03.png` 头是 PNG、有 6 MiB。
#[cfg(unix)]
#[tokio::test]
async fn links_and_oversized_files_are_not_handed_over() {
    let site = Site::new();
    let extra = "os.remove(os.path.join(out, '01.png'))\nos.symlink(os.path.join(out, '02.png'), os.path.join(out, '01.png'))\nwith open(os.path.join(out, '03.png'), 'wb') as f:\n    f.write(png(3, 6 * 1024 * 1024))\n";
    site.file("scripts/frames.py", frames(extra).as_bytes());
    let done = site.script("scripts/frames.py", json!({"count": 3})).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(sides(&done), [2]);
}

/// 参数太长（施工 X-37）：超过 32 KiB 的不放进环境变量（Linux 上一个环境变量最多 128 KiB、Windows 上 32767 个字，超了起都
/// 起不来），只在标准输入里；小的照旧两处都有。Python 写的，三个平台都跑。
#[tokio::test]
async fn long_arguments_come_only_on_stdin() {
    let site = Site::new();
    site.file(
        "scripts/size.py",
        b"#!/usr/bin/env python3\n# Description: Report argument sizes.\n# Parameters: {\"type\": \"object\", \"properties\": {\"text\": {\"type\": \"string\"}}}\nimport os, sys\nprint('env=' + str(len(os.environ.get('MIYU_ARGS_JSON', ''))))\nprint('stdin=' + str(len(sys.stdin.read())))\n",
    );
    let long = "x".repeat(40 * 1024);
    let done = site.script("scripts/size.py", json!({ "text": long })).await;
    assert!(!done.error, "{}", text(&done));
    let full = json!({ "text": long }).to_string().len();
    assert_eq!(text(&done), format!("env=0\nstdin={full}\n"), "长的只在标准输入里");
    let done = site.script("scripts/size.py", json!({"text": "hi"})).await;
    let small = json!({"text": "hi"}).to_string().len();
    assert_eq!(text(&done), format!("env={small}\nstdin={small}\n"), "小的两处都有");
}
