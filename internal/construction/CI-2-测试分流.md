## 施工单 CI-2：测试分流

状态：已完成（2026-11-05）。core1 11-04 派：三台 check（尤其 Windows）别再逼近时限；先量每台最耗时的测试，新旧对照类的重测试平时少跑种子，全量挪到 randomized 那一项。11-05 加派两样：ubuntu 的几项开头腾出盘（runner 报「No space left on device」死在门禁开头）；查 Windows 上偶发红的 `a_removed_extension_stops_before_its_files_go`（见「Windows 偶发：卸包先停再删」）。只动测试分流、腾盘和这一条偶发；编译、clippy、文档这几块的建议写在最后，没动。

### 为什么

Windows 的 check 一趟 47～50 分钟，时限 core1 已从 45 放到 60（CI-1）。V-39 以后每一步性能活都带一个「新旧逐字比」的对照测试，几千个种子，debug 下跑：Windows 上 `miyu-endpoint` 的库测试一个程序就要 345 秒，一半以上的测试时间是这些对照测试。它们比的是纯逻辑（同一份输入新旧两种读法交的一样），种子多少只决定覆盖多宽，不用每一台、每一趟都跑满。

ubuntu 的 check 11-04 两次死在编译测试那一步：托管机器的盘被第三方依赖的缓存加 debug 的测试程序吃满（派活时说可用的只有约 14 GB）（`ld terminated with signal 7 [Bus error]`、`rustc-LLVM ERROR: IO failure on output stream: No space left on device`，runner 报剩 0 MB）。

### 量（改前）

数据：main 上最近三趟整套跑过的全绿 run（41902342833、41899172224、41884523614，11-04；之后两趟 main 是「分支上测过」直接跳过的，不算）。日志下在 `scratchpad/ci2/`。`cargo xtask check` 里 `cargo test --quiet` 不打程序名，照每个程序的测试个数和本机 `cargo test --workspace -- --list` 的先后对上；单个测试的时间是本机 Linux debug、4 线程（同 CI 的核数）、`--report-time` 量的。

#### 每项一趟（三趟的中位，分钟）

| 项 | 改前 | 改后 |
|---|---|---|
| check（windows-latest） | 48.7（47.3～50.0） | 37.9～42.5（四次：37.9、38.4、38.7、42.5，托管机器之间差约一成） |
| check（ubuntu-24.04） | 22.3（15.5～24.6） | 14.0～18.8（含腾盘 0.6～1.9） |
| check（macos-latest） | 26.4（12.7～27.0） | 19.7～22.6 |
| randomized | 3.1（2.5～4.1） | 22.8～28.8（时限 45） |
| perf | 10.0（10.0～10.7） | 10.4（含腾盘 0.6～0.7） |
| tested | 0.1 | 0.1 |

#### check 里每一步（三趟的中位，秒）

| 步 | Windows | ubuntu | macOS |
|---|---|---|---|
| 取代码、装工具链、取缓存 | 56 | 25 | 26 |
| 编 xtask | 53 | 22 | 34 |
| 格式 | 11 | 6 | 7 |
| clippy（整个工作区） | 197 | 108 | 148 |
| clippy（核心不开可选包） | 59 | 36 | 46 |
| 文档 | 59 | 38 | 50 |
| 许可证 | 5 | 3 | 5 |
| 编测试（`cargo test --no-run`） | 808 | 310 | 442 |
| 跑测试 | 1629 | 786 | 790 |

Windows 上跑测试占 56%，编测试占 28%，clippy、文档合起来 11%。

#### 每台前 20 名的测试程序（三趟的中位，秒）

类别：对照＝新旧逐字比、照种子造数据；随机＝随机造数据查性质；起停＝每个测试起一个真核心（进程内或子进程）、临时数据根；等待＝等超时、等网络、等文件监视。

| 名次 | Windows | 秒 | ubuntu | 秒 | macOS | 秒 |
|---|---|---|---|---|---|---|
| 1 | `miyu-endpoint` 库（对照） | 344.8 | `miyu-endpoint` 库（对照） | 311.3 | `miyu-endpoint` 库（对照） | 246.5 |
| 2 | `miyu-endpoint` all（起停） | 335.6 | `miyu-assemble` 库（对照） | 73.5 | `miyu-endpoint` all（起停） | 79.2 |
| 3 | `miyu-session` all（起停，一个对照） | 293.9 | `miyu-session` 库（对照） | 73.4 | `miyu-session` all（起停，一个对照） | 77.4 |
| 4 | `miyu-session` 库（对照） | 110.1 | `miyu-session` all（起停，一个对照） | 63.4 | `miyu-session` 库（对照） | 62.6 |
| 5 | `miyu-assemble` 库（对照） | 107.8 | `miyu` all（起停，子进程） | 52.8 | `miyu-assemble` 库（对照） | 59.5 |
| 6 | `miyu` all（起停，子进程） | 70.2 | `miyu-endpoint` all（起停） | 46.8 | `miyu` all（起停，子进程） | 51.3 |
| 7 | `miyu-cli` all（起停） | 41.2 | `miyu-endpoint` providers_log（等待） | 30.2 | `miyu-endpoint` providers_log（等待） | 30.5 |
| 8 | `miyu-endpoint` providers_log（等待） | 30.6 | `miyu-basesystem` all（等待、子进程） | 22.3 | `miyu-basesystem` all（等待、子进程） | 24.0 |
| 9 | `miyu-store` 库（对照） | 27.7 | `miyu-cli` all（起停） | 20.6 | `miyu-cli` all（起停） | 21.6 |
| 10 | `miyu-basesystem` all（等待、子进程） | 26.2 | `miyu-store` 库（对照） | 18.3 | `miyu-store` 库（对照） | 17.9 |
| 11 | `miyu-net` all（等待、网络） | 24.3 | `miyu-assemble` all（随机） | 14.2 | `miyu-assemble` all（随机） | 15.2 |
| 12 | `miyu-store` all（文件） | 18.0 | `miyu-kernel` 库（随机、纯逻辑） | 11.0 | `miyu-wallpaper` 库（等待） | 10.2 |
| 13 | `miyu-assemble` all（随机） | 17.6 | `miyu-drivers` all（对照、流） | 9.2 | `miyu-kernel` 库（随机、纯逻辑） | 9.8 |
| 14 | `miyu-kernel` 库（随机、纯逻辑） | 14.7 | `miyu-basesystem` read_memory（量内存） | 9.2 | `miyu-basesystem` 库（对照、子进程） | 9.4 |
| 15 | `miyu-drivers` all（对照、流） | 11.0 | `miyu-wallpaper` 库（等待） | 8.6 | `miyu-drivers` all（对照、流） | 9.1 |
| 16 | `miyu-endpoint` robust_methods（起停） | 10.2 | `miyu-basesystem` 库（对照、子进程） | 7.6 | `miyu-endpoint` robust_methods（起停） | 7.6 |
| 17 | `miyu-wallpaper` 库（等待） | 8.7 | `miyu-endpoint` robust_methods（起停） | 6.8 | `miyu-core` all（起停） | 7.0 |
| 18 | `miyu-core` all（起停） | 8.4 | `miyu-recall` 库（对照） | 6.6 | `miyu-view` all（随机） | 6.6 |
| 19 | `miyu-view` all（随机） | 8.2 | `miyu-view` all（随机） | 6.1 | `miyu-recall` 库（对照） | 5.6 |
| 20 | `miyu-basesystem` 库（对照、子进程） | 8.2 | `miyu-core` all（起停） | 5.4 | `miyu-http` all（网络） | 5.2 |

测试程序一个接一个跑，程序里的测试 4 个线程并着跑，所以一个程序的时间约等于「它的测试加起来 ÷ 4」和「最慢的那一个」里大的那个。ubuntu 的 `miyu-endpoint` 库三趟是 131、265、311 秒，托管机器之间差得多。

#### 最耗时的单个测试（本机 Linux debug、4 线程，秒；CI 上 Windows 报过「跑了 60 秒以上」的标 ★）

| 秒 | 测试 | 类 |
|---|---|---|
| 35.3 | `miyu-endpoint` `undo::tests::same::undo_reports_are_the_same_as_reading_the_whole_log` ★ | 对照 |
| 32.8 | `miyu-endpoint` `asked::tests::the_same_as_the_whole_read_over_seeded_sessions` ★ | 对照 |
| 30.1 | `miyu-endpoint` providers_log `a_test_is_one_line_and_the_pasted_key_is_nowhere` | 等待（列模型 30 秒到时） |
| 26.6 | `miyu-session` `store::tests::seen::seen_files_are_the_same_as_reading_the_whole_log` ★ | 对照 |
| 23.5 | `miyu-session` all `fork_streamed::handed::a_handed_base_loads_what_reading_the_parent_again_did` ★ | 对照 |
| 23.3 | `miyu-session` `actor::extract::planned::tests::swept::reading_once_plans_…` ★ | 对照 |
| 20.6～23.0 ×6 | `miyu-endpoint` `venues::records::tests::same::chats_read_the_same_as_before_1..6` ★ | 对照 |
| 20.0 | `miyu-basesystem` all `script::a_script_stopped_at_its_timeout_hands_over_nothing` | 等待（脚本 20 秒到时） |
| 19.8 | `miyu-endpoint` `venues::records::tests::files::logs_in_segments_read_the_same_as_before` ★ | 对照 |
| 13.1～18.8 ×6 | `miyu-endpoint` `view::read::tests::same::every_page_reads_like_the_whole_0..5` ★ | 对照 |
| 13.3～16.7 ×6 | `miyu-assemble` `group::records::tests::same::chats_render_the_same_as_before_1..6` ★ | 对照 |
| 11.1 | `miyu-session` all `fork_subagent::limits::a_fork_stops_at_the_fork_step_limit` | 起停（真跑 500 步） |
| 10.6 | `miyu-endpoint` `view::read::tests::same::a_broken_line_is_reported_…` | 对照 |
| 10.6 | `miyu-assemble` all `random_logs::five_hundred_random_sessions_keep_the_properties` | 随机（长跑版另在 randomized） |
| 8.6 | `miyu-basesystem` read_memory `reading_one_page_does_not_hold_the_whole_file` | 量内存 |
| 8.6 | `miyu-wallpaper` `tests::watch_calls_back_when_listed_files_change_and_only_then` | 等待（文件监视） |
| 7.3 | `miyu-endpoint` robust_methods `every_method_answers_odd_params_and_keeps_the_connection` | 起停 |
| 6.9 | `miyu-session` all `memory_extract_failing::one_success_resets_the_wait` | 等待 |
| 6.9 | `miyu-store` `log::open::walk::tests::ten_thousand_events_read_the_same` | 对照 |

`miyu-endpoint` all（1107 个）、`miyu-session` all（654 个）在 Windows 上没有一个超过 60 秒，是每一个都慢：本机 Linux 27、34 秒，Windows 336、294 秒（慢 9～12 倍；ubuntu 慢 1.7～2 倍）。每个测试起一个进程内核心、建临时数据根、写配置、开 SQLite，Windows 上建文件、刷盘贵。这不是对照测试，没有靠少跑糊弄，见「没做的和建议」。

### 怎么分

1. **对照测试平时只跑够快的几份**：默认种子数照 Windows debug 的时长定，本机 Linux debug 一个测试两三秒以内（Windows 上约十来秒）；设环境变量的全量，比的东西和原来一样，只是种子多。已有开关的（`MIYU_UNDO_SEEDS`、`MIYU_V43_SEEDS`、`MIYU_PAGES_FULL`、`MIYU_RECORDS_FULL`）改小默认；没有开关的大头加同样形式的开关：`MIYU_ASKED_FULL`、`MIYU_HANDED_FULL`，`venues/records/tests/files.rs` 跟着同一个模块的 `MIYU_RECORDS_FULL`。`miyu-assemble` 是纯逻辑的层，门禁不许读环境变量（01-架构 第九节规则 3）：它的全量照 `miyu-kernel` 长跑的写法，另写六个名字以 `_for_longer` 结尾的 `#[ignore]` 测试，randomized 原来那一步 `--ignored for_longer` 就跑到。分六份并着跑的取每份的前几段（照原来的种子，确定的）。
2. **数路数的断言照份数折**：「放早了再读一遍的 > 50」这类下限照全量定，平时照份数按比例折。千份里才一两份的退路（`asked` 的骨架认不得、`venues::records` 第一份的整份读）平时那几十、几百份里不一定有，只在全量查走到没有；平时照旧逐条比，走到了也比，只是不断言一定走到。
3. **全量挪到 randomized**：加一步「对照全量」，设上所有开关，照测试名只跑这些对照测试。randomized 原来只编 `miyu-kernel`、`miyu-assemble`，现在加上 `miyu-endpoint`、`miyu-session`、`miyu-basesystem`、`miyu-drivers`、`miyu-sandbox`，只编它们的库和 `tests/all.rs`（`--lib --test all`），长跑那一步也照同一份编（同一组包、同一套 profile，不重编）。整项照 release 编（不开整体优化、16 份代码单元，同原来），另开 `debug-assertions`、`overflow-checks`：全量和 check 里 debug 比的一样，溢出、`debug_assert!` 照样拦。
4. **providers_log 等的 30 秒**：第二个回应写成「一句字以后停住」，落在了后一家列模型那一下，列模型要等 30 秒到时才接着发那一句（施工 8-11 起就这样）。改成后一家列模型回 404（不给列），照写的模型发一句、401：走的路、记的那一行（`provider tested provider=candidate model=m ok=false`）、断言一样，30 秒 → 0.02 秒。确定能快的只有这一个，别的等待见下。
5. **约定写进 `AGENTS.md`「测试」**：以后派对照测试，默认种子照 Windows debug 的时长定，全量加到 randomized「对照全量」那一步。

### 每个测试几份

只在 Linux 上跑的不用说；对照的都是纯逻辑或照真文件读写的（文件是测试自己写的），Windows 上 check 里照旧跑平时那几份，Windows 特有的那一层（开文件、路径）不随种子多少变。没有只在 Windows 上才有意义、要在 Windows 上跑全量的。

| 测试 | 原来 check | 现在 check | randomized |
|---|---|---|---|
| `miyu-endpoint` `undo::tests::same` 三条（主的、坏行 ÷3、分叉的） | 1000 / 333 / 200 | 100 / 33 / 20 | `MIYU_UNDO_SEEDS=5000`：5000 / 1666 / 1000 |
| `miyu-session` `store::tests::seen` 两条（主的、坏行 ÷4） | 1000 / 250 | 100 / 25 | 同上：5000 / 1250 |
| `miyu-endpoint` `asked` 对照 | 3000 | 300 | `MIYU_ASKED_FULL`：3000 |
| `miyu-endpoint` `venues::records::tests::same` ×6 | 3000 段，窗口比三样里的一部分 | 240 段（每份前 40），同 | `MIYU_RECORDS_FULL`：3000 段，每样窗口都比 |
| `miyu-endpoint` `venues::records::tests::files` | 150 | 15 | 同上：150 |
| `miyu-endpoint` `venues::records::tests::recent` | 250 | 80 | 同上：6000 |
| `miyu-endpoint` `view::read::tests::same` ×6 | 90（每份 15） | 18（每份 3） | `MIYU_PAGES_FULL`：3000 |
| `miyu-endpoint` `view::read::tests::same` 坏一行 | 160 | 40 | 同上：160 |
| `miyu-assemble` `group::records::tests::same` ×6 | 3000 | 300（每份前 50） | `…_for_longer` ×6（`#[ignore]`，长跑那一步）：3000 |
| `miyu-session` `swept` 两条（主的、坏行 ÷2） | 240 / 120 | 24 / 12 | `MIYU_V43_SEEDS=2400`：2400 / 1200 |
| `miyu-session` all `fork_streamed::handed` | 2500 | 250 | `MIYU_HANDED_FULL`：2500 |
| `miyu-basesystem` `history::tests::same`、`same::only` | 12 / 12 | 不变 | `MIYU_HISTORY_FULL`：3000 / 1500 |
| `miyu-drivers` all `memo` | 24 | 不变 | `MIYU_MEMO_SEEDS=1000` |
| `miyu-sandbox` `linux::reads::tests::same`（只在 Linux） | 300 | 不变 | `MIYU_V46_SEEDS=50000` |
| `miyu-sandbox` `linux::cache::tests::same`（只在 Linux） | 40 | 不变 | `MIYU_V47_SEEDS=5000` |

randomized 的个数照 45 分钟的时限挑：`MIYU_V47_SEEDS=20000`（V-47 施工时跑过的）在本机 release 一个测试就 246 秒、单线程，CI 上要再慢一两倍，改 5000（约 60 秒）；`MIYU_MEMO_SEEDS`、`MIYU_UNDO_SEEDS`、`MIYU_V43_SEEDS` 同理取 1000、5000、2400，都是原来 check 里的 5～40 倍。

本机 release（同 randomized 那套 profile）照 CI 那组开关跑过一遍全量（`UNDO=10000`、`V43=5000`、`MEMO=2000`、`V47=20000`，比现在 CI 设的还多）：全过，4 线程一共 713 秒，最长的是 `linux::cache` 246 秒、`history::tests::same` 147 秒、`view::read::tests::same` 每份 77 秒、`undo` 44 秒。

### 改后（本机 Linux debug、4 线程）

| 程序 | 改前 | 改后 |
|---|---|---|
| `miyu-endpoint` 库 | 90.6 s（测试加起来 344） | 17.4 s（55） |
| `miyu-assemble` 库 | 30.0 s（89） | 2.6 s（8） |
| `miyu-session` 库 | 33.0 s（70） | 8.7 s（27） |
| `miyu-session` all | 33.7 s（134） | 29.9 s（119） |
| `miyu-endpoint` providers_log | 30.1 s | 0.02 s |

### 腾出盘

ubuntu 上编东西的三项（check、randomized、perf）开头加一步，`sudo rm -rf /usr/share/dotnet /usr/local/lib/android /opt/ghc /usr/local/.ghcup /opt/hostedtoolcache/CodeQL`，删前删后各打一行 `df -h /`。不引第三方 action；`tested` 不编东西，不用。macOS、Windows 的盘没撞过，不动。

缓存（`Swatinem/rust-cache`）没改：它存的只是第三方依赖的编译产物和 `~/.cargo` 的源码，工作区自己的 crate、测试程序、增量目录存之前就清掉（`cache-workspace-crates: false`，日志里 check 一份约 600 MB、perf 351 MB、randomized 104 MB）；撑满盘的是这一趟自己编出来的 debug 测试程序（本机一份 `target` 26 GB）。randomized 的 profile 环境变量变了，缓存的键跟着变，头一趟从头编依赖，main 存了以后照常命中。

改前改后盘还剩多少、缓存多大见「改后实测」。

### 改后实测

三趟：41929612529（`step/CI-2-测试分流`，第一版：`miyu-assemble` 的全量用环境变量开，门禁「纯逻辑」一项拦了，三台 check 红在
门禁结果上，测试本身都跑完、都过，时长照算）、41932485427（`step/CI-2-测试分流-r2`，改成 `_for_longer` 以后，六项全绿）、
41939943755（`step/CI-2-测试分流-r3`，加上偶发那一条的修法；Windows 第一次 38.7 分钟红在和这一步无关的
`memory_recall::nothing_is_brought_unless_it_is_sure`（向量小程序冷启动超过产品的 1 秒等待，另记 FL-4），只重跑那一项，第二次 42.5
分钟绿：那台机器 clippy、编译、起停测试全都慢一截）。Windows 没有稳在 40 分钟以内：分流以后对照测试在 Windows 上一共只剩一分多钟，
剩下的见「没做的和建议」。

| 项 | 改前（中位） | 第一趟 | 第二趟 | 第三趟 |
|---|---|---|---|---|
| check（windows-latest） | 48.7 | 38.4 | 37.9 | 38.7（红，无关偶发）；重跑 42.5 |
| check（ubuntu-24.04） | 22.3 | 18.8 | 14.0 | 17.3 |
| check（macos-latest） | 26.4 | 22.6 | 19.7 | 20.5 |
| randomized | 3.1 | 28.8 | 22.8 | 28.6 |
| perf | 10.0 | 10.5 | 10.4 | 10.8 |

Windows 一趟里（第二趟，秒）：跑测试 1629 → 1061，编测试 808 → 777，clippy 两遍、文档、格式、许可证同改前。程序：`miyu-endpoint`
库 345 → 62、`miyu-assemble` 库 108 → 10、`miyu-session` 库 110 → 46、`miyu-session` all 294 → 257、providers_log 31 → 不到 1；
剩下最大的是 `miyu-endpoint` all 316、`miyu-session` all 257、`miyu` all 69（都是起停，见「没做的和建议」）。

ubuntu 跑测试 786 → 326～440 秒，`miyu-endpoint` 库 311 → 52。macOS 跑测试 790 → 482。

randomized（第二趟，秒）：编 417（第一趟 523，缓存的键没跟着 profile 的环境变量变，新加的五个 crate 的依赖照样要编），长跑那一步
跑 140（内核、assemble 的长跑各 60 上下，assemble 群聊渲染六份全量 15），对照全量 724：`miyu-endpoint` 库 354、`miyu-basesystem`
库（history 全量）190、`miyu-sandbox`（V-47 5000 棵）86、`miyu-session` 库 51、`memo` 32、handed 10。离 45 分钟还有 16～22 分钟。

盘（这两趟拿到的 ubuntu 机器那块盘是 145 GB）：删前剩 86 GB，删后剩 108 GB，腾出 22 GB；删要 0.6～1.9 分钟（check 那台最慢）。
11-04 红的那两趟日志里只有「剩 0 MB」，没打 `df`，拿到的是不是小盘的机器看不出来；以后再撞，日志开头那一行就看得出删前剩多少。
缓存：check 一份 Linux 597 MB、macOS 451 MB、Windows 600 MB，perf 351 MB，randomized 104 MB（main 存了新的一份以后会大一些，
多了五个 crate 的依赖），都没改。

### Windows 偶发：卸包先停再删

main 7eb93d975 那一趟（run 41921321964）Windows 的 check 红在 `miyu-endpoint` all 的
`packages_extensions::a_removed_extension_stops_before_its_files_go`：「等不到 3 行」，记下的只有两行（`cwd:…\var\lib\xgone` 和握手的回应），
少了第三行 `exists:true`。下一趟全绿，最近 40 趟只见过这一次。日志在 `scratchpad/ci2/flake-xgone-win.log`。

- **等的是什么**：测试扩展照参数一步步做：记工作目录、握手、`serve`（读标准输入直到读到头），读到头以后看清单在不在、记一行
  `exists:…`、退出。测试卸包，核心先停扩展（关它的标准输入），等它退出，再删文件；测试等记下的第三行，最多 60 秒。
- **时限**：核心关了输入以后等扩展自己退出最多 `grace`，到了就杀。产品里 5 秒；这个测试用的 `quick()` 是 300 毫秒（为了别的测试
  里退避、杀进程快）。
- **慢在哪**：扩展读到头以后要再做一次文件查询、一次写；Windows 的 CI 忙（4 个测试线程并着起核心、起子进程）的时候，这个子进程
  过了 300 毫秒还没轮到做完，被核心杀了，第三行永远不会来，测试白等 60 秒。不是产品的竞态：卸包照旧先停再删，删的时候扩展已经
  退了（被杀也是退了）；也不是测试等得太短，60 秒等的那一行在扩展被杀的那一刻就没了。
- **改法**：这一条测试的 `grace` 改成 60 秒（别的照 `quick()`）。核心等的是进程退出这件事（`child.wait()`），退了当场往下走，
  平时一点不多等；60 秒只是兜底，扩展真卡住了才用得上。
- **复现**（照 FL-2 的做法，在 Linux 上拖慢）：测试扩展的 `exists:` 那一步前面临时加睡 800 毫秒（只为复现，没提交），改前
  「等不到 3 行：cwd:…/var/lib/xgone」红（同 CI 上的一字不差，60.4 秒）；改后同样拖慢绿（0.85 秒），去掉拖慢照旧绿。
- 同一个文件里别的几条测试等的都是扩展（重新）拉起以后记的行，不靠停下以后再记，不受这个时限影响；别的文件里 `listen` 那种
  记的是收到的推送，也是停下以前记的。

### 没做的和建议

Windows 改后 38 分钟，离 40 只有两分钟上下的余量；再往下靠分流挤不出多少了（对照测试在 Windows 上一共剩一分多钟），要动下面这些。

不是对照的慢测试没有少跑：

1. **`miyu-endpoint` all、`miyu-session` all 在 Windows 上慢 9～12 倍**（336、294 秒）：每个测试都起进程内核心、建临时数据根、开 SQLite。可能的原因是 Windows 上建文件、刷盘（`FlushFileBuffers`）贵，还有 Defender 实时扫描临时目录。没量到是哪一样，不改；建议下一步在 Windows 上照一个测试量一次分布（建目录、写配置、开库、起核心各多久）再定。
2. **脚本到时的测试等 20 秒**（`miyu-basesystem` all `script::a_script_stopped_at_its_timeout_hands_over_nothing`）：脚本的时限只能写整秒，8 秒的在 Windows CI 上被杀过，所以写 20。要快得让时限能写更小的单位，或者测试里注入时钟，动产品代码，拿不准，交给派活的人。它和同程序的别的测试并着跑，ubuntu、macOS 上这个程序就是 22～24 秒。
3. **`fork_subagent::limits` 真跑 500 步**（11 秒）、`memory_extract_failing` 等退避（7 秒）、`miyu-wallpaper` 等文件监视（9 秒）、`read_memory` 量一个大文件的内存（9 秒）、`robust_methods` 每个方法都试（7～10 秒）：都是测的东西本身要这么久，不动。

测试以外的大头（Windows：编测试 13.5 分钟、clippy 两遍 4.3 分钟、文档 1 分钟）这一步没动，建议：

1. **CI 上 debug 只留行号**（`CARGO_PROFILE_DEV_DEBUG=line-tables-only`）：测试程序的调试信息是编测试、链接、占盘的大头，Windows 上生成 PDB 尤其慢；panic 的回溯照样有文件和行号，只是调试器里看不了变量。预计编测试少一两成以上，ubuntu 的盘也少占一大块。要不要拿 CI 上的调试信息换时间，交主人定。
2. **Windows 上关掉 Defender 实时扫描工作目录**（`Set-MpPreference -DisableRealtimeMonitoring $true` 或把工作目录、`~/.cargo`、临时目录加排除）：常见的 CI 提速做法，编译、建文件多的测试都受益。托管机器用完就扔，没有安全上的代价；改的是 CI 环境，交派活的人定。
3. **clippy 两遍**：`--all-targets` 一遍以后，`-p miyu-core --no-default-features` 那一遍还要再编一份不同特性的核心（Windows 59 秒）。可以考虑只在一台上跑这一遍（它查的是特性组合，和平台关系不大）。

### 验收

- 本机 Windows 版 clippy（`cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets -- -D warnings`）过。
- 远程门禁 11 项全过（第一版红在「纯逻辑」：`miyu-assemble` 的测试读了环境变量，改成 `_for_longer` 以后过）。
- 本机 Linux：改了开关的测试平时那份 debug 全过（上表）；全量 release（randomized 那套 profile）全过（上面一段）。
- 偶发那一条：拖慢扩展改前红、改后绿（上一节）。
- CI：`step/CI-2-测试分流-r2` 六项全绿（41932485427）；并进偶发那一条的终版推 `step/CI-2-测试分流-r3`。

### 风险

- 平时少跑的种子覆盖窄了：同一个改动在 check 里只比几十、几百份，坏在少见路子上的要等 randomized 那一项才红。randomized 和 check 在同一趟 CI 里，分支上照样拦得住，只是 Windows、macOS 上只比少的那几份。
- randomized 变长：编的 crate 多了，全量又在这一项。时限照旧 45 分钟，实测见上；以后再加全量，照 `AGENTS.md` 的约定先估 CI 上的时长，挤了就把种子往下调，或者拆一项出去。
