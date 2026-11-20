## 施工单 AU-P1：外部审计中低危三十三条核实

状态：做下来了，待验收。接 AU-P0：项目主人 2026-11-18 转来的外部审计报告（gemini 写的，不在仓库里，不一定属实），第一章 1.2「中低危代码缺陷」、1.3「损坏、无用与不必要的部分」里核心仓库的 #015–#049（#019、#037 已经做完，不在这里），逐条在 main 上复现或照代码核对，结论分成立、部分成立、不成立；成立的照「改前红」修，不成立的不改代码、写清理由。编号照审计号写成 AU-015 这样。

删除类的（死代码、没用的依赖、测试用的小程序这些）和假实现的配置项只核实、列出处理建议，不动手删、不改界面和配置，交项目主人拍板；文档类的（#041）、本机检出里没进仓库的脚本（#039）、打包的样例（#038）只核实。

### 结论（一条一行）

| 编号 | 报告说的 | 结论 | 做了什么 |
|---|---|---|---|
| AU-015 | Linux seccomp 只挡 `socket(AF_UNIX)`，`io_uring` 绕得过；别的架构没有编译期保护 | 部分成立：io_uring 成立；别的架构本来就编不过 | 过滤多挡 `io_uring_setup`（这个架构的、32 位的） |
| AU-016 | `/tmp/miyu-call-<进程号>` 照「已经有了」算，不看属主、跟链接 | 部分成立：别人的目录收紧不了、照旧报错（只是用不了通道）；链接成立 | 已经在的不跟链接、不是自己的目录不用 |
| AU-017 | `miyu call` 照长度头一下子要 4 GiB，abort | 成立 | 照来的长度边读边长 |
| AU-018 | 回调一条一条读，浏览器的预连接卡住真正的回调 | 成立 | 每条连接各读各的，一条最多读 10 秒 |
| AU-020 | SSE 开头的 BOM 丢第一条；同一个 `index` 不同 `id` 拼坏参数；流里的限速当可重试 | 部分成立：前两条成立，第三条是蓝图定的 | 去掉开头的 BOM；同一个 `index` 来了新 `id` 是新的一次 |
| AU-021 | 折叠块里不是围栏的反引号行没中和 `</details>`；`CON.tar.gz` 躲过设备名 | 成立 | 照这一行真开、真合认围栏；照第一个点以前认设备名 |
| AU-022 | 壁纸清单换成真实路径以后链接改指别处盯不到；空的 `XDG_CONFIG_HOME` 当相对路径 | 成立 | 清单原样留着、每次照它重算；空的当没设 |
| AU-023 | `Timestamp::from_local` 年份大得离谱时乘法溢出 | 部分成立：唯一的调用方只给四位数的年份 | 年份先挡 |
| AU-024 | 压缩时的文件清单照现在的工作目录写相对路径，Windows 分隔符对不上 | 不成立 | 不改 |
| AU-025 | 一段都没有的会话导得进来；每一行不照事件的写法核 | 部分成立：前一条成立，后一条是设计 58 定的 | 一段都没有的整个拒；后一条交项目主人 |
| AU-026 | 删人格漏删回合库，记忆日志的缓存不清 | 不成立 | 不改 |
| AU-027 | 取汇率的端口没叫 `done` 就丢了，那个币种一直挂在「在取」里 | 成立 | 跟着 `done` 走的一样东西丢下时拿掉 |
| AU-028 | 按天分组朝 0 截，1970 年以前的和 1970-01-01 并成一天 | 部分成立：只在钟拨到 1970 年以前的机器上 | 照往下取整算 |
| AU-029 | `replaces` 写空串、改的是别的类时记得下没写是谁的认识的人 | 成立 | 工具先认「没传」再查；`Keeper::save` 照样查，拒成 `NoWho` |
| AU-030 | 包住数据根的自己的几处把密钥算成能写；遮密钥照字典序，长的留下后半截 | 部分成立：前一条要人自己把那几处换成包住数据根的链接；后一条成立 | 自己的几处同工作区，本身在数据根里才算；长的密钥先换 |
| AU-031 | 后台写盘跑在放锁以后；空闲时限热改不重算看的间隔；整理目录的临时文件名撞 | 部分成立：第一条不成立（TMP-1 补2 已改），第二条最多晚 15 秒不改，第三条成立 | 临时文件名带进程内的计数 |
| AU-032 | 回顾最多 0 轮时整场会话全取 | 部分成立：快照里的数是写死的 8 | 0 轮的什么都不取 |
| AU-033 | 语义模型小程序杀了不等它退 | 成立 | 杀了也等它退完 |
| AU-034 | `miyu mcp login` 拉起的打开网址的程序不收，留僵尸 | 成立 | 另一个线程等它退 |
| AU-035 | `job.output` 带 `after` 的字比 128 KiB 多到三倍 | 部分成立：和蓝图的字面对不上，一行 1 MiB 照样放得下 | 不改代码，蓝图写明照输出文件的字节算 |
| AU-036 | 四个反引号的块被三个合上；`İ` 让文件名打分错位；`glob("~")` 落回工作目录；`packages` 参数写错列全表 | 部分成立：第 3 条结果是「找不到」，不越权 | 第 1、2、4 条修；第 3 条不改 |
| AU-038 | `miyu-web.service` 的程序路径哪种装法都不在；OpenRC 的开头写死 `openrc-run` | 部分成立：前一条样例和 README 都写明要换；后一条是 OpenRC 的规矩 | 不改，前一条交项目主人 |
| AU-039 | `.worktrees/try-onebot.sh` 三重损坏 | 不成立（不在仓库里）：本机检出里没进版本库的旧脚本 | 不改，进可删清单 |
| AU-040 | `miyu-heap` 漏了 `undocumented_unsafe_blocks` | 部分成立：八处 `unsafe` 都写了 `SAFETY` | 不改，交项目主人 |
| AU-041 | 快速上手是占位稿；`DOC-2` 有断链；`FH-盘点` 和代码对不上；新版 clippy 三处告警 | 部分成立：快速上手项目主人已经写好（不成立）；`DOC-2` 那一处在代码格里、不是链接；盘点表那一格写错了；工具链钉在 1.97 | 不改（文档类只核实） |
| AU-042 | 模型的 `tools` 开关、`inputs` 的 `audio`、`video` 是假的 | 部分成立：都是只声明、不改请求，项目主人 10-07 定的 | 不改，交项目主人 |
| AU-043 | `Sandbox.workplace` 只写不读 | 不成立（现在）：AU-002 起关起来的会话读它 | 不改 |
| AU-044 | `miyu-core` 的正式依赖里有没用的 `miyu-drivers` | 成立 | 不删，进可删清单 |
| AU-045 | `DataRoot::imported_file` 没人调 | 成立 | 不删，进可删清单 |
| AU-046 | `Sampling.task` 标着 `allow(dead_code)`、丢了不停 | 不成立：没有那个标，测试拿它停，采样照核心在不在自己停 | 不改 |
| AU-047 | 私有模块里的 `pub const` 用不到；只有测试用的公开函数编进正式库 | 部分成立：常量在 crate 里用着；那几个函数只有集成测试用 | 不删，进可删清单 |
| AU-048 | 画图库带着 1751 行没用的命令行和 `layout_dump` | 部分成立：命令行在 `cli` 特性后面、不编；`layout_dump` 编进来没人用 | 不删，进可删清单 |
| AU-049 | 测试用的 `miyu-test-embed` 正式构建也编 | 部分成立：发版只编 `miyu`、`miyu-sandbox`、`miyu-extract`，不编它；`--workspace` 才编 | 不改，进可删清单 |

### 逐条

（见下面各条；验收里列了每条改前红的测试。）

**AU-015**：`seccomp.rs` 的过滤只看系统调用号：`socket(AF_UNIX)`、32 位的 `socketcall`、x32、认不出的架构回 `EACCES`，别的放行。io_uring 的 `IORING_OP_SOCKET`（5.19 起）在内核里建套接字，不经 `socket` 系统调用，seccomp 看不到；走 seccomp 的那一路（Landlock 管不住连 Unix 套接字的内核）关起来的会话能这样建 AF_UNIX 套接字、连路径上的（Landlock 第 9 版以前不管连）。修：`io_uring_setup`（各架构都是 425）这个架构的、32 位的都回 `EACCES`，同 Docker 的默认规则；没有 io_uring 的建不了 ring，`io_uring_enter`、`io_uring_register` 不用另挡。Landlock 管得住的那一路不装 seccomp，io_uring 的连接照样经 LSM 的钩子被 Landlock 挡。别的架构的 `compile_error!`：`NATIVE`、`COMPAT` 只为 x86_64、aarch64 定义，别的架构编到这里就报找不到，编不出来，不会悄悄放行。

**AU-016**：临时目录（`std::env::temp_dir()`，没设 `TMPDIR` 就是 `/tmp`）几个用户共用，`miyu-call-<进程号>` 猜得到。原来照 `recursive(true)` 建，叫这个名字的已经在了（哪怕是链接、别人的目录）也算成；接着 `set_permissions` 跟着链接走。别人的目录收紧不了（`EPERM`），照旧交回出错，结果是这一回没有通道，命令照跑（DoS）；预先放一个指到本人某个目录的链接，那个目录被改成 `0700`、往里放套接字和 `bin/`。修：不递归，已经在的照 `symlink_metadata` 看：是目录、属主是自己的照用（收紧成 `0700`），链接、别人的、不是目录的交回出错。

**AU-017**：`exchange` 读到长度头直接 `vec![0u8; len]`，`0xFFFFFFFF` 要 4 GiB。机器上内存够、允许超额分配的，`calloc` 拿得到、只是白占；限了地址空间的（报告用 `ulimit -v 2097152`）分配失败、整个进程 abort（退出码 134），不是说好的退出码 3。改：`take(len).read_to_end`，照来的边读边长，没读够的照「所属的命令结束了」。

**AU-018**：`Listener::next` 一条连接读完（不限时）才接下一条。Chrome 这类浏览器开页面前常多开一条空着的连接（预连接），真正的回调走另一条；先接下的是空着的那条时，一直卡到登录的时限（10 分钟）。改：每条连接各起一个任务读请求那一行，最多 10 秒、同时最多 16 条，先读出回调的那条交回，其余的丢掉（断开）。

**AU-020**：
1. 成立：WHATWG 的 SSE 规矩里流开头的一个 U+FEFF 要去掉；原来它粘在第一行字段名前面，`\u{feff}data` 不认，第一条丢了。修：第一行去掉一个开头的 BOM，后面的照原样。
2. 成立：`slot` 照 `index` 找到了就用，同一个 `index` 来了不一样的 `id` 也接到那一次上。有的兼容层并行的几次调用都写 `index: 0`，参数接成 `{"p":"1"}{"p":"2"}`、后一次丢了。修：同一个 `index` 来了个和那一次不一样的 `id` 是新的一次；同一个 `index` 的照最近的那一次认（不带 `id` 的后续片接到最近的那一次）。每次调用 `index` 各不相同的照旧。
3. 不成立：流里报的错没有 HTTP 状态，限速的说法照可重试算，是蓝图定的（`drivers/openai-chat.md`「出错分类」第 3 条「有状态、是 4xx、有限速的说法」，`drivers/openai-responses.md`「定了什么」那一行「流里的错没有状态，限速的说法也照可重试算」）。不改。

**AU-021**：
1. 成立：`prose` 只要这一行长得像围栏（开头三个反引号）就不中和，`fence_of` 认得出、但它后面的字里又有反引号，并不开代码块；这一行里的 `</details>` 原样写出去，折叠块提前合上。修：照这一行真开、真合了代码块判，没开没合的照普通的一行中和。
2. 成立：`clean` 照最后一个点拆，`CON.tar.gz` 拆成 `CON.tar` 和 `.gz`，`CON.tar` 不是设备名；Windows 照第一个点以前认。修：设备名照第一个点以前那一截（去掉末尾空格）认。

**AU-022**：
1. 成立：`start_native`、`Watch::set` 一开始就把清单换成真实路径，原样的那一份丢了；每次有动静重算真实路径时照的是换过的那一份，链接改指别处以后算出来的还是旧的那一张，新的那一张怎么改都不叫。修：清单原样留着（`wanted`），每次照它重算。链接本身改指向（盯不到它所在的目录：家目录、`~/.config` 里别的程序一直在写，设计上不盯）照旧靠下一次有动静、拿回焦点时再找。
2. 成立：`unix.rs` 的 `config_dir` 没去掉空的 `XDG_CONFIG_HOME`（`watch.rs` 去掉了），设成空的时配置目录是相对的空路径。修：空的当没设。

**AU-023**：`from_local` 先查月、日、钟点，再拿年份去乘；年份 3 亿时 `days * 86_400_000` 溢出，debug 下 panic，release 下绕成别的时刻。文档注释说出了 0000 到 9999 年的没有。唯一的调用方（`history` 读 `since`、`until`）只认四位数的年份，走不到。修：年份先挡。

**AU-024**：不成立。`shown` 只在压缩那一刻算一次，写进 `context.compacted` 的 `notes`、照日志原样回放，不是每次请求重算；清单给的是她压缩以后接着干活的工作目录里怎么找这些文件，照现在的工作目录写相对路径才对（照当时的写，换了目录以后照着读就读错了地方）。Windows 上分隔符、大小写对不上的写绝对路径，信息照样对，只是长一点。不改。

**AU-025**：
1. 成立：清单里一段都没有（`segments: []`、没写 `upto`）的会话照样建目录、算进 `added`，开日志就是「没有这个会话」。修：一段都没有的整个拒（`Mismatch`）。
2. 设计如此：设计 58 第二节第 3 条、蓝图 `store.md`「导入」第 1 条定的是每一行是一整行 JSON、序号连续，不照事件的写法核；测试的样本也是 `{"seq":…,"type":"note"}` 这种只有序号的行。要不要改成照 `Event::from_line` 核（认不得的种类照样收）交项目主人，见下。

**AU-026**：不成立。
1. 回合库 `turns-<人格>.db` 是照会话日志派生的，删人格不删会话，那些会话还在、还是这个人格的；删了库，下次开会话照日志补回来（`memory.md`「怎么走」第一条第 9 款）。设计 55 第三节第 6 条只要记忆日志跟着进回收处、记忆库删掉，回合库不在里面。
2. `MemoryLogs` 有 `forget`，删人格的端点（`miyu-endpoint/src/personas/delete.rs`）挪记忆之前先 `logs.forget(…)`；报告的复现直接调 `take_memory`，跳过了这一步。

**AU-027**：成立。`rates` 把缺口放进 `flying`，只在 `done` 被叫时拿掉；端口的任务 panic、核心停下时任务被取消，`done` 没叫就丢了，那个缺口一直在 `flying` 里：往后每一次查都等满时限（2 秒），又因为「有人在取」不再交给端口。修：跟着 `done` 走一样东西（`Landing`），它丢下时（叫过了、没叫就丢了都算）拿掉缺口、叫醒等着的。

**AU-028**：部分成立。SQLite 的整数除法朝 0 截，`(at + 时区) / 86400000` 落在 `(-86400000, 0)` 的商是 0，和 1970-01-01 并成一天。记用量的时刻是发请求的那一刻，只有钟拨到 1970 年以前的机器上才有。照付钱的那一项（`daily`，`at / 86400000`）也一样。修：两处都照往下取整算。

**AU-029**：成立。`remember` 先用原样的 `replaces` 判要不要写是谁，再把空串、`null` 当没传：`"replaces": ""` 躲过了检查，记下没有 `who` 的 `person`。`replaces` 指着别的类（没有 `who` 可带）的同样。修：工具先认「没传」再查；`Keeper::save` 算完要带的 `who` 再查一遍，记成认识的人又不知道是谁的拒成 `Refused::NoWho`（工具照「认识的人要写是谁」那一句说，协议照 `bad_params`）。

**AU-030**：
1. 部分成立：`own` 是数据根里照 XDG 的那几处（`pictures` 这些），本来就在数据根里。`zone` 判数据根时只看路径在不在某一处自己的里面，不看那一处本身在不在数据根里（工作区看）：人把 `pictures` 换成指到家目录的链接，真实位置包住数据根，数据根里的密钥、令牌都算能写。她自己改不了这几处本身（文件工具不建链接，沙盒的写名单不放数据根），要人自己这样摆。修：同工作区，自己的那一处本身在数据根里，数据根里的那一块才照它算。
2. 成立：`redact` 照给的先后一个个换，短的是长的开头时先换短的，长的剩下后半截原样留着。修：长的先换。

**AU-031**：
1. 不成立：报告看的是旧的写法。施工 TMP-1 补2 起 `serve_then` 在交回之前等收尾（`pending::closing`：清回收处、清缓存、等另起线程写盘的写完），锁跟着 `running` 里的监听器，交回时才放（`miyu-core/src/serve.rs`，守着它的是 `tests/serve.rs` 的 `the_closing_work_is_done_before_the_lock_goes`）。
2. 部分成立、不改：看空不空闲的间隔起来时定一次（时限的四分之一，最少 100 毫秒、最多 30 秒）。配置里的时限按分钟写，热改以后间隔最多从 30 秒变成 15 秒，退出最多晚 15 秒；现有两条测试（`a_core_without_an_idle_limit_keeps_running`、`a_core_whose_data_root_is_gone_stops_by_itself`）就靠这个定死的间隔，改了要连它们一起改，不值。
3. 成立：整理好的目录先写到 `<文件>.<进程号>.tmp` 再换过去，同一个进程里两次刷目录的写盘线程同时在写时往同一个文件里交替写、再把写了一半的换过去。修：名字再带一个进程内的计数。

**AU-032**：部分成立。`exchanges_of` 先数了一轮再比 `answered == turns`，0 轮的永远不等于，整场会话全取。快照里的回顾的数是写死的出厂值（8 轮，`RECAP`），配置改不了，走不到。修：0 轮的什么都不取。

**AU-033**：成立。`Worker::stop` 等满 5 秒没退的只发了杀的信号就返回，进程还在（Linux 上至少是没收走的僵尸），`Embedder::shut` 说好退完才返回（Windows 上紧接着删模型文件）。修：杀了以后也等它退完，同扩展的 `finish`。

**AU-034**：成立。`opened` 起了打开网址的程序（`xdg-open`、`open`、`rundll32`）就丢下 `Child`；Unix 上它马上退，等回调的十分钟里一直是僵尸。修：另一个线程等它退。

**AU-035**：部分成立、不改代码。带 `after` 的一次照输出文件读最多 128 KiB，解不开的字节换成 `�`（3 个字节），交出去的字最多是三倍。蓝图 `protocol.md` 的 `job.output` 第 3 条写「一次最多交 128 KiB（照 UTF-8 算）」，那是给不带 `after`、照字截尾的那一种的；那一条定这个数是为了「一行最长 1 MiB」放得下：一个字节最多写成 6 个（控制字符），`�` 写进 JSON 不转义，带 `after` 的最坏也不到 768 KiB，放得下。`size` 照输出文件的字节记，头照它接着读，不能改成照交出去的字算。蓝图第 9 条写明 128 KiB 照输出文件的字节算。

**AU-036**：
1. 成立：`chunk::markdown` 见到开头是三个反引号的就翻一次，四个反引号开的块里写着三个反引号的被当成合上，里面的 `#` 当成标题，标题路径错了。修：照围栏的字和长短认，同一种字、不比开的短才合上。
2. 成立：`score` 整串小写以后按字数比，文件名从第几个字起照原串算；`İ` 小写成两个字，后面错开一格，文件名的首字丢了 100 分，交回的标的位置也对不上原串。修：一个字一个字地小写，只取头一个，字数不变。
3. 部分成立、不改：`glob(pattern="~")` 照「最后一段是要找的名字」拆出空的目录，落回工作目录找名叫 `~` 的，结果是找不到；不越权（找的是工作目录），没有通配的模式本来也只找那一个名字。
4. 成立：`packages` 的参数读不成（`{"package": 3}`、不是对象的）当成什么都没写，把整张表列给她。修：照别的工具说「参数不对」，端口不问。`package` 读不成的照旧说「写一个」，不会做错事，不改。

**AU-038**：部分成立、不改。`miyu-web.service` 的 `ExecStart=/usr/bin/miyu-web serve` 确实没有哪种装法放在那里，但样例开头的注释、`packaging/README.md`「服务器」那一节都写明「装之前把 `ExecStart` 换成它的真实位置」，并给了 AUR、安装脚本的两个位置。`#!/sbin/openrc-run` 是 OpenRC 服务脚本的规矩（只装在 OpenRC 的机器上），不成立。要不要把样例直接写成 AUR 的位置交项目主人。

**AU-039**：不成立（不在仓库里）。`.worktrees/try-onebot.sh` 在主检出的 `.worktrees/` 下，`.gitignore` 第 2 行忽略整个 `.worktrees/`，版本库里没有它；是旧仓库时期留在本机的调试脚本，确实三处都过时了。进可删清单，删不删由项目主人定。

**AU-040**：部分成立、不改。`miyu-heap/Cargo.toml` 照抄工作区的 lints、注释说「同 `miyu-sandbox`」，`miyu-sandbox`、`miyu-pipe`、`miyu-cli` 有 `undocumented_unsafe_blocks = "deny"`，它没有。现在八处 `unsafe` 块都写了 `// SAFETY:`，加上不会红。交项目主人（推荐加）。

**AU-041**：部分成立，文档类只核实不改。
1. 快速上手：报告看的时候是占位（「由项目主人撰写，尚未放进来」），现在 main 上项目主人已经写好（DOC-15 起在 `docs/zh/guide/quickstart.md`），不成立。
2. `DOC-2-英文版.md` 第 10 行那一段整个在反引号里（代码格），不是链接，不成立。
3. `FH-盘点-核心.md` 第 89 号那一行写 `bundle/import.rs` 用 `DataRoot::imported_file`：成立，`import.rs` 直接照 `MARKER` 拼路径（文件名是 `imported.json`，报告说的 `".imported"` 不对），`imported_file` 没人调（见 AU-045）。
4. 新版 clippy 的 `chunks_exact_to_as_chunks`：不成立，工具链钉在 1.97.0（`rust-toolchain.toml`），门禁的 clippy 不报；升级工具链是单独的一步。

**AU-042**：部分成立、不改。`inputs` 里的 `audio`、`video` 是施工 8-27 照项目主人 2026-10-07 定的「只是声明能收，驱动照旧不发」（`settings.rs` 的注释）；`tools` 给 `model.list` 显示、给首次引导挑推荐的模型（`onboard/recommend.rs`），不改请求的工具面。说它们是假的不对，说它们关了不影响请求是对的：界面上的说明要不要写明交项目主人。

**AU-043**：不成立（现在）。报告看的是 AU-002 以前：那时关起来的会话确实不读。AU-002（已在 main）起 `sandbox/confined.rs` 第 162、177 行读它，宽的工作目录退回默认工作区。

**AU-044**：成立。`miyu-core` 的 `src/` 只有 `models/tests.rs` 用 `miyu_drivers`，开发依赖里已经有了。删掉正式依赖那一行不影响程序（`miyu-session`、`miyu-http` 照样把它带进来）。进可删清单。

**AU-045**：成立。`DataRoot::imported_file` 全仓库没人调，导入照 `MARKER` 直接拼（`bundle/import.rs`），读标记的几处同样。进可删清单（或者让那几处改调它）。

**AU-046**：不成立。现在的 `Sampling.task` 是公开的字段、没有 `allow(dead_code)`，测试（`tests/memlog.rs`）拿它 `abort`；采样的循环拿的是 `Weak<Core>`，核心没了就停，不会一直跑。正式代码不读它，留着不碍事。

**AU-047**：部分成立。`NESTED_PER_PARENT`、`NESTED_PER_SESSION` 在 crate 里用着（出厂值、没写时的默认），只是没导出，不是死代码。`builtin_tools`、`as_released`、`lifeline::unix::by_helper`、`human::with_dir`、`personas::at_home` 只有集成测试（`tests/`）用：集成测试只看得到公开的，不能写成 `#[cfg(test)]`，要挪就是放进 `testkit` 特性。进可删清单。

**AU-048**：部分成立。`cli.rs`（1586 行）在 `#[cfg(feature = "cli")]` 后面，`miyu-mermaid` 用 `default-features = false`，不编；`layout_dump.rs`（159 行）是无条件的 `pub mod`、没人用，LTO 会去掉。原样拷来的第三方库，删了和上游的差别变大，升级时多一处要挪。进可删清单（推荐不删）。

**AU-049**：部分成立。`miyu-test-embed` 是 `miyu-session` 的一个 `[[bin]]`，`cargo build --workspace` 会编它；发版（`xtask dist`）只编 `-p miyu -p miyu-sandbox -p miyu-extract`，不编它，也不打进包里。`miyu-endpoint` 的 `miyu-test-extension`、`miyu-test-mcp` 同样的写法：别的 crate 的集成测试照「和测试程序同一层」找它，加 `required-features` 要连那几处一起看。进可删清单（推荐不动）。

### 蓝图改了哪几节

- `sandbox/linux.md`「怎么走」第二条第 6 款、「守着它的」seccomp 那一行。
- `tools/shell.md`「miyu call」第 2 条：放套接字的目录已经在的不跟链接、不是自己的不用。
- `cli/call.md` 第 4 条：长度头照来的边读边长。
- `mcp.md`「怎么走」第八条第 3 条：回调每条连接各读各的。
- `drivers/openai-chat.md`「SSE 分帧」、「工具调用」第 1 条。
- `view.md`「Markdown」第 2、3 条。
- `fs.md` 边界表；`store.md`「导入」第 1 条；`protocol.md` `job.output` 第 9 条；`tools/packages.md`「参数」；`recall.md` 第四条；`knowledge.md`「切块」第 1 条。

### 不做什么

- 删除类（AU-039、AU-044、AU-045、AU-047、AU-048、AU-049）、假实现的配置项（AU-042）、`miyu-heap` 的 lint（AU-040）、打包样例（AU-038）、文档（AU-041）只核实，交项目主人。
- AU-025 第 2 条（照事件的写法核每一行）是设计 58 定的，交项目主人。
- AU-031 第 2 条、AU-035、AU-036 第 3 条部分成立但不改，理由见上。

### 验收

1. 每条修了的都有改前红的测试（本机先在没修的代码上跑过，红的原因对得上）：`miyu-sandbox` 的 `io_uring_cannot_be_set_up_and_the_old_answers_stay`、`a_confined_command_under_seccomp_cannot_set_up_io_uring`，`miyu-call` 的 `a_private_directory_is_never_taken_over_through_a_link`，`miyu-cli` 的 `a_garbage_reply_length_is_not_allocated_up_front`、`the_opener_is_reaped_once_it_exits`，`miyu-mcp` 的 `an_idle_preconnect_does_not_hold_up_the_real_callback`，`miyu-drivers` 的 `a_leading_bom_is_dropped_once`、`a_new_id_on_the_same_index_is_a_new_call`，`miyu-view` 的 `a_backtick_line_that_is_not_a_fence_is_still_escaped`、`names_are_safe_on_every_platform`，`miyu-wallpaper` 的 `an_empty_xdg_config_home_means_the_default`、`a_linked_target_follows_where_the_link_points_now`，`miyu-kernel` 的 `a_local_date_and_time_turns_back_into_the_moment`，`miyu-store` 的 `broken_bundles_leave_nothing_behind`、`a_fetch_dropped_without_an_answer_is_asked_again_next_time`、`a_moment_before_1970_is_on_its_own_day`，`miyu-session` 的 `a_person_replacing_a_memory_without_who_is_refused`、`a_worker_that_will_not_exit_is_killed_and_reaped`，`miyu-memory` 的 `a_person_says_who`，`miyu-fs` 的 `an_own_dir_wrapping_the_data_root_does_not_open_it`、`query_characters_must_all_appear_in_order_name_hits_segment_starts_and_runs_score_higher`，`miyu-recall` 的 `a_longer_secret_is_replaced_before_a_shorter_one_inside_it`、`a_longer_fence_is_not_closed_by_a_shorter_one`，`miyu-assemble` 的 `zero_turns_take_nothing`，`miyu-basesystem` 的 `bad_arguments_are_said_not_listed`，`miyu-core` 的 `two_writes_in_one_process_use_different_temporary_files`。
2. `cargo xtask check` 全过、Windows 版 clippy、三平台 CI。
