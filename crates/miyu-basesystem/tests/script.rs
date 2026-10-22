//! 脚本当工具（施工 X-12 上，`docs/blueprint/skills.md`「脚本」第 3 条）：照 `#!` 起；参数从环境变量 `MIYU_ARGS_JSON`、标准输入
//! 都拿得到；在这一轮的工作目录里跑；有一个能写的缓存目录；退出码不是 0 的出错；到时整组杀掉、说一句；起不来的说清楚。
//! 交图（施工 X-35）：拷进 `MIYU_SCRIPT_IMAGES_DIR` 的照文件名排、至多 4 张，链接、超大的不交，到时被杀的不看，跑完目录删掉。
//! 链接成的脚本照它指到的真实位置起（施工 X-37）。调用的事实进 `MIYU_CALLER_*`，开头的 `Venues` 定给哪种会话（施工 X-43）。Python 的几条三个平台都跑（Windows 上照 `python` 找）；用 `sh` 的、要建链接
//! 的只在 Unix 上。

use serde_json::json;

use miyu_kernel::block::Block;
use miyu_kernel::id::VenueId;
use miyu_tool::{Caller, Done, Seen, Stop, Venues};

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
    let done = site
        .script("scripts/size.py", json!({ "text": long }))
        .await;
    assert!(!done.error, "{}", text(&done));
    let full = json!({ "text": long }).to_string().len();
    assert_eq!(
        text(&done),
        format!("env=0\nstdin={full}\n"),
        "长的只在标准输入里"
    );
    let done = site.script("scripts/size.py", json!({"text": "hi"})).await;
    let small = json!({"text": "hi"}).to_string().len();
    assert_eq!(
        text(&done),
        format!("env={small}\nstdin={small}\n"),
        "小的两处都有"
    );
}

/// 参数正好在上限上（施工 X-37 补）：30720 个字节的两处都有，多一个字节的只在标准输入里。Windows 上整条环境变量最多 32767 个
/// 字，32 KiB 的会超；三个平台都跑。
#[tokio::test]
async fn arguments_right_at_the_limit_still_come_both_ways() {
    let site = Site::new();
    site.file(
        "scripts/size.py",
        b"#!/usr/bin/env python3\n# Description: Report argument sizes.\n# Parameters: {\"type\": \"object\", \"properties\": {\"text\": {\"type\": \"string\"}}}\nimport os, sys\nprint('env=' + str(len(os.environ.get('MIYU_ARGS_JSON', ''))))\nprint('stdin=' + str(len(sys.stdin.read())))\n",
    );
    // `{"text":"…"}` 的外壳 11 个字节。
    let limit = 30 * 1024;
    for (size, env) in [(limit, limit), (limit + 1, 0)] {
        let args = json!({ "text": "x".repeat(size - 11) });
        assert_eq!(args.to_string().len(), size);
        let done = site.script("scripts/size.py", args).await;
        assert!(!done.error, "{}", text(&done));
        assert_eq!(
            text(&done),
            format!("env={env}\nstdin={size}\n"),
            "{size} 个字节"
        );
    }
}

/// 脚本是链接（施工 X-37）：照它指到的真实位置起，`$0` 是真实的路径。沙盒只放开那一个文件，链接本身在藏着的数据根里，照链接
/// 起的话 macOS 上解释器读不到（Linux 上的两种手段照链接也读得到，这一条在三处都守着「照真实位置起」）。
#[cfg(unix)]
#[tokio::test]
async fn a_linked_script_starts_where_it_really_is() {
    let site = Site::new();
    site.file(
        "elsewhere/where.sh",
        b"#!/bin/sh\n# Description: Where.\necho \"$0\"\n",
    );
    std::fs::create_dir_all(site.0.join("scripts")).expect("建得了");
    std::os::unix::fs::symlink(
        site.0.join("elsewhere/where.sh"),
        site.0.join("scripts/where.sh"),
    )
    .expect("建得了链接");
    let done = site.script("scripts/where.sh", json!({})).await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        text(&done),
        format!("{}\n", site.real("elsewhere/where.sh").display())
    );
}

/// 调用的事实（施工 X-43）：派它的那一轮交的场所、是不是主人进 `MIYU_CALLER_VENUE`、`MIYU_CALLER_OWNER`；没有的（会话外面的
/// 调用）两个都没有；核心自己的环境里残留的不带进去（`MIYU_*` 本来就不传，施工 4-8）。Python 的，三个平台都跑。
#[tokio::test]
async fn a_script_gets_who_called_it_and_nothing_left_over() {
    if !crate::support::in_child(
        "script::a_script_gets_who_called_it_and_nothing_left_over",
        &[
            ("MIYU_CALLER_VENUE", "qq:group:9"),
            ("MIYU_CALLER_OWNER", "1"),
        ],
    ) {
        return;
    }
    let site = Site::new();
    site.file(
        "scripts/who.py",
        b"#!/usr/bin/env python3\n# Description: Who.\nimport os\nget = lambda name: os.environ.get(name, 'unset')\nprint(get('MIYU_CALLER_VENUE') + '|' + get('MIYU_CALLER_OWNER'))\n",
    );
    for (caller, printed) in [
        (Some(("qq:group:1", false)), "qq:group:1|0\n"),
        (Some(("qq:private:10001", true)), "qq:private:10001|1\n"),
        (Some(("local", true)), "local|1\n"),
        (None, "unset|unset\n"),
    ] {
        let caller = caller.map(|(venue, owner)| Caller {
            venue: VenueId::parse(venue).expect("场所合写法"),
            owner,
            model: None,
            log: None,
        });
        let done = site.script_as("scripts/who.py", json!({}), caller).await;
        assert!(!done.error, "{}", text(&done));
        assert_eq!(text(&done), printed);
    }
}

/// 开头写了 `Venues` 的（施工 X-43）：工具照它给哪种会话；没写的没有，照工具面自己的规矩（哪种会话都有）。
#[test]
fn a_script_is_offered_where_its_head_says() {
    let site = Site::new();
    site.file(
        "scripts/near.py",
        b"# Description: Near.\n# Venues: local, private\n",
    );
    site.file("scripts/any.py", b"# Description: Anywhere.\n");
    assert_eq!(
        site.script_tool("scripts/near.py").venues(),
        Some(Venues {
            local: true,
            private: true,
            group: false,
        })
    );
    assert_eq!(site.script_tool("scripts/any.py").venues(), None);
}

/// 印出状态目录、在里面数一次跑了几回的脚本（施工 X-44）：没给的印 `unset`。
const COUNT: &[u8] = b"#!/usr/bin/env python3\n# Description: Count.\nimport os, pathlib\nfolder = os.environ.get('MIYU_SCRIPT_STATE_DIR')\nif folder is None:\n    print('unset')\nelse:\n    mark = pathlib.Path(folder) / 'n'\n    n = int(mark.read_text()) + 1 if mark.exists() else 1\n    mark.write_text(str(n))\n    print(n, folder)\n";

/// 状态目录（施工 X-44）：照会话属主、来处、工具名在数据根的 `state/scripts/` 下面建好交给 `MIYU_SCRIPT_STATE_DIR`，写进去的
/// 下一次还在；换一个属主、换一个来处都是另一格；会话外面的调用（没有属主）不给。Python 的，三个平台都跑。
#[tokio::test]
async fn a_script_keeps_its_state_per_owner_and_source() {
    let site = Site::new();
    site.file("scripts/count.py", COUNT);
    let state = |owner: &str, source: &str| {
        let folder = site
            .0
            .join("data/state/scripts")
            .join(owner)
            .join(source)
            .join("count");
        std::fs::canonicalize(folder).expect("建好了")
    };
    let printed = |done: &Done| text(done).trim_end().to_string();
    let first = site.script_by("scripts/count.py", "~home", "alice").await;
    assert!(!first.error, "{}", text(&first));
    assert_eq!(
        printed(&first),
        format!("1 {}", state("alice", "~home").display())
    );
    let again = site.script_by("scripts/count.py", "~home", "alice").await;
    assert_eq!(
        printed(&again),
        format!("2 {}", state("alice", "~home").display()),
        "下一次还在"
    );
    let bob = site.script_by("scripts/count.py", "~home", "bob").await;
    assert_eq!(
        printed(&bob),
        format!("1 {}", state("bob", "~home").display()),
        "另一个属主另一格"
    );
    let kit = site.script_by("scripts/count.py", "kit", "alice").await;
    assert_eq!(
        printed(&kit),
        format!("1 {}", state("alice", "kit").display()),
        "另一个来处另一格"
    );
    let outside = site.script("scripts/count.py", json!({})).await;
    assert_eq!(printed(&outside), "unset", "没有属主的不给");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(state("alice", "~home"))
            .expect("在")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o700, "只有本人进得去");
    }
}

/// 状态目录建不成的（施工 X-44）：这一次不给，照常跑。
#[tokio::test]
async fn a_script_runs_without_state_when_the_folder_cannot_be_made() {
    let site = Site::new();
    site.file("scripts/count.py", COUNT);
    site.file("data/state/scripts", b"not a folder");
    let done = site.script_by("scripts/count.py", "~home", "alice").await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(text(&done), "unset\n");
}

/// 进度行（施工 X-45）：一整行以 `miyu-progress: ` 开头的，写在标准错误、标准输出都一样，跑的时候推给头、不进结果；别的行照写
/// 的先后在。Python 的，三个平台都跑。
#[tokio::test]
async fn progress_lines_reach_the_head_but_not_the_result() {
    let site = Site::new();
    site.file(
        "scripts/slow.py",
        b"#!/usr/bin/env python3\n# Description: Slow.\nimport sys\nprint('start', flush=True)\nprint('miyu-progress: 1/2', file=sys.stderr, flush=True)\nprint('miyu-progress: 2/2', flush=True)\nprint('done', flush=True)\nsys.stdout.write('mi')\n",
    );
    let (done, pushed) = site.script_progress("scripts/slow.py").await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        text(&done),
        "start\ndone\nmi\n",
        "读完时攒着的开头照一整行留着"
    );
    let pushed = pushed.concat();
    assert!(
        pushed.contains("miyu-progress: 1/2") && pushed.contains("miyu-progress: 2/2"),
        "推给头的有进度行：{pushed:?}"
    );
}

/// 工具登记的参数，读成 JSON。
fn parameters_of(tool: &std::sync::Arc<dyn miyu_tool::Tool>) -> serde_json::Value {
    serde_json::from_str(tool.spec().parameters.get()).expect("参数是 JSON")
}

/// 脚本也能这一次在沙盒外跑（施工 5-14 改下，同 `shell` 的 `outside_sandbox`）：登记时参数里多一格布尔，写了 `true` 的这次要在
/// 沙盒外跑（权限策略照它问人）；交给脚本之前去掉这一格，脚本只拿到自己声明的参数。
#[tokio::test]
async fn a_script_can_ask_to_run_outside_the_sandbox_once() {
    let site = Site::new();
    site.file(
        "scripts/echo.py",
        b"#!/usr/bin/env python3\n# Description: Echo the arguments.\n# Parameters: {\"type\": \"object\", \"properties\": {\"city\": {\"type\": \"string\"}}, \"additionalProperties\": false}\nimport os, sys\nprint('env=' + os.environ['MIYU_ARGS_JSON'])\nprint('stdin=' + sys.stdin.read())\n",
    );
    let tool = site.script_tool("scripts/echo.py");
    let parameters = parameters_of(&tool);
    let property = &parameters["properties"]["outside_sandbox"];
    assert_eq!(property["type"], "boolean", "{parameters}");
    assert!(
        property["description"]
            .as_str()
            .is_some_and(|text| text.contains("sandbox")),
        "{property}"
    );
    let call = |args: serde_json::Value| site.call_for("work", args, Seen::new(), Stop::default());
    assert!(tool.outside_sandbox(&call(json!({"city": "东京", "outside_sandbox": true}))));
    assert!(!tool.outside_sandbox(&call(json!({"city": "东京", "outside_sandbox": false}))));
    assert!(!tool.outside_sandbox(&call(json!({"city": "东京"}))));
    let done = site
        .script(
            "scripts/echo.py",
            json!({"city": "东京", "outside_sandbox": true}),
        )
        .await;
    assert!(!done.error, "{}", text(&done));
    assert_eq!(
        text(&done),
        "env={\"city\":\"东京\"}\nstdin={\"city\":\"东京\"}\n",
        "交给脚本的去掉了这一格"
    );
}

/// 脚本自己的参数里本来就有 `outside_sandbox`（施工 5-14 改下，主会话定）：不加这一格、不覆盖，照原样交给脚本；它这一格不让她
/// 越过沙盒。
#[tokio::test]
async fn a_script_s_own_outside_sandbox_parameter_is_left_alone() {
    let site = Site::new();
    site.file(
        "scripts/own.py",
        b"#!/usr/bin/env python3\n# Description: Has its own flag.\n# Parameters: {\"type\": \"object\", \"properties\": {\"outside_sandbox\": {\"type\": \"string\", \"description\": \"Where to look.\"}}}\nimport sys\nprint('stdin=' + sys.stdin.read())\n",
    );
    let tool = site.script_tool("scripts/own.py");
    assert_eq!(
        parameters_of(&tool)["properties"]["outside_sandbox"],
        json!({"type": "string", "description": "Where to look."})
    );
    let args = json!({"outside_sandbox": true});
    let call = site.call_for("work", args.clone(), Seen::new(), Stop::default());
    assert!(!tool.outside_sandbox(&call), "脚本自己的那一格不算");
    let done = site.script("scripts/own.py", args).await;
    assert_eq!(text(&done), "stdin={\"outside_sandbox\":true}\n");
}
