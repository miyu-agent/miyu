## 施工单 RB-1：可靠性修理一批

状态：施工完，待 core1 审（2026-11-16）。来源：LiteMiyu 会话报的后台命令结束被扔（JB-1）、CI run 43589383742 Windows 那格跑满 60 分钟、
门禁报过的偶发红和留下的临时目录、HT-4 漏掉的软件源那一路。一条一个提交。

### 1. 后台命令结束比起它的那次调用的结果先到，被内核扔掉（JB-1）

- **现象**：`run_in_background` 起一条瞬间跑完的命令（`echo`、一起来就报错退出的），日志里 `job.started` 记着、`job.reported` 永远没有；
  她不被叫醒，`jobs` 一直显示在跑，长期目标的续轮一直等（`commands_running`），重开会话才补一条 `aborted`。
- **根因**：工具在调用里同步起命令、交给任务端口，等它的线程当场起；调用交回以后，结果还要过一趟阻塞线程（存 blob）才交回 actor。命令
  一两毫秒就退出的，等的线程存好输出就把结束交回 actor，结束排在结果前面。内核这时还不认得这个编号，照账本「对不上一个还没结束的后台命令
  的，不理」扔掉（`miyu-kernel/src/session/jobs.rs` 的 `job_ended`），任务表照常拿掉它，之后再没人报。监视的命令攒好的行（`job_lines`）、
  到了上限停掉的那一条走同一处。存 blob 要 `fsync`：本机 btrfs 上结束要晚到十几到几十毫秒，盖住了这个窗口；`fsync` 快的盘上（tmpfs、
  ext4、macOS）结束几乎总是先到。
- **复现**：临时的会话测试（没提交）真的 `shell` 后台跑 `echo hi`，等十秒看有没有 `job.reported`。数据根在 btrfs（`target/tmp`）上，
  4 核 8 路各 50 遍、2 核 16 路各 40 遍，一共 1040 遍 0 遍卡住（结束比结果晚 13～97 毫秒）；数据根挪到 tmpfs（`/dev/shm`），
  本机 4 核（app.slice 当时只给 4 个核）8 路各 50 遍，400 遍卡住 3 遍（0.75%：`job.started` 记着、没有 `job.reported`），之前那 10 遍
  里也卡过 1 遍。.91（16 核）上同样 8 路各 50 遍、挤在 2 个核上 16 路各 50 遍，修以前都是 0 遍：那台机器不够忙，窗口盖不住。修以后 .91 上
  这两种压法也都是 0 遍；本机之后不许编译，没在本机复跑，修好这件事由下面钉死时序的单元测试担保。
- **修**（`miyu-session/src/jobs/gate.rs`，[`Gate`]）：照调用扣。actor 派一次调用、给任务端口时记下这次调用还没落盘；端口起的命令记下是
  哪次调用起的；交给 actor 的结束、行、到了上限停掉的，起它的那次调用还没落盘的扣下。actor 每落一批盘都看一遍（`SessionJobs::note`），
  记到一条 `tool.result`：那次调用起的不再扣，扣着的照到的先后交回收件箱，这时内核已经认得它们了。结果里没有 `job.started` 的（工具崩了、
  被掐掉）照样交，内核不认，同以前；调用的结果先落了盘、命令才起来的（被打断还接着跑完的工具）不扣，不然任务表一直不空。不早到的一个字节
  都不变。
- **测试**：`jobs/tests/early.rs`：结束等结果落盘才交、监视的行和停掉的那一条照先后扣着再交、结果先落了盘再起的不扣。改前红：
  把扣的那一处（`Gate::pass`）临时改成一律当场交，`an_end_waits_until_the_call_that_started_it_is_recorded`（红在「扣着，不交」）、`lines_and_a_stop_at_the_limit_wait_too_and_keep_their_order`（十秒内没扣下两封）红，换回来全绿。原来等结束、等行的单元测试起命令以后加一句「那次调用的结果落了盘」，照产品里的先后，断言一条没改。

### 2. Windows 上 `mcp_login::only_a_listed_url_server_without_its_own_token_logs_in` 一直不返回

- **现象**：CI run 43589383742 第一次的 Windows 那格：这一条 11:37 报「跑了 60 秒以上」，之后同程序 1349 条都过了，它一直到 12:18 整个步骤被
  取消都没返回；重跑那一趟全绿。
- **复现**：没复现出来。.91 上（Linux，16 核）原样的测试单独跑 8 路各 40 遍，每遍限 90 秒、超时的抓全部线程的栈：320 遍 0 遍卡住、0 遍失败。
- **推理**：测试里每一处等都带了时限（`Client::call`、`until_listed` 都是 60 秒），卡 40 分钟说明这些时限一个都没到。它们是 tokio 的计时，
  `#[tokio::test]` 是单线程的运行时，只有两种情况会让它们都不到：运行时的那个线程被同步地堵住了（一把锁、一个同步的等；核心的任务都在这个
  线程上跑），或者测试体已经完了、卡在运行时收尾（单线程运行时收尾要等阻塞线程池里的活都做完，那里有一件不完就一直等）。顺这条路查了：
  测试体里同步的几步（`watch_config` 在 Windows 上要等 notify 的监视线程回话，配置、密钥的重读拿着配置的锁），配置的 `watch` 通道的读锁
  有没有跨着配置锁拿（没有），这条路上的 `spawn_blocking` 有没有会一直不完的（没找到），各处 `Drop` 有没有同步地等（没找到）。没有一处
  能单独说明只有 Windows 会卡，根因没找到，不瞎改生产代码。
- **修成有界地失败**（`crates/miyu-endpoint/tests/support/watchdog.rs`，`Watchdog`）：另起一个线程、用真的时钟，连运行时的收尾一起限时
  180 秒（自己建单线程运行时，`block_on` 完了再丢掉它，丢的那一下也在看门的眼里）；测试每到一步记一笔，到了时限在标准错误上说「测试 … 过了
  180 秒还没完，卡在「…」」，整个测试程序 `exit(101)`。下次再卡，这一格几分钟内红、说得出是哪一步，不再白等 60 分钟。这一条测试改成照它跑，
  收尾也拆成几步（放下监视、断开连接、放下核心、删数据根、停假服务、运行时收尾）。
- **自查**：在「等 notes 跑起来」那一步插一个同步的 30 秒睡眠、时限改成 5 秒：跑了 5 秒，标准错误上「测试 only_a_listed_url_server_without_its_own_token_logs_in 过了 5 秒还没完，卡在「等 notes 跑起来」」，退出码 101。还原后照常过。一开始用 `eprintln!` 写，被测试框架接住（连带测试里起的线程），退出时那一句就丢了，改成直接写标准错误。
- **还要做的**：下次 Windows 上再卡，照它说的那一步接着查；到时候能在 Windows 虚拟机上抓栈的再抓。

### 3. `the_total_limit_holds_even_when_deltas_keep_coming` 负载下偶发红

- **根因**：每 10 毫秒来一段、空闲时限 100 毫秒，用的是真的时钟；机器忙时两段之间隔过 100 毫秒，先按空闲超时结束（`Stalled`）。同文件
  `a_quiet_driver_times_out_but_a_trickling_one_does_not` 的后一半同理（每段隔 60 毫秒、空闲时限 300 毫秒）。别的几条空闲时限 5 秒、
  段间隔 0，不受影响；叫停那一条 50 毫秒叫停、空闲时限 5 秒，也不受。
- **修**：这两条改成 `#[tokio::test(start_paused = true)]`：时钟停着，只在都等着的时候往前跳，段间隔、空闲时限、总时限都照假的时钟算，机器
  忙不忙都一样。总时限那一条的断言收紧成「到 1 秒那一刻停，交了 95～105 段」。`miyu-session` 的开发依赖给 tokio 开 `test-util`。
- **改前红**：把总时限临时放到一千倍（等于去掉），`the_total_limit_holds_even_when_deltas_keep_coming` 红（三百段交完、正常结束），换回来绿。

### 4. 测试数据根里留下 `var/cache/mermaid/<哈希>.location`

- **根因**：画图的库记字体位置的目录整个进程只有一个（`set_font_cache_dir`，`third_party/mermaid-rs-renderer`），每次画图前定成这份核心
  的 `cache/mermaid`；位置只在进程头一回读某串字体时记。一个进程里有几份核心（`miyu-core` 的测试）时，一份正在头一回读系统字体库，另一份
  这时把目录定成自己的、读源码失败、测试结束删了数据根；前一份读完字体，位置就记进了后一份的目录，`create_dir_all` 把删掉的数据根又建了
  回来，只剩这一个文件。生产里一个进程一份核心，碰不到。
- **修**（`miyu-mermaid/src/lib.rs`）：一把整个进程共用的锁，一次画图从定目录到画完都拿着，别的家底插不进来改它。生产里一份核心的画图本来
  就一次一张，没有多等。
- **测试**：`crates/miyu-mermaid/tests/font_dir.rs`（自己一个测试程序）：一份一直画读不懂的源码（每次都先定成自己的目录），另一份画一张
  正常的图，位置只记在画图的这一份的目录里。改前红：去掉那把锁，`a_render_remembers_its_fonts_only_in_its_own_directory` 连跑三遍三遍红（位置记进了没画过图的那一份的目录），换回来绿。

### 5. 从软件源装的包的预览不带 `hosted`

- **根因**：HT-4 的 `hosted` 只从清单读，软件源那一路（看一眼、`package.info` 没装的那一份）只有索引条目，索引里没有这一格。
- **修**：`miyu-source` 的索引条目多 `hosted`（驱动名到清单 `[[driver]] hosted` 的原样，没写的驱动不列，一个都没写的不写这一格），
  `collect.rs` 照清单写；端点的看一眼（`source/glance.rs`）、`package.info`（`source/look.rs`）照它带 `hosted`。命令行确认屏照 `drivers`
  的先后、再接 `hosted` 里有 `drivers` 里没有的（软件源那一份没有 `drivers`）；`miyu pkg info` 的「提供」后面一个驱动一行，装着的照
  `contains` 里的。旧索引没有这一格的照旧不印。
- **packages 仓库**：收录程序就是这个仓库的 `miyu-index`（`collect.yml` 照根目录的 `miyu-rev` 装），脚本不用改；这一步合进 main 以后把 `miyu-rev` 换到带它的那一版、重跑一次收录，索引里才有 `hosted`。只报告，没碰那个仓库。
- **网页**：网页的软件包详情要照 `package.info` 的 `hosted` 印那一行，转给 UI 会话。
- **改前红**：收录不写 `hosted`、端点不带、命令行不印，`collect::tests::hosted::an_entry_carries_the_tools_its_drivers_have_the_provider_run`、`package_source_hosted` 两条、`pkg::info::tests::hosted_tools_follow_provides_one_driver_a_line`、`pkg::confirm::tests::an_upgrade_says_what_it_replaces_carries_needs_and_weighs` 红，换回来绿。

### 验收

- 编译、测试都在 .91 上（本机要玩游戏，2026-11-15 主人定）。门禁 `cargo xtask check` 十一项全过（.91 上一趟；变基到 main 的 118cb7aed 以后本机又一趟）。
- 第一趟门禁的「测试」红在 `config_watch_log::a_hand_edit_and_an_unavailable_watch_are_logged`（六十秒内没等到配置的推送）：这一步没碰
  配置监视，同一台机器上单独连跑 20 遍 20 遍过，是那一趟 .91 上几个门禁挤在一起时的抖动；记一笔，没改。
- `cargo test -p miyu-kernel random:: -- --include-ignored` 过（3 条，293 秒）；`cargo xtask prompts`、`cargo xtask logs` 干净。
- Windows 版 clippy（`cargo clippy --target x86_64-pc-windows-gnu --workspace --all-targets -- -D warnings`）：.91 没装 mingw 跑不起来，
  本机能编译以后在本机跑，过。
