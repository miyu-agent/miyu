## 施工 WN-1：Windows 对齐的一批小修

状态：已完成（2026-11-14），两项等拍板（见「等拍板」）。写文档的会话 11-14 做三平台摸底，查出 Windows 上和 Linux、macOS 不一致的几处；项目主人同一天定危险命令拦截发版前补上 PowerShell 和 cmd。每条先查清现象是真的再修。

### 1. 危险命令拦截认 PowerShell 和 cmd

**现象（真的）**：`shell` 的高危规则（`miyu_policy::hazard`）只拆 bash、zsh 的写法，`crates/miyu-basesystem/src/shell/hazard.rs` 见 PowerShell 直接交回「没认出来」。Windows 上她的命令一律走 PowerShell（`shell/program.rs`：装了 PowerShell 7 用它，不然用自带的 5.1；cmd 的写法只会出现在 `cmd /c "…"` 里），所以 Windows 上删家目录、格式化、关机一样都不拦、不问。

**改了什么**：`miyu_policy::hazard::assess_powershell`（`hazard/windows.rs`，拆命令 `windows/lex.rs`、cmd 的拆法 `windows/cmd.rs`、删除 `windows/delete.rs`、路径 `windows/paths.rs`）。和 bash 那一路共用判目标的那一段（`paths::judge`，多一个 `drives`：盘符算一段）、合成结果的那一段（`rules::settle`）、git 和容器的规则。基础系统照 shell 的种类挑哪一路。

规则清单（口径同 bash 那一路：同样的种类、拒和问照同样的分法、归同样的类）：

| 种类 | 拦的（例子） | 不拦的相近常用命令 |
|---|---|---|
| 删根（拒） | `Remove-Item -Recurse -Force C:\`、`rd /s /q C:\`、`cmd /c "rd /s /q C:\"` | `cmd /c dir C:\` |
| 删家目录（拒） | `Remove-Item -Recurse ~`、`rm -r -fo $env:USERPROFILE`、`Remove-Item "$HOME\*" -Recurse`、`Get-ChildItem ~ \| Remove-Item -Recurse`、`cmd /c rd /s /q %USERPROFILE%` | `Remove-Item C:\Users\me\AppData\Local\Temp\x -Recurse`、`Get-Content $HOME\.miyu\etc\config.toml` |
| 删系统目录（拒） | `Remove-Item C:\Windows -Recurse`、`ri -r $env:SystemRoot`、`Remove-Item 'C:\Program Files' -Recurse`、`cmd.exe /c "del /s /q C:\Windows\*"` | `Remove-Item build -Recurse -Force` |
| 删数据根（拒） | `Remove-Item -LiteralPath C:\Users\me\.miyu -Recurse`、`del C:\Users\me\.miyu\etc\config.toml` | `Remove-Item -Recurse C:\Users\me\.miyu\workspace\s1\out` |
| 删工作目录（问，归「递归删除」） | `Remove-Item -Recurse -Force .`、`Get-ChildItem \| Remove-Item -Recurse` | `Remove-Item .\node_modules -Recurse`、`Get-ChildItem *.tmp \| Remove-Item` |
| 删不清的（问，归「递归删除」） | `Remove-Item -Recurse $target`、`$files \| Remove-Item -Recurse`、`ForEach-Object { Remove-Item $_ -Recurse }` | `rm *.log`、`Remove-Item -Recurse -WhatIf ~` |
| 格式化（拒） | `Format-Volume`、`Clear-Disk`、`Remove-Partition`、`format D:`、`cmd /c format D:` | `Format-Table`、`Format-List`、`Format-Hex`、`Get-Partition` |
| 关机重启（问，归「关机和重启」） | `Stop-Computer`、`Restart-Computer`、`shutdown /s /t 0`、`shutdown.exe /r`、`cmd /c shutdown /h` | `shutdown /a`、`Stop-Process` |
| 下载后直接执行（问，归「下载后直接执行」） | `irm … \| iex`、`iwr … \| Invoke-Expression`、`iex (irm …)`、`iex ((New-Object Net.WebClient).DownloadString(…))`、`$s = irm …; iex $s`、`irm … \| powershell -` | `irm … \| ConvertFrom-Json`、`iwr … -OutFile a.zip`、`iex 'Get-Date'`、`irm … \| Out-File a.ps1` |
| git、容器（同 bash 那一路） | `git push --force`、`git.exe reset --hard`、`& 'C:\…\git.exe' clean -fdx`、`docker run --privileged`、`docker run --device …` | `git push origin main`、`docker run --rm x` |

拆法、外壳、路径的细则写在蓝图 `tools/shell.md`「高危」末的「PowerShell」。要点：

- 程序名、参数名、路径不分大小写；参数名照 PowerShell 的规矩可以只写开头（`-r`、`-rec`），`-Path:值` 也认；cmd 的单字母开关（`/s`、`/q`）不是目标；bash 习惯的 `-rf` 也算整个往下删（PowerShell 上这么写会报错，照删的意思判）。
- 盘符本身算根目录、盘符下一级算系统目录（同 bash 那一路的「根下一级」）：`C:\temp`、`C:\dev` 这类自建的顶层目录也算，照 `rm -rf /opt` 拒。
- `$env:SystemDrive`、`$env:SystemRoot` 照家目录所在的盘算（这一层不碰系统，拿不到真的系统盘）。
- 网络路径（`\\server\share`）、注册表等别的提供程序（`HKCU:\…`）不判。
- 不认：变量里存的路径（bash 那一路有 `vars.rs`，这一路没有，照删不清问）、`-EncodedCommand`（刻意绕的）、往裸设备写（`\\.\PhysicalDrive0`）。

**等拍板**：`Set-ExecutionPolicy Unrestricted`/`Bypass`、`reg delete HKLM…`（连同 `Remove-Item HKLM:\…`）没有对应的种类，现在不认。要认就得加新的种类：协议里多一个 `hazard` 写法、头要有它的说法、问人的要归哪一类（现有五个开关都不合适，要么不归类、关不掉，要么设置页多第六个开关）。推荐：加一种 `system_config`，问人、归新的第六类「改系统设置」（三语、配置键 `permission.confirm.system`），`reg delete` 只认 `HKLM`（`HKCU` 下删自己软件的键很常见，不拦）。

**怎么验**：`hazard/tests/windows.rs` 八个测试，每种拦的、不拦的各几条；`crates/miyu-session/tests/hazard.rs` 原来 Windows 上不编，现在打开（`rm -rf ~`、git 那几条两边认出来的一样，Windows 上照 PowerShell 跑一遍，要看三台 CI）。改前红：`assess_powershell` 换成交回空的以后，`hazard/tests/windows.rs` 八条里七条红（剩下的是「不该拦的」那一条）；`miyu-session` 那一组原来在 Windows 上不编，Windows 上照 PowerShell 跑要看三台 CI。

### 2. 开头是 `Start-Sleep` 的到时照杀

**现象（真的）**：`waiting.rs` 的 `sleeps` 只认开头是 `sleep` 的：后台运行开着时，到时没跑完的命令挪到后台，开头是 `sleep` 的照杀（等着本来就是它要做的）。PowerShell 的 `Start-Sleep` 不认，被挪到后台。前台干等的拦截（`waits`）原来就认 `Start-Sleep`。

**改了什么**：`sleeps` 不分大小写地认 `Start-Sleep`。测试 `a_leading_sleep_is_not_moved_to_the_background`，改前红。

### 3. `~\` 各处都认

**现象（真的）**：`miyu_fs::tilde`（文件工具、知识库、`shell` 的工作目录照它）Windows 上认 `~\`；`sandbox.private.add`（`miyu_fs::written`）、`skills.external.dirs`（`well_written`）、MCP 的 command 第一项（`mcp/home.rs`）、`miyu check` 的路径（`check.rs`）各写了一份，只认 `~/`。

**改了什么**：这几处都照 `miyu_fs::tilde`，不再各写一份。Linux、macOS 上 `\` 不是分隔符，`~\x` 照旧不认（同 `tilde`）。测试照平台断言（Windows 上认、别处不认）：`miyu-fs` 的 `a_backslash_after_the_tilde_follows_the_platform`、`miyu-store` 的同名测试、MCP 的 `a_tilde_command_follows_the_home` 加一段；改前红只在 Windows 上看得到。

### 4. 人格、预设的编号不许是 Windows 设备名

**现象（真的）**：编号的写法（`miyu_store::layers::valid`，人格、预设、知识库共用）只管小写字母开头、字符、长度，`con`、`nul`、`com1` 这些都收；编号要当目录名、文件名用，Windows 上建不出来。

**改了什么**：设备名的判法从 `miyu_fs::usable` 里拆成 `miyu_fs::windows_reserved`（`CON`、`PRN`、`AUX`、`NUL`、`COM1`～`COM9`、`LPT1`～`LPT9`，不分大小写，带扩展名也算），`valid` 照它拦，三个系统一律拦。知识库编号写错的提示加一句「不能是 con、nul、com1 这类 Windows 保留名」。测试 `missing_and_badly_named_personas_are_refused`、预设的 `find` 那一组，改前红。

另有三份同样的表没动（`miyu-kernel` 的 `id.rs`、`miyu-config` 的 `key.rs`、`miyu-store` 的 `attachments.rs`，`miyu-source` 的 `archive.rs` 多了 `COM0`、`LPT0`）：合成一份要动四个 crate 的依赖，这一步不做，记在这里。

### 5. Windows 上在家目录开会话：没改，报告

**查下来**：核心这一侧和 Linux、macOS 一样有提示。工作目录太宽时（Windows 上家目录本身就算，`miyu-fs` 的 `wide.rs`），造会话的回应里 `cwd` 是退回的默认工作区，`miyu ask` 照它说「警告：工作目录 ~ 范围过大，改用 …」（`ask/follow/opening.rs` 的 `moved`，测试 `a_directory_too_wide_says_where_she_works`），不分平台。不提示的是终端、网页两个头：它们不看这一格，Linux 上同类情形（在 `/` 或数据根里开）一样不提示。头怎么提示是头的产品决定，这一步不动，转给界面会话。

### 6. 「受保护的命令」五个开关在 Windows 上不起作用

**查下来**：开关本身不分平台（权限策略照 `HazardAsk.class` 去掉关掉的那几类），不起作用是因为第 1 条：Windows 上一种都认不出来，没东西可关。第 1 条补上以后五类在 Windows 上都认得出（git、容器的命令两边一样，删除、关机、下载后执行照 PowerShell 的写法认），开关照样管用，不用写「暂时不生效」。「下载后直接执行」的说明加上 PowerShell 的写法（`irm … | iex`），三语。

### 7. 几处说法

- Windows 命名管道出错的两句（`miyu-ipc` 的 `OpenError::Occupied`、`ConnectError::NotPrivate`）原来照套接字说（「不是套接字的文件」「别人也能进」），Windows 上改说命名管道：「命名管道 … 已经被别的程序占了」「命名管道 … 另一头不是自己的进程：不连」。这两句只有中文（`miyu-ipc` 本来就只写中文）。测试 `a_pipe_is_called_a_named_pipe`，改前红。
- Windows 沙盒探不出基础容器时的告警：Windows 的沙盒施工 5-9 改上以后是 MXC 的基础容器（只管写，读关不住），「此版本在 Windows 上还不能把命令关进沙盒」和现状对不上，改成「这台 Windows 起不了 MXC 的基础容器」/「this Windows cannot start an MXC base container」（`miyu ask` 的字只有中英两种）；`miyu-sandbox` 的两处注释同改。不加提示行：基础容器要什么条件代码和文档里都没写，不编。
- 「无法升级：没有这种系统的发行包」加提示：发行包有哪四种、别的系统到发行页拿源码自己编译（中英）。测试 `an_unknown_system_is_told_what_to_do`，改前红。

### 蓝图改哪几处

- `tools/shell.md`：「高危」开头一句、末尾加「PowerShell」一段，「还没有的」去掉 PowerShell 那一条。
- `cli/ask.md`：Windows 没有手段那一行的字。
- `cli/upgrade.md`：没有这种系统的发行包加提示。
- `config.md`：`permission.confirm.pipe_to_shell` 的说明。
