## `shell`

### 是什么

在这一轮的工作目录里用这台机器的 shell 执行一条命令，跑完交回输出和退出码。前台的到时、叫停时整组杀掉；写了 `run_in_background` 的放到后台（施工 7-3）：照前台一样起，交给执行器的任务表，当场交回编号，结束了由任务表记 `job.reported`。每次调用起一个新的 shell，`cd`、变量都不带到下一次。命令拿到核心的环境变量，名字像密钥的、`MIYU_*` 不给，沙盒里另不给桌面的（4-8 补）。不报改了哪些文件：内核看不懂，撤销不了；后台的只报派出去了（`job.started`）。

### 在哪

| 代码 | 管什么 |
|---|---|
| `crates/miyu-basesystem/src/shell.rs` | 参数、超时、结果那一句 |
| `crates/miyu-basesystem/src/shell/program.rs` | 用哪个 shell、怎么起 |
| `crates/miyu-basesystem/src/shell/env.rs` | 命令拿得到哪些环境变量 |
| `crates/miyu-basesystem/src/shell/process.rs` | 起命令、读输出、等它结束、整组杀 |
| `crates/miyu-basesystem/src/shell/process/waits.rs` | 等命令结束（施工 M-7 上挪出来）：进程结束和「在等人」「等完了」进同一根通道，等人的那几段不算时限 |
| `crates/miyu-basesystem/src/shell/hazard.rs` | 照命令的字面认高危，报给权限策略（施工 5-17；施工 HZ-1 从 `shell.rs` 挪出来）：下面「高危」 |
| `crates/miyu-basesystem/src/shell/person.rs` | 给人跑一条（施工 SH-1 上，`interface.md`「七、给人跑一条」）：只给人用，下面「给人跑」 |
| `crates/miyu-basesystem/src/shell/background.rs` | 后台命令：起好交出去的输出和进程（`miyu_tool::Background`），等它、整组杀（施工 7-3） |
| `crates/miyu-basesystem/src/shell/channel.rs` | 脚本调工具的通道这一头（施工 M-3）：开端点（认这条命令，`miyu_call::Kin::command`）、造命令（`prepare`），环境变量、沙盒的规格照端点加上 |
| `crates/miyu-call/src/lib.rs`、`serve.rs`、`kin.rs`、`unix.rs`、`windows.rs`、`reply.rs`、`section.rs`、`other.rs`、`leftovers.rs` | 通道本身（施工 M-3 起在基础系统，XG-1 下挪成共用的一层，扩展声明了 `runs_commands` 的工具也用，`providers.md`「会跑命令的」）：两边共用的一问一答、令牌、环境变量、交给扩展的那一格（`serve.rs`），认人两种（`kin.rs`：这条命令的进程组、作业对象；扩展进程的子孙），Unix 的套接字，Windows 的命名管道（施工 M-3 下），结局写成退出码和字，说明末尾那一段；别的系统开不成 |
| `crates/miyu-basesystem/src/shell/touched.rs` | 前台命令改了她看过的文件：起来之前记修改时刻和大小，跑完变了的重读，交 `file.read`（施工 E-D 补） |
| `crates/miyu-basesystem/src/shell/output.rs` | 输出：边读边解成字、内存里只留头尾、截成头尾两段；后台的边读边换行尾（`Crlf`） |
| `crates/miyu-basesystem/src/load.rs` | 说明是一段模板，核心起来时换进 shell 的名字 |
| `resources/software/basesystem/tools/shell.json` | 说明和参数格式 |
| `resources/software/basesystem/shell/*.txt`、`common/bad-args.txt` | 输出里给她看的几句 |
| `resources/core/call/*.txt` | `miyu call` 退出码 1 到 4 的说法、`--schema` 印的样子（施工 M-3）、说明末尾那一段（施工 M-4）；命令行那头说的几句也在这里。施工 XG-1 下从 `software/basesystem/shell/call/` 挪成核心的字，字节不变 |
| `resources/software/basesystem/human/{zh,en}.json` | 显示名、结果那一句 |

### 对外的样子

访问类别 `execute`。说明和参数的原文如下。

样本 `resources/software/basesystem/tools/shell.json`：

```json
{
  "description": "Execute a command with {shell} and return its output. Every call starts in the working directory, so cd does not carry over to the next call.",
  "parameters": {"type":"object","properties":{"command":{"type":"string"},"description":{"type":"string","description":"Short title, a few words."},"timeout":{"type":"integer","description":"Milliseconds, up to 600000. Default 120000."},"run_in_background":{"type":"boolean","description":"No timeout. Returns a job id at once."},"watch":{"type":"boolean","description":"Implies run_in_background. Each stdout line wakes you."}},"required":["command","description"]}
}
```

- 说明是一段模板：核心起来时找一次用哪个 shell，把 `{shell}` 换成它的名字（`bash`、`zsh`、`PowerShell 7`、`Windows PowerShell 5.1`），核心跑着时不变；造会话时连同工具面存进快照。例如 Linux 上：`Execute a command with bash and return its output. Every call starts in the working directory, so cd does not carry over to the next call.`
- 别的工具的说明不当模板读，里面的花括号是字面的。

| 参数 | 必填 | 怎么认 |
|---|---|---|
| `command` | 是 | 要执行的命令 |
| `description` | 是 | 这条命令在做什么的短标题，几个词。前台的不用它跑命令，记在调用里，前端显示用（施工 4-13，2026-09-28 项目主人定；前端随 M8）；后台的是任务的标题，记进 `job.started`（施工 7-3）。没写的，参数不对 |
| `timeout` | 否 | 毫秒。没给、给了 0，是 120000；大过 600000 的照 600000。要是不小于 0 的整数，负数参数不对。后台的不看它，照样查写法 |
| `run_in_background` | 否 | 布尔，没写是假。真的放到后台（施工 7-3，下面「后台」）。写成字符串的（`"true"`）内核照参数格式修正成布尔。预设关了「后台运行」的会话工具面上没有这一项（施工 T-1 上，`presets.md`「怎么走」；施工 BS-1 上起照 `Tool::background_parameters` 去掉，不看名字） |
| `watch` | 否 | 布尔，没写是假。真的：监视，照放到后台走（和 `run_in_background` 一起写、只写它都一样），标准输出每出一批行记一条 `job.lines`、叫醒她（施工 MN-1 上，设计 56，下面「后台」第 9 条）。说明一句 `Implies run_in_background. Each stdout line wakes you.`。和 `run_in_background` 一样只在有后台时给，预设关了「后台运行」的会话工具面上没有 |

- 别的参数不认，也不报错。施工 D-4 加过一格 `outside_sandbox`（这一次在沙盒外跑、先问人），随「工作区」一级去掉了（`11-权限与沙盒.md` A16，施工 LV-2）：旧会话里带着它的调用照常跑。
- 不报要碰的路径。权限策略照访问类别判：完全放开放行、不进沙盒；只读时沙盒能用就放行、在只读沙盒里跑，用不了的问人（施工 5-4 上，`session/guard.md`；A16 起只剩这两级，施工 LV-1）。
- 问人时交给说明的几格（`Tool::asking`，施工 D-4）：`title` 是 `description`，`command` 原样。

### 怎么走

1. 读参数，读不懂的（没写 `command`、类型不对）：参数不对。
2. `run_in_background` 或者 `watch` 是真的：照下面「后台」走，不定超时。
3. 定超时（上面的表）。
   - 调用带了沙盒的（`Call.sandbox`，施工 5-1）：命令写成 `<助手> run --spec <规格的 JSON> -- <shell> <shell 的参数…>`，别的都照下面走：Unix 上助手换成了 shell，是同一个进程；Windows 上助手起子进程、等它（`sandbox.md`）。规格写不成 JSON 的（里面有不是 UTF-8 的路径）：照下面「起不来」说，不会不经沙盒就跑。沙盒带的环境变量（`Sandboxed.env`，例如 `TMPDIR`）照挑好的之后设上，同名的盖掉（施工 5-4 上）。Linux 上只用 Landlock、要认缓存规则集指纹的（`miyu_sandbox::checks_files`，`sandbox/linux.md`「核心按规格缓存规则集」），整条命令在阻塞线程里造，造好了拿回来起（施工 V-50），前台、后台一样；别的当场造。
4. 前台干等的不跑（2026-10-26 项目主人定，`shell/waiting.rs`）：照顶层把命令切成几段（`;`、`&&`、`||`、换行、单个 `&`；引号、反斜杠、括号、花括号里的不算），有一段就是 `sleep <时长…>`（PowerShell 的 `Start-Sleep`），时长合起来不短于 2 秒（写成变量这类看不出多长的当长的），整条不跑：出错，给她的一句 `no-sleep.txt`（后台任务办完会自己回报），说法 `shell/no-sleep`。管道里的、循环体里的、放到后台的（`sleep 5 &`）、两秒以内的短停照跑。
5. 起命令：
   - 程序和参数照下面「用哪个 shell」。
   - 在这一轮的工作目录里跑：`cwd` 是 `~` 开头的，照 `tilde` 接家目录（`fs.md` 第二节第 1 条，和别的工具一样），别的照原样当目录（施工 4-9 再补二）。
   - 环境变量先清空，放进照下面「环境变量」挑好的，再设上那四个。
   - 标准输入接空的：要人输入的命令读到结尾就退出（脚本另接一根管道写进参数，施工 X-12 上，`skills.md`「脚本」）。标准输出、标准错误接到同一根管道上，照写出来的先后。
   - Unix 上命令自成一个进程组；Windows 上不弹控制台窗口。组里先起一个看门的（照沙盒的助手 `miyu-sandbox lifeline` 起，退回 `/bin/sh`，施工 V-48），命令起在它的组里，整组杀照它的组号：核心崩了，它把整组杀掉（施工 7-8，`core.md`「子进程随核心退出」第 1 条）；起不来的记一行 `WARN` `command lifeline not started`，照旧自成一组。
   - 起不来（程序找不到、工作目录不在、管道建不起来）：执行不了，带上系统的原话；出错。
6. 读输出：另一个线程一次读 8192 字节，读到的一段照 UTF-8 解，一个字切在两段中间的留到下一段再解，解不开的字节换成 `�`，推给头（瞬时的 `tool.progress`，不落盘）；读完了剩下没配齐的也换成 `�` 推出去。内存里只留最前 64 KiB 和最后 64 KiB，另外数一共多少字节、多少个字、多少个换行，输出再多内存也不涨。读的线程带着派活时的 span（施工 4-9 再补四上）。
7. 等它结束，最多等超时那么久：
   - 自己结束了：记下退出码（Unix 上被信号杀掉的，记下信号）。
   - 到时了、这次调用有任务端口（后台运行开着的会话里）、命令开头不是 `sleep`（照 Claude Code）的：不杀，挪到后台（施工 7-12）。读的线程把之后解好的一段段改送进一根通道；到这一刻读到的照给她看的截法（太长的头尾两段、中间说省了多少，不写末尾那一句）先交，接着交通道里的，`\r\n` 换成 `\n`，和一开始就放到后台的同一种输出；进程连同在等它的线程交给任务端口，照「后台」第 3、4 条交回编号、记 `job.started`，给她的字照放到后台的那一句（`started.txt`），说法 `shell/moved`（字段 `job`）。挪过去的结束了照后台命令报（`job.reported`），组里剩下的照样杀掉；人、她照样停得掉。一个字正好切在挪过去那一刻两边的，那个字可能成 `�`。
   - 到时了，挪不了的（没有任务端口、开头是 `sleep`）：整组杀掉，再等它最多 5 秒，算超时。5 秒还不结束的（Windows 上 `taskkill` 没杀掉之类）也算超时，记一行 `WARN` `command still running after kill`，不再等它（施工 4-9 再补二）。
   - 等不了（系统报错）：执行不了。
8. 结束以后：Unix 上把组里还在跑的（`&` 放到后台的）也杀掉；Windows 上不杀，退出以后按编号杀可能杀错。再等读输出的线程最多 0.5 秒：还有东西拿着管道的（Windows 上它放出去的孙进程，Unix 上 `setsid` 出了组的），不等它，读到多少算多少，记一行 `DEBUG` `command output still open after the command ended`。这几行运行日志的来源是 `shell`，带会话编号（`log.md` 第 8 条；施工 4-9 再补四上：原来来源写 `basesystem`，也没有会话编号）。
9. 截给她看的（最多 30000 个字）：
   1. 内存里留的头尾接起来照 UTF-8 解（解不开的换成 `�`）。不多过 30000 个字的，整段给。
   2. 多过的，留开头 15000 个字、结尾 15000 个字：开头那一段的最后一个换行落在它后一半里的，截在这个换行后面；结尾那一段的开头已经在行首的（前一个字是换行），照原样；不然第一个换行落在它前一半里的，从这个换行后面起（施工 4-9 再补二）。一半按字节算。
   3. `\r\n` 换成 `\n`，单独的 `\r` 不动。
   4. 每一段不是空的就以换行结尾：头、`[... {count} characters omitted ...]`、尾、`(Showed the start and end of {total} characters. Output saved to a file can be read in full.)`。`count` 是一共的字数减去头尾给了的字数，`total` 是一共的字数，内存里丢掉的中间那一段也数在里面。
   - 「字」按 UTF-8 数：不是接续字节的每个字节算一个字。
10. 结果：

   | 怎么结束的 | 给她的字 | 出错 | 说法 |
   |---|---|---|---|
   | 退出码 0，一个字节都没输出 | `No output` | 否 | `shell/quiet` |
   | 退出码 0 | 输出 | 否 | `shell/done`，`count` 是几行 |
   | 退出码不是 0 | 输出，接 `Exit code {code}` | 是 | `shell/exited`，字段 `code` |
   | 被信号杀掉（Unix） | 输出，接 `Killed by signal {signal}` | 是 | `shell/signal`，字段 `signal`（信号的编号） |
   | 到时了 | 输出，接 `Stopped at the {timeout} ms timeout. timeout can go up to {max}.` | 是 | `shell/timed-out`，字段 `seconds` |

   - chromium 在沙盒里起不来（施工 SB-1，2026-10-23 项目主人定；SB-1 修二补）：退出码不是 0、被信号杀掉的，输出（截了的看头尾两段）里有 chromium 自己的这几句原话之一——`Failed to create a unique user data directory`、`The SUID sandbox helper binary was found, but is not configured correctly`、`Zygote process exited prematurely`、`No usable sandbox!`、`chrome_crashpad_handler: --database is required`、`Crash Reports`（`shell/results.rs` 的 `CHROMIUM_SIGNS`）——的，末尾再接 `chromium-sandbox.txt`。后两句是 SB-1 修二加的：加了 `--no-sandbox --user-data-dir` 以后，crashpad 照 `XDG_CONFIG_HOME` 在 `~/.config/chromium/Crash Reports` 建库，那里是沙盒藏起来的私密目录（只读的空目录），建不了就把主进程一起带崩（SIGTRAP，退出码 133）；她在这一条命令前加 `XDG_CONFIG_HOME=<临时目录>` 就过，不改挂载、不改全局的 `XDG_CONFIG_HOME`。
   - firefox 截图写了相对路径（施工 SB-1 修，2026-10-23 主人验收撞见）：命令里调了 firefox（某个词的文件名以 `firefox` 开头），`--screenshot` 跟着的路径（`=` 后面或者下一个词，去掉引号）不以 `/`、`$`、`~` 开头，或者没写路径（下一个词是网址、`data:`、别的参数，或者没有了：默认的 `screenshot.png` 也是相对的）的，不管跑成没有，末尾再接 `firefox-screenshot.txt`。firefox 起来先切到 `/`，相对路径悄悄写不出来、退出码照样 0；沙盒外也一样。chromium 照工作目录写，不接。
   - 「几行」是换行的个数，最后一段没有换行的也算一行。
   - 等人确认不算时限（施工 M-7 上）：命令里 `miyu call` 调的工具在等人确认的那几段，从时限里扣掉，到时挪到后台也照这一条；端口告诉它在等、等完了（`NestedPort::hold`），等进程的那条通道一起收，不轮询。人停掉这次调用照旧整组杀；会话被打断，在等的了结、进程照旧被杀。不另设上限：人一直不答就一直等，收尾靠停掉、打断。
   - `{timeout}` 是这一次实际用的毫秒数，`{max}` 是 600000。`seconds` 是它换成秒：整秒的不带小数，不然带一位小数（`300` 毫秒写成 `0.3`）。
11. 不报效果：改了哪些文件内核看不懂。
12. 叫停：丢掉这次调用时整组杀掉（Unix 上 `kill(-pgid, SIGKILL)`，Windows 上 `taskkill /T /F /PID`）。

**后台**（施工 7-3，`agents.md` 第四条）

1. 这次调用没有任务端口（`Call.jobs` 是空的：会话外面的调用，例如测试；后台运行关着的会话，施工 T-1 上）：不跑，说这里不能放到后台，让她在前台跑、慢的放宽 `timeout`；出错。后台运行开着的会话里的调用总有。
2. 照前台一样造命令：同一个 shell、同样挑过的环境变量、同一个沙盒（`Call.sandbox`），在这一轮的工作目录里；标准输入接空的，标准输出、标准错误接到同一根管道上，自成一组（上面第 3、5 条）。起不来、造不成的照前台说起不来。
3. 起来了，把输出和进程（`miyu_tool::Background`）交给任务端口，拿回编号。端口收不下（输出的文件建不起来、会话已经停了）：任务表已经整组杀掉了它，照起不来说，原因是端口的原话；出错，不报效果。
4. 交上了：给她的字是 `started.txt` 换进编号，说法 `shell/background`（字段 `job`），效果 `job.started`（`job` 是编号，`what` 是 `command`，`title` 是 `description`，没有 `session`）。
5. 从起命令到交回结果一个 `await` 都没有：交上了就一定交回，不会交上了却被掐掉、没人知道它在跑。
6. 交出去的输出一段一段读：一次读 8192 字节，照 UTF-8 边读边解，一个字切在两段中间的留到下一段，读完了剩下的换成 `�`；`\r\n` 换成 `\n`，一段以 `\r` 结尾的留着它看下一段，单独的 `\r` 不动；不截。读不了的当读完了，记一行 `DEBUG` `command output not readable`。
7. 交出去的进程：等它结束交回退出码，没有退出码的（Unix 上被信号杀掉）交回信号的编号。等到了先记下它结束了、再放下句柄，Unix 上组里还在跑的（`&` 放到后台的）也杀掉，和前台一样。整组杀照前台：Unix `kill(-pgid, SIGKILL)`，Windows `taskkill /T /F /PID`；已经结束了的不再按编号杀，可能杀错：杀的那一头拿着「结束了没有」那把锁杀，杀完之前句柄放不下，Windows 上编号也就不会被别的进程拿去。
8. 之后的事是任务表的：输出写进会话目录的 `jobs/<编号>.out`，结束了记 `job.reported`（`session/tools.md`「后台命令」）。
9. 写了 `watch` 的（监视，施工 MN-1 上，设计 56，`agents.md` 第四条第 8 条）：起进程时标准输出、标准错误各接一根管道（`process::spawn` 的 `apart`，三个平台一套写法；Windows 沙盒的助手本来就分开搬两路，分开保得住），各一个线程照第 6 条读：两路照到达的先后都交进 `Background::output`（写 `.out` 的那一份），标准输出另抄一份交进 `Background::lines`，任务表攒成批记 `job.lines`；标准错误不叫醒。给她的字是 `started-watch.txt`（编号、每一行标准输出都会叫醒她所以只打印要她动手的行、标准错误只进输出文件、跑到 `minutes` 分钟停下要接着看再起；分钟照任务端口的 `JobPort::longest`，即这一轮的 `jobs.max_running`，向上取整），说法照 `shell/background`。不写 `watch` 的照旧一根管道，一个字节不变。

**命令改了她看过的文件**（施工 E-D 补，项目主人 2026-10-23 定「认前台 shell 改的」，`touched.rs`）
1. 前台命令起来之前，照 `Call.seen` 记下她看过的每个文件这一刻的修改时刻和大小（不读内容；不在的、不是普通文件的记空）。
2. 跑完了（照常结束的、到时被杀的）再看一遍：修改时刻或大小变了的重读，内容哈希和她看过的不一样的，结果多一个 `file.read`（整份，不带 `lines`），照先后。不在了、不是文件、读不了的不交，`edit`、`write` 照原来的规矩报。
3. 执行器照这个效果把「看过的版本」换成新的（`session/tools.md`，同读过的），之后 `edit`、`write` 当是她自己改的、不要她重读；日志里记着，载入时照样重建。
4. 挪到后台的、被叫停的、写了 `run_in_background` 的不认：后台命令跑得久，外面改同一个文件的机会大。
5. 代价（项目主人定的时候知道）：前台命令跑着时外面恰好改了同一个文件，也当成她改的，不再拦。
6. 为什么：验收数据根里 10 次「上次读以后改过」，8 次是她读过、改过以后用 `shell`（`python3`、`sed -i`、`cat >>`）改了同一个文件再 `edit`（施工单 `E-D-编辑少失败.md`）。

**脚本调工具**（施工 M-3，设计 `39-脚本调工具.md` 第五节，`cli/call.md`）

1. 哪些命令开：拿到了端口的（`Call.nested`，执行器只给直接调的 `shell`，这个会话的工具面上有 `clicall` 开着的工具才给，`session/tools.md`「1g」）。前台、放到后台、到时挪到后台都开。开不成的（目录建不成、套接字绑不上、Windows 上）记一行 `WARN`，命令照旧跑、没有端点。
2. 端点：Unix 上一个 Unix 套接字，放在 `<临时目录>/miyu-call-<核心的进程号>/s/` 下（`0700`），名字是另取的一个随机串的前 16 个十六进制字符加 `.sock`（施工 XG-1补2 起不用令牌的：以前是令牌的前 16 个字符）；第一次建那个目录时，顺手删掉别的核心进程留下、进程已经不在了的同名目录（只删自己的）。Windows 上一根命名管道 `\\.\pipe\miyu-call-<另取的 16 个十六进制字符>`（管道名全机的进程都列得出来，带令牌的一段就等于摆出半个令牌，施工 XG-1补2 改），访问控制开给本人，另给本人沙盒里的命令（MXC 的基础容器，令牌带 `ALL APPLICATION PACKAGES`）只够连上、读写的权限（`miyu_pipe::create_for_containers`，`miyu call` 照同样几样权限开），一直留两个空着的实例等人连：连上一个，先建好下一个顶上再去答（施工 M-3 下；XG-1补2 起两个：只留一个时，连上那一刻到建好下一个之间再来连的拿到 `ERROR_PIPE_BUSY`，`miyu call` 会重试两秒，别人写的客户端不一定）。容器里的命令由助手起、放进 MXC 自己的作业对象，嵌在这条命令的作业对象里，认人照旧过得了；别的进程连得上也过不了令牌、作业对象两关。令牌 128 位，系统给的随机字节（`getrandom`）。
3. 环境变量：在 `env::passed` 挑好以后加 `MIYU_CALL_SOCK`（套接字路径、管道名）、`MIYU_CALL_TOKEN`（令牌），`PATH` 最前面（照这个系统的分隔符连，Windows 上 `Path` 不分大小写）加 `<临时目录>/miyu-call-<进程号>/bin`，里面只有一个 `miyu`：Unix 上是指向核心自己的符号链接，Windows 上是核心程序的硬链接 `miyu.exe`（建符号链接要特权），建不成的（不在同一个盘上、没有权限）把核心所在的目录放到最前面，建了的 `miyu-call-<进程号>` 当场删掉。脚本里调到的一定是同一个版本。核心走时不删自己的那个目录（Windows 上链接指着正在跑的程序），第一次建它时顺手删掉别的核心进程留下、进程已经不在了的同名目录（`miyu-call` 的 `leftovers.rs`，名字是前缀加全是数字的进程号、是目录不是链接才看）：Unix 上只删属主是自己的（临时目录大家共用），Windows 上不看属主（`%TEMP%` 是每个用户自己的；指到共用目录的，别人的目录照访问控制删不掉，别人的进程打不开、当还在），问进程在不在用 `miyu_pipe::process_running`（打开进程看退出码，拿不准的当还在，施工 WC-1）。
4. 沙盒：规格（`Call.sandbox` 的一份拷贝）的 `sockets` 加上这个端点（`sandbox.md`），只准读名单里的再放开读 `bin` 那个目录和核心的程序。Windows 上放进去的是管道名，助手不看它：容器里连得上靠管道自己的访问控制（第 2 条，施工 M-3 下）。
5. 认人（`miyu-call` 的 `kin.rs`）：请求里的令牌对得上（等时比较，每个字节都比完，不在第一个不同的字节就停，`serve.rs` 的 `same_token`，施工 XG-1补2），连上来的进程是这条命令的（扩展的工具开的端点照「是扩展进程的子孙」认，`providers.md`「会跑命令的」第 2 条）。Unix 上照对方的进程号（`SO_PEERCRED` / `LOCAL_PEERPID`）查它的进程组，要等于这条命令的那一组（`process.rs` 的 `Group`，每条命令一个看门的、一组）；Windows 上照对方的进程号（`GetNamedPipeClientProcessId`）查它在不在这条命令的作业对象里（`IsProcessInJob`）：命令起来以后马上放进它自己的作业对象（`miyu_sandbox::lifeline::Job`，套在核心那个里面；只用来认人，不设限制，整组杀照旧 `taskkill /T`，2026-10-20 照推荐定），起来到放进去之间它起的不在里面，PowerShell 起来要上百毫秒，碰不到。命令起来以后才记下这一组、这个作业对象：前台的起来就记；放到后台的、到时挪过去的，交给任务表、端口挂到那个任务下面以后才记，之前连上来的最多等 5 秒。过不了的答 `stranger.txt`，退出码 2。
6. 一次连接一个请求：4 字节大端长度加 JSON（`cli/call.md`「怎么走」第 4 条）。请求帧超过 1 MiB 的断开；同一个端点同时连着的超过 32 个，多的直接断开；协议版本对不上的答 `version.txt`。`call` 交给端口、等内核记过落了盘的结局；`schema` 照端口给的工具面答，不进内核、不记事件。
7. 结局写成退出码和字（`miyu-call` 的 `reply.rs`）：
   - 跑完成了：0，标准输出是 `data` 的一行 JSON（中文不转义），没有的是工具交回的字，都以换行结尾；丢了图的标准错误写 `images.txt`。
   - 工具出错：1，工具的原话；被拒：1，拒绝的原话；要问人：前台、有人能确认的主会话里停下来等人（施工 M-7 上，`kernel/asking.md`「脚本里调的要人确认」），允许的照常跑、拒绝的 1；后台任务下面的、没人能确认的、子会话里的 4，`approval.txt`；被打断、父级先结束：3，`interrupted.txt`。
   - 没收下：父级不在了 3，`ended.txt`；不认识 2，`unknown.txt`，能调的里面只差一两个字的接上 `guess.txt`；调不了 2，`not-allowed.txt` 列出能调的；参数不是对象 2，`bad-args.txt` 接上这件的 `schema.txt`。
   - `schema.txt` 换进名字、说明、参数格式，说明和参数不转义：是工具面上的原样，她在 tools 数组里看到的也是原样。
8. 放到后台（「后台」第 3 条）、到时挪到后台（「怎么走」第 6 条）：端点跟着进程交给任务表（`miyu_call::keep`），任务结束、进程收掉时关；交上了，端口挂到这个任务下面（`NestedPort::adopt`，2026-10-20 项目主人照推荐定）。挪的那一刻还在跑的照「父调用结束」记已取消。
9. 关：丢掉端点就掐掉服务、删掉套接字，令牌跟着作废，等着的 `miyu call` 读到结尾（退出码 3）。前台的命令结束时丢。
10. 给她看的字（施工 M-4，设计 `39-脚本调工具.md` 第九节）：组工具面时（`Agents::face`，`agents/clicall.rs`），工具面上有跑命令的工具（`Tool::runs_commands`，施工 BS-1 上起不看名字；出厂的就是 `shell`）、有脚本里能调的工具（`clicall` 开着、`Access::scriptable`；预设关了「脚本调工具」的一件都不算，施工 M-6）的，跑命令的工具（`shell`，施工 XG-1 下起还有扩展声明了 `runs_commands` 的）的说明末尾空一行接上 `Tool::clicall_section` 交回的那一段（`miyu-call` 的 `section.rs`，字在 `core/call/`），system 里带 `core/clicall.txt`（`policy.md` 的 `with_clicall`）；一件都没有的两样都不带，说明、system 一字不变。那一段一行一句：
    - `call/describe.txt`：怎么用，`{tools}` 换成能调的名单，照名字排，一个 MCP 服务的几件并成 `mcp__<服务>__*`。
    - `call/example.txt`：`Example: {example}`，核心起来时和 `{shell}` 一起挑一次：PowerShell 的用 `call/example-powershell.txt`；bash、zsh 的 `PATH` 上有 `jq` 用 `example-jq.txt`，没有、有 `python3` 用 `example-python.txt`；都没有的不带这一行。
    - `call/shapes.txt`：能调的工具里写了 `result` 的（`tools/interface.md`），`{shapes}` 换成 `名字 样子`、`; ` 隔开；一件都没有的不带这一行。
    - 换进去的都是登记来的字，照原样、不转义（同 `--schema`）。工具面在一个会话里不变，这一段也不变；人改了预设、装了包，下一个回合工具面重算时一起换。

**用哪个 shell**（核心起来时找一次，照资源目录和核心环境里的 `PATH`；`PATH` 里相对路径的那几项不看，照它找到的跟着工作目录变，核心的工作目录是数据根，施工 R3-1 补）：

| 平台 | 程序 | 参数 | 说明里的名字 |
|---|---|---|---|
| macOS | `/bin/zsh` | `-f +o nomatch -c <下面那一句>` | `zsh` |
| Windows，资源目录里有 `pwsh\pwsh.exe`（施工 SW-5：桌面端的安装包带官方的 zip，版本钉住、和沙盒一起试过） | 它 | 同下 | `PowerShell 7` |
| Windows，`PATH` 里有 `pwsh.exe` | 找到的那一个 | `-NoLogo -NoProfile -NonInteractive -OutputFormat Text -EncodedCommand <编好的命令>`（`-OutputFormat Text` 施工 SW-2；`-EncodedCommand` 起的照样写 CLIXML，施工 SW-4 起输出里认出 `#< CLIXML` 那一段换回错误、警告的原文，`shell/clixml.rs`） | `PowerShell 7` |
| Windows，没有 | `%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe`，没设 `SystemRoot` 的是 `powershell.exe` | 同上 | `Windows PowerShell 5.1` |
| 别的（Linux 等） | `PATH` 里的 `bash`，找不到用 `/bin/bash` | `--noprofile --norc -c <下面那一句>` | `bash` |

- 不读用户的启动文件：那里可能 `export` 了密钥，去掉密钥的那一步就白做了。
- bash、zsh 的命令不写进命令行（施工 4-8 再补，2026-10-18）：命令原文放在环境变量 `MIYU_COMMAND` 里，`-c` 后面是固定的一句 `eval "unset MIYU_COMMAND; $MIYU_COMMAND"`——先展开出原文，再去掉这个变量，命令和它起的子进程都看不到它，退出码是命令的。命令行里有原文时，命令里的 `pkill -f <词>` 会连自己所在的 shell 一起杀掉，只剩一句 `Killed by signal 15`（真跑碰到两次）。PowerShell 照旧编进命令行：Windows 上没有照命令行找进程杀的习惯。
- zsh 带 `+o nomatch`：没匹配到的通配符照原样传下去，和 bash 一样（没加引号的网址里的 `?`、`pip install foo[bar]` 里的方括号，施工 4-9 再补二）。
- PowerShell 的命令前面先加一句，和她的命令写在同一行（出错时报的行号还对得上），整条编成 UTF-16LE 再 base64：

  ```text
  [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false); $ProgressPreference = 'SilentlyContinue'; <命令>
  ```

  输出编码设成不带 BOM 的 UTF-8（Windows PowerShell 默认用系统的代码页）；关掉进度条（输出接到管道上时，进度条会变成一段 XML 写进标准错误）。

**环境变量**（`11-权限与沙盒.md` 第四节，4-8 补改，照 dsh、Codex）：核心的环境整份继承，去掉下面几类；名字在 Windows 上不分大小写，别处分（像密钥的那一条哪里都不分）。

| 哪一类 | 怎么认 |
|---|---|
| 像密钥的 | 名字里有 `KEY`、`SECRET`、`TOKEN`、`PASSWORD`、`PASSWD`、`CREDENTIAL`（不分大小写）：`DEEPSEEK_API_KEY`、`AWS_ACCESS_KEY_ID`、`GH_TOKEN`、`PGPASSWORD` 都在里面 |
| Miyu 自己的 | `MIYU_` 开头 |
| 沙盒里另去掉的：桌面、D-Bus、ssh-agent | `DISPLAY`、`WAYLAND_DISPLAY`、`XAUTHORITY`、`DBUS_SESSION_BUS_ADDRESS`、`XDG_RUNTIME_DIR`、`SSH_AUTH_SOCK`（2026-10-17 项目主人定：沙盒里的命令不开窗口） |
| 另外设上的，原来的那一份不要 | `GIT_TERMINAL_PROMPT=0`：git 要密码时直接失败，不等人输入；`PAGER=cat`、`GIT_PAGER=cat`、`NO_COLOR=1`：不翻页、不上色（照 dsh）；`TERM=dumb`（施工 4-8 再补）：有的程序不认 `NO_COLOR`、也不看输出接没接终端，只照终端的种类上色（实测 `cargo fmt --check` 的 diff），`dumb` 的一律不上 |
| 这一轮的事实（施工 E-M） | `MIYU_MODEL`、`MIYU_EFFORT`：这一轮发给的模型名、思考强度，同尾巴上模型那一块（还没解析过模型的不设）；`MIYU_SESSION_LOG`：会话日志的目录，只给不关起来的会话（群、陌生人的私聊看不到运行它的那台电脑）。核心环境里原有的 `MIYU_*` 照旧不传，这三个是另设的 |

- 名字不是 UTF-8 的照能读出来的那部分判，不像密钥的照传。
- 这只防不小心带出去（她跑一下 `env`，密钥就进了对话），不是安全边界：同一个系统用户的命令读得到核心的 `/proc/<pid>/environ`。
- 原来只传白名单，名单外的一律不传：桌面的（`DISPLAY`）、工具链的（`KUBECONFIG`、`PKG_CONFIG_PATH`）都丢了，终端里能跑的命令到这里就不行，名单补不完（2026-10-17 项目主人报，照推荐改）。
- 拿到的是核心起来时的环境（`config.md`「密钥」）：核心是从 SSH 或者桌面起来以前拉起的，就没有 `DISPLAY`；桌面重登以后那几个值也是旧的。

**给人跑**（施工 SH-1 上，设计 57 第二节第 1 条，`Tool::run_for_person`）：人在头里用 `!` 打的一条，执行器调这个钩子，她的调用碰不到。

1. 同一个程序（「用哪个 shell」）、同一套挑过的环境变量（上面的表，不套沙盒，所以桌面的那几个照传）、同一个起法（自成一组、看门的、标准输入接空的、两路合一根管道）、同一个截法（第 7 条的头尾两段，中间一行写省了多少）。在交来的目录里跑（会话那一刻的工作目录，执行器已经接好 `~`）。
2. 给她设的几道挡不过：不查高危（那是权限链的事，这条路不过权限链）、不拦干等的 `sleep`（第 4 条）、不开 `miyu call` 的通道、不看她看过的文件改没改、不挪到后台。
3. 时限照不写 `timeout` 时的默认 120 秒，不开参数调：到了整组杀，交回到这时的输出，结束是 `TimedOut`。
4. 叫停（`Halt`）：等的那根通道多一种「停」（`process/waits.rs`），举起来就整组杀，交回到这时的输出，结束是 `Interrupted`。它的前台、后台调用不收这一种。
5. 交回退出码（被信号杀掉的是信号，Unix）、合法化过的输出、截掉几个字、用时；起不来的交回出错，原话照系统的。

### 样子

`printf 'x\n'; exit 2`：

```text
x
Exit code 2
```

输出太长时（Linux 上 `seq 1 20000`，一共 108894 个字；这一页用 `…` 省掉了中间的行）：

```text
1
2
…
3221
[... 78896 characters omitted ...]
17501
…
20000
(Showed the start and end of 108894 characters. Output saved to a file can be read in full.)
```

- 头：前 15000 个字截在 `3222` 的中间，最后一个换行落在后一半里，截在 `3221` 那一行后面，给 14998 个字。
- 尾：后 15000 个字正好从 `17501` 那一行的开头起，已经在行首，照原样给 15000 个字。
- 省了 108894 − 14998 − 15000 = 78896 个字。说法是 `shell/done`，`count` 是 20000。

给她的字都在 `resources/software/basesystem/shell/` 下，每一份以一个换行结尾，登记在 `26-提示词.md` 第十节：

| 什么时候 | 文件 | 原文 |
|---|---|---|
| 退出码 0、没有输出 | `empty.txt` | `No output` |
| 退出码不是 0 | `exit.txt` | `Exit code {code}` |
| 被信号杀掉 | `signal.txt` | `Killed by signal {signal}` |
| chromium 在沙盒里起不来（施工 SB-1、修二） | `chromium-sandbox.txt` | `Chromium can't start its own sandbox, profile or crash reporter inside this sandbox, which already contains it. It runs as XDG_CONFIG_HOME=$(mktemp -d) chromium --no-sandbox --user-data-dir=$(mktemp -d) <your other arguments>.` |
| firefox 截图写了相对路径（施工 SB-1 修） | `firefox-screenshot.txt` | `Firefox switches to / before it writes --screenshot, so a relative path is dropped without an error. An absolute path such as "$PWD/shot.png" works.` |
| 到时了 | `timed-out.txt` | `Stopped at the {timeout} ms timeout. timeout can go up to {max}.` |
| 放到后台了 | `started.txt` | `Started {job} in the background. Its end is reported automatically. jobs output shows its output.` |
| 监视的放到后台了（施工 MN-1 上） | `started-watch.txt`，字段 `job`、`minutes` | `Watching {job} in the background. Each stdout line wakes you, so print only lines you must act on. stderr goes only to the output file. It stops after {minutes} minutes; start it again to keep watching.` |
| 截在中间 | `omitted.txt` | `[... {count} characters omitted ...]` |
| 截了，末尾 | `truncated.txt` | `(Showed the start and end of {total} characters. Output saved to a file can be read in full.)` |
| 执行不了 | `failed.txt` | `Could not run {shell}: {error}.` |
| 要放到后台，没有任务端口 | `no-background.txt` | `Running in the background is not available here. Foreground runs can take a larger timeout.` |
| 认出灭顶的高危，拒了（施工 5-17） | `hazard.txt` | `Not run: this command {reason}, which is never allowed.` |
| 同上，`{reason}` | `hazard/delete-root.txt`、`delete-home.txt`、`delete-system.txt`、`delete-data-root.txt`、`disk-format.txt`、`disk-write.txt`、`container-host.txt`（施工 5-17 三补） | `would delete the root directory`、`would delete the home directory`、`would delete a top-level system directory`、`would delete Miyu's own data`、`would format a disk`、`would write to a raw disk device`、`would give a container full access to this machine's devices` |

输出本身不转义；换进这几句的字段照模板的规矩转义（`tools/read.md` 第 8 条）。

### 高危（施工 5-17）

报给权限策略（`Tool::hazard`，`session/guard.md` 第四条末），不进 `run`。只认 bash、zsh 的写法：用 PowerShell 的（Windows）不报，转义、删除命令、路径的写法都不一样，另排：

1. 读参数，读不懂的不报。
2. 照命令的字面认（`miyu_policy::hazard::assess`）：工作目录照家目录换成绝对路径，家目录、数据根照这次调用带的。
3. 拆命令：`&&`、`||`、`;`、`|`、`|&`、`&`、换行、括号是边界；去掉单双引号、反斜杠；`$( )`、反引号、`<( )`、`>( )` 里面的另认一遍，套到第 8 层以下的看不透、照问人（`Kind::TooDeep`，施工 HZ-1：不归哪一类，设置页的开关关不掉；对外的写法照旧是 `delete_unresolved`）；`>` 后面的是写到的地方，`<` 后面的、紧贴着重定向的纯数字不是参数。heredoc（`<<结束符`、`<<-结束符`，`$( )` 里面的也一样）的正文从下一行起到结束符那一行为止，不拆、里面的撇号不当引号；结束符没加引号的，正文里的 `$( )`、反引号另认（施工 AS-9：原来 `git commit -m "$(cat <<'EOF' … don't … EOF)" && …` 从撇号起一直吞到结尾）。
4. 剥外壳：开头的 `名字=值`，`sudo`、`doas`、`env`、`nice`、`nohup`、`timeout`、`time`、`stdbuf`、`command`、`exec`、`xargs` 连同它们的选项；`sh -c`、`bash -c` 这类和 `eval` 的里面另认：`-c` 和别的选项连写的（`bash -lc`、`sh -ec`）也算，脚本是它后面第一个不是选项的词（施工 AS-9）。要带值的选项落在命令末尾的（`sudo -u`、`xargs -I`），照剥完了算，剩下的是空的、不报（鲁棒性 B-1：原来越界 panic，整个会话崩掉）。开头的 `if`、`then`、`elif`、`else`、`do`、`while`、`until`、`!` 也剥（施工 5-17 再补）。
   - 同一条命令里单独成句的赋值（`T=…`，`export`、`local`、`declare`、`typeset`、`readonly` 后面的）照先后记下，删的目标里的 `$名字`、`${名字}` 照记下的换（施工 5-17 再补）：值开头的 `~` 照家目录接，值里又引了别的变量的照记下的换，换不掉的忘掉这个名字；`for`、`select` 的循环变量和 `read`、`mapfile`、`unset` 碰过的名字也忘掉；只给一个命令的（`T=x rm …`）、管道里的不算。没赋过值的 `$HOME` 照家目录换。`${名字:?}`、`${名字:-…}`、`${名字=…}` 照这个名字换（shellcheck 推荐 `"${HOME:?}"`），`${名字:+…}` 换不掉（施工 AS-9）。替换里、`sh -c` 里的另起一份。
   - 同一条命令里 `mktemp` 刚建的临时路径删了不问（施工 HZ-2，2026-11-12 项目主人定）：单独成句的 `x=$(mktemp …)`、`` x=`mktemp …` ``（`export`、`local` 这类不带选项的也算）以后，`rm` 的目标去掉引号以后正好是 `$x`、`${x}` 的不判，同一个 `rm` 里别的目标照旧各判各的。`mktemp` 每次新建一个唯一的空路径，删的就是它自己刚建的；拿临时目录跑测试最常见的就是这么写，每次都问只会让人习惯性点允许。边界（core1 加，拿不准的一律照旧判，代码在 `hazard/fresh.rs`）：
     - 只认不带后缀的引用：`$x/*`、`"$x"/`、`$x/..`、`${x}foo`、`${x:-…}` 照旧判。`mktemp` 失败时 `x` 是空的，`rm -rf $x/*` 就是 `rm -rf /*`；空的 `$x` 什么也删不到。
     - 赋值正好是一个命令替换（前后拼了字的不算），里面正好一条 `mktemp`：没有第二条命令、管道、重定向、`&`、套着的替换；参数写死，没有变量、命令替换、通配、空白；认的是不带参数、`-d`、`-u`、`-q`、`-t`（可以连写）、`-p 目录`、`--directory`、`--dry-run`、`--quiet`、`--tmpdir[=目录]`，加至多一个模板。`-u` 只给名字不建，删一个不存在的路径无害。
     - 赋值一定在这个 shell 里跑到：不在括号、花括号、`if`、`case`、循环里，前面不是 `&&`、`||`，后面不是 `&`，不在管道里，不是只给一个命令的；不然 `x` 可能还是外面带进来的值。
     - 赋值以后、删以前改过 `x` 的不算：有词以 `x=`、`x+=`、`x[` 开头（`export x=…`、`let x=1`、只给一个命令的 `x=/ true`），`read`、`mapfile`、`unset`、`getopts`、`printf`、`let`、`declare`、`local`、`export`、`wait`、`for`、`select` 这类后面的词里出现 `x` 这个名字（`declare -n r=x` 也算）。删以后才改的不影响前面那次删。不照先后跑的改法整层不认：块里（循环、函数、`if`）任何一处改 `x`，原文任何一处有 `${x:=…}` 这类带修饰的、`((…))`、`$((…))` 里出现 `x`；这一层出现 `eval`、`source`、`.`、`trap`、`alias`、`case`、`IFS` 的，整层什么名字都不认。
     - 只认同一条命令里的：替换里、`sh -c` 里的另起一份，上一次调用里赋的不算；`find $x -delete` 照旧（`x` 空了 `find` 就删当前目录）；别的命令替换照旧问。
     - 拆命令时替换只留一个 `$`，`$(echo /)x` 看着和 `$x` 一样：带命令替换的词不当 `$x`，也不照记下的变量换（原来 `T=/tmp/zt; rm -rf $(echo /)T` 照删 `/tmp/zt` 放过去了）。
     - 没管的：没加引号的 `$x` 会被 shell 再按空白拆开，`TMPDIR` 里有空白时 `rm -rf $x` 删的是拆开的几段；Linux、macOS 上 `TMPDIR` 照惯例没有空白，写死的赋值（上一款）也一样不管拆开。
   - 同一条命令里单独成句的 `cd`、`pushd` 照先后记下当前目录（施工 AS-9）：不写的是家目录，写了的照上面的办法换；`cd -`、`popd`、换不掉的之后算说不清，整个往下删相对目标的照删不清。删的目标里相对的照这时的当前目录接；「工作目录或它上面的」照旧比会话的工作目录。
5. 认：`rm`、`rmdir`、`unlink`、`shred`、`find … -delete`（`-exec rm`）的目标照字面换成绝对路径；通配的照它删得到的最大的那一个（施工 AS-9，通配只看最后一段）：整段都是通配的（`*`、`.*`、`?*`、`{*,.*}`）照删它所在的目录，`前缀*` 照删同一层里名字合前缀的家目录、数据根、工作目录（或通往它们的那一级），根下面的照根下那一级，有尾巴的（`*.log`）照原样；是根、家目录或它上面的、根下一层、数据根或装着它的（默认工作区 `workspace/` 里面除外）、工作目录或它上面的；带变量、命令替换又是整个往下删的照删不清，经 `xargs` 喂目标、整个往下删的也照删不清（施工 AS-9）；git 跳过 `-C`、`-c`、`--git-dir`、`--work-tree`、`--namespace` 和别的选项再认子命令：`push` 带 `-f`、`--force*`、`+分支`，`reset --hard`，`clean -f`，`checkout .`（不是 `-b`），`restore .`（只带 `--staged`/`-S` 的不算，带了 `--worktree`/`-W` 的照算，施工 AS-9），`stash drop`、`stash clear`；`mkfs*`、`wipefs`、`mke2fs`、`blkdiscard`、`mkswap`、`sgdisk -Z`/`--zap-all`，`dd of=`、`>`、`tee`、`shred`、`cp`（最后一个参数）写 `/dev/sd*` 这类裸设备（施工 AS-9 加上 LVM、LUKS 的 `/dev/mapper/…`、`/dev/dm-…`，软 RAID `/dev/md…`，`/dev/loop…`、`/dev/nbd…`）；`shutdown`、`reboot`、`halt`、`poweroff`、`init 0`、`init 6`、`systemctl poweroff` 这类；`curl`、`wget` 接到读标准输入的 shell 或解释器，`bash <(curl …)`、`sh -c "$(curl …)"`，在这个 shell 里执行的 `source <(curl …)`、`. <(wget …)`、`eval "$(curl …)"`（施工 AS-9）；容器（施工 5-17 三补，`docker`、`podman`、`nerdctl` 的 `run`、`create`、`container run`）：`--privileged`、`--cap-add` 加 `ALL`/`SYS_ADMIN`、把宿主的 `/`、`/dev`、`/sys` 整个挂进去（`-v`、`--volume`、`--mount` 的 `source`）是 `container_host`（拒），`--device`、`--device-cgroup-rule`、`--pid`/`--net`/`--network`/`--ipc` 写 `host`、挂 `/dev`、`/sys` 下面的某一个是 `container_share`（问人）；入口是 systemd 本身不拦（不带特权的 `podman --systemd=always` 是测 systemd 单元该用的）。
6. 几处都认出来的：有拒的交回拒的那一种（`Hazard::Refused`），带写给她的一句和说法（上面的表）；都是问的全交回（`Hazard::Asks`，施工 HZ-1），照认出的先后、同一种只留一次，每一种带协议里的写法（`code`）和归设置页「受保护的命令」的哪一类（`class`：`git`、`delete`、`power`、`pipe_to_shell`、`container`；看不透的没有）。问哪一种、关掉的那几类问不问由权限策略判（`session/guard.md` 第四条末）。这一段在 `crates/miyu-basesystem/src/shell/hazard.rs`（施工 HZ-1 从 `shell.rs` 挪出来）。

### 出错

| 什么时候 | 给她的字 | 说法 |
|---|---|---|
| 参数不对 | `Bad arguments: {error}.` | `common/bad-args`，字段 `error` |
| 要放到后台，没有任务端口 | `no-background.txt` | `shell/no-background` |
| 这一轮的工作目录没了（删了、挪走了），前台、后台都不跑（施工 CW-1，2026-11-02 项目主人定：照实说、不换到别处跑） | `no-cwd.txt`，字段 `cwd` | `shell/no-cwd`，字段 `cwd` |
| 认出灭顶的高危（权限策略拒的，不进 `run`，施工 5-17） | `hazard.txt` 填上那一种 | `shell/hazard-<种类>` |
| 起不来、等不了；后台的任务端口收不下 | `Could not run {shell}: {error}.` | `shell/failed`，字段 `error` |
| 退出码不是 0、被信号杀掉、到时了 | 见「怎么走」第 10 条 | 同上 |

- `{shell}` 是说明里的那个名字；`{error}` 是系统的原话。工作目录不在的，施工 CW-1 起先查、照 `no-cwd.txt` 说，不再报起进程的原话。

### 给人看的字

显示名：执行命令（Run），后面跟 `command` 的值；符号 `$`；`block` 是 `command`：`miyu ask` 里写成 `$ 命令`，不写显示名，下面印她看到的输出（`cli/ask.md`「执行命令那一块」）。

| 说法 | 中文 | 英文 |
|---|---|---|
| `shell/done`（`count`） | `{count} 行`（施工 T-8 补：「输出」两个字多余，项目主人 2026-10-22 定） | `{count} lines` |
| `shell/quiet` | 没有输出 | no output |
| `shell/exited`（`code`） | `退出码 {code}` | `exit code {code}` |
| `shell/signal`（`signal`） | `被信号 {signal} 停掉了` | `killed by signal {signal}` |
| `shell/timed-out`（`seconds`） | `超过 {seconds} 秒，停掉了` | `stopped after {seconds} s` |
| `shell/failed`（`error`） | `执行不了：{error}` | `can't run: {error}` |
| `shell/no-background` | 这里不能放到后台跑 | can't run in the background here |
| `shell/hazard-delete_root` | 没有执行：会删掉根目录 | not run: would delete the root directory |
| `shell/hazard-delete_home` | 没有执行：会删掉家目录 | not run: would delete the home directory |
| `shell/hazard-delete_system` | 没有执行：会删掉系统目录 | not run: would delete a system directory |
| `shell/hazard-delete_data_root` | 没有执行：会删掉 Miyu 自己的数据 | not run: would delete Miyu's own data |
| `shell/hazard-disk_format` | 没有执行：会格式化磁盘 | not run: would format a disk |
| `shell/hazard-disk_write` | 没有执行：会直接写磁盘设备 | not run: would write to a disk device |
| `shell/hazard-container_host` | 没有执行：会让容器拿到整台机器的设备 | not run: would give a container full access to this machine's devices |
| `shell/background`（`job`） | `放到后台了：{job}` | `running in the background as {job}` |
| `shell/moved`（`job`） | `超时，挪到后台了：{job}` | `timed out, moved to the background as {job}` |
| `common/bad-args`（`error`） | `参数不对：{error}` | `bad arguments: {error}` |

### 三个平台

| | Linux | macOS | Windows |
|---|---|---|---|
| 用哪个 | bash | zsh | PowerShell 7，没有就 Windows PowerShell 5.1 |
| 整组杀 | `kill(-pgid, SIGKILL)` | 同 Linux | `taskkill /T /F /PID`，杀整棵进程树 |
| 命令退出以后剩下的 | 杀掉 | 杀掉 | 不杀 |
| 被信号杀掉 | 说是哪个信号 | 同 Linux | 没有信号，总有退出码 |
| 后台的整组杀 | 同前台 | 同前台 | 同前台；结束了的不再杀 |
| 环境变量的名字 | 分大小写 | 分大小写 | 不分 |
| 标准错误和标准输出的先后 | 照写出来的先后 | 同 Linux | PowerShell 两条输出各有缓冲，先后不一定 |

### 守着它的

| 测试 | 守哪几条 |
|---|---|
| `crates/miyu-basesystem/tests/person.rs`、`src/shell/person/tests.rs`（施工 SH-1 上） | 真的起进程、三个平台：给人跑的交回退出码、两路合在一起的输出；人打的 `sleep` 照跑、她的同一句照旧拒 `shell/no-sleep`；叫停整组杀、交回到这时的输出；到时（测试里换短的）整组杀、带着到这时的输出 |
| `crates/miyu-basesystem/tests/watch.rs`（施工 MN-1 上） | 真的起进程、三个平台：写了 `watch` 的放到后台、结果那一句（30 分钟）；标准输出单独交进 `lines`、标准错误不在里面，写文件的那一份两路都有；不写的没有 `lines`；没有任务端口的不跑 |
| `crates/miyu-basesystem/tests/background.rs`（施工 7-3） | 真的起进程、三个平台：当场返回编号、说法、`job.started`（标题是 `description`），命令接着跑；标准错误合进来、`\r\n` 换成 `\n`；退出码；`timeout` 不管后台的；环境变量和前台一样（4-8 补：带着探针重新拉起测试程序）；杀掉就停；没有任务端口的不跑；端口收不下的说起不来、不报效果。Unix：整组杀连它放到后台的、自己退出以后组里剩下的也停了、信号照原样、单独的 `\r` 不动、最后半个字换成 `�` |
| `crates/miyu-session/tests/jobs_moved.rs`（施工 7-12） | 真的 `shell`、三个平台：前台到时挪到后台，调用当场交回编号、`job.started`，跑完了记 `job.reported`，输出是挪过去以前读到的接上以后的（`\r\n` 换好）；挪过去以前输出太长的照给她看的截在中间；人停得掉；开头是 `sleep` 的照旧杀、不进任务表；Linux 上结束了组里放出去的也杀掉 |
| `crates/miyu-basesystem/src/shell/waiting/tests.rs`、`tests/human.rs`（2026-10-26） | 前台干等的认法：顶层的、不短于两秒的 `sleep`、`Start-Sleep` 拒（几种时长写法、变量当长的、`;`、`&&`、`||`、换行、后台以后的那一段）；管道、循环体、放到后台的、引号里的、套在 `sh -c` 里的、短停照跑；拒了说 `shell/no-sleep` |
| `crates/miyu-basesystem/src/shell/output/tests.rs` 的 `line_ends_are_made_unix_across_pieces`（施工 7-3） | `\r\n` 切在两段中间也换，和一整段换出来的一样 |
| `crates/miyu-basesystem/tests/shell_sandbox.rs` 的 `a_sandboxed_background_command_runs_through_the_helper`（施工 7-3） | 后台命令一样经沙盒的助手起 |
| `crates/miyu-basesystem/tests/sandbox_thread.rs`（施工 V-50，Linux） | 认缓存规则集的指纹不占着跑异步任务的线程：单线程的运行时里另一个任务每 1 ms 醒一次，前台、后台、脚本工具起关起来的命令时它最晚晚的不到认一遍指纹的一半（藏起来的放在几万个文件的目录里，认一遍几十毫秒）；脚本工具的规格标了不缓存（施工 V-52），不带 `--ruleset`、当场包 |
| `crates/miyu-basesystem/tests/script_uncached.rs`（施工 V-52，Linux） | 脚本工具的规格不进核心的规则集缓存：假助手 `/bin/echo` 印出参数，接连 10 次脚本都不带 `--ruleset`；之后 `shell` 交的还是缓存里原来那一份（测试拿着头一次的规则集，重建了号码就不一样） |
| `crates/miyu-basesystem/tests/shell_seen.rs`（施工 E-D 补） | 她看过、命令改了的带 `file.read`（改后的哈希），没动的、没看过的不带；只动了修改时刻、内容没变的不带；命令起来之前外面就改过、命令没碰的不带（照旧要重读） |
| `crates/miyu-basesystem/tests/shell.rs` | 说明里写的是这台机器的 shell、换过字段、不报路径；在工作目录里跑；标准错误合进来、照先后；退出码；没有输出；超时整组杀、说法的秒数；核心的变量照传、像密钥的和 `MIYU_*` 不给、`PAGER` 是 `cat`（4-8 补：测试进程里不许 `set_var`，带着探针重新拉起测试程序自己，`support::in_child`）；中文照原样；太长截成头尾、一共多少个字，尾巴从行首起的不多丢一行；参数不对、要放后台的不跑、没写 `description` 参数不对；工作目录不在、`~` 开头的照家目录接；输出边跑边推、最后半个字换成 `�`；Unix：放到后台的在命令退出以后停了、叫停时整组停了、被信号杀掉；照命令里的词 `pkill -f` 杀不到自己所在的 shell、命令看不到 `MIYU_COMMAND`（施工 4-8 再补） |
| `crates/miyu-basesystem/src/shell/tests.rs` | 超时的上下限；秒数怎么写；参数格式里写的上限、默认值和代码一样；每一段以换行结尾 |
| `crates/miyu-basesystem/src/shell/output/tests.rs` | 数行、数字；`\r\n`；内存只留头尾；截在行尾；中间丢过的照样数；一整行很长的照字数截；正好 30000 个字的整段给；切开的字等配齐；离截处太远的换行不用 |
| `crates/miyu-basesystem/src/shell/program/tests.rs` | 各系统用哪个 shell、找不到时用什么、`PATH` 里的相对路径不看（施工 R3-1 补）；名字带版本；不读启动文件、只带给的变量；zsh 带 `+o nomatch`、没匹配到的通配符原样传下去（macOS）；PowerShell 的编码和前面那一句；带了沙盒的四种 shell 都经助手起、参数照先后、工作目录和环境变量照旧，规格写不成 JSON 的起不来（施工 5-1） |
| `crates/miyu-basesystem/tests/shell_sandbox.rs` | 带了沙盒的真跑一次（Unix）：假助手 `/bin/echo` 收到的是 `run --spec <规格> -- <shell> <参数…>`；规格写不成 JSON 的说起不来，命令没跑（施工 5-1）；`a_sandboxed_command_gets_no_desktop`（4-8 补）：假助手 `/bin/sh` 读工作目录里的 `run`，沙盒里拿不到 `DISPLAY` |
| `crates/miyu-basesystem/src/shell/process/tests.rs` | 杀不掉的命令，最多再等那么久就交回超时 |
| `crates/miyu-basesystem/tests/log.rs` | 运行日志的来源是 `shell`；输出还没关那一行在阻塞线程里发，带会话编号（Linux） |
| `crates/miyu-basesystem/src/shell/env/tests.rs` | 去掉像密钥的、`MIYU_*`，别的照传；沙盒里另去掉桌面的；另设的四个总是我们的；Windows 上名字不分大小写；密钥的词在名字哪里都算；不是 UTF-8 的名字照传（4-8 补） |
| `crates/miyu-basesystem/tests/script.rs` 的 `a_script_gets_the_same_environment_as_shell`（4-8 补） | 脚本拿到的环境变量和 `shell` 一样，自己那两个照设（Unix） |
| `crates/miyu-basesystem/tests/human.rs` | 每一种结果的说法，两种语言都换得出字 |
| `crates/miyu-session/tests/write.rs` | 会话里真的跑：结果进日志，给她的是输出加退出码，说法里有退出码，没有效果 |
| `crates/miyu-session/tests/guard.rs` | 完全放开执行命令不问（沙盒能不能用都一样），只读时沙盒能用不问、用不了问（没人能确认就拒绝） |
| `crates/miyu-basesystem/tests/shell.rs` 的 `shell_says_what_to_show_when_asking`（施工 D-4）、`a_call_with_the_retired_outside_sandbox_parameter_still_runs`（施工 LV-2） | 问人时交 `command`、`title`，参数不对的交空的；旧会话里带着 `outside_sandbox`（施工 LV-2 去掉的参数）的调用照常跑 |
| `crates/miyu-call/src/leftovers/tests.rs`（施工 WC-1） | 留下的目录：名字对得上、是目录、进程不在了的整个删，名字差一点的、文件、进程还在的、不是自己的不动；进程号只认全是数字、放得进 `u32` 的 |
| `crates/miyu-pipe/src/process/tests.rs`（施工 WC-1，只在 Windows 上） | 自己在跑；等它退了的子进程不在，句柄关了也不在；没有的进程号不在 |
| `crates/miyu-call/src/reply/tests.rs`（施工 M-3） | 结局写成退出码和字：一行 JSON、中文不转义；丢了图的说一句；出错、被拒、要问人、被打断、父级不在了各自的码和话；写错名字的指出对的；调不了的列出能调的；参数不对的跟上格式；`--schema` 的样子 |
| `crates/miyu/tests/call.rs`（施工 M-3，Unix） | 真核心、真 `shell`：能调的一行 JSON、退出码 0，子 shell 里再起一层照样；调不了的、写错名字的退出码 2；`--schema`；`setsid` 出去的退出码 2（Linux）；命令结束以后同一个端点连不上，退出码 3；没有那两个变量的退出码 2；每次调用记 `tool.nested`、挂在那次 `shell` 下面、排在它的结果前面。放到后台的挂在任务下面（`j1#1`）、不带回合、排在任务报告前面；到时挪到后台的，挪过去以后的调用挂到任务下面 |
| `xtask/src/ledger.rs` | 这些字的指纹和登记簿对得上 |

### 出处

- `10-自带软件.md` 第三节（「`shell` 的细则」）、第五节（`shell` 不报改了哪些文件）、第七节（经过 `shell` 的改动撤销不了）、第八节（B8：各平台的 shell）、第十节（不记 `cd`；参数名照 Claude Code）。
- `agents.md` 第四条（后台命令，施工 7-3）。
- `11-权限与沙盒.md` 第二节（沙盒能用时执行命令不问，用不了时问人）、第四节（环境变量：继承、去掉像密钥的，4-8 补改）。
- `26-提示词.md` 第十节：登记簿里的 `tools/shell.json`、`shell/*.txt`。

### 还没有的

- PowerShell 的高危规则（施工 5-17 只认 bash、zsh）：`Remove-Item -Recurse`、反引号转义、`C:\` 路径。

- 沙盒：命令现在以本人的身份直接跑，碰得到任何地方；只读时每条都问人（`11-权限与沙盒.md` 第二节、第六节，M5）。放行规则照命令开头记，也随 M5（第二节）。
- Windows 上改用 Git Bash 的配置（`10-自带软件.md` 第八节）。
- 环境变量的规则是策略数据，现在写在代码里。像密钥的也要传的（例如 `gh` 要的 `GH_TOKEN`）放行的配置项：2026-10-17 项目主人定先不做，要用的在命令里自己 `export`。
- 配置里 `{ env = "<变量>" }` 引用的变量名不像密钥的（例如起名 `MAGPIE`）照传：要从会话一路交到执行器，没接。
