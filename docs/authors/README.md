# 写一个 Miyu 软件包

给第三方作者。英文版：[README.en.md](README.en.md)。协议的每个方法查 [协议参考](../reference/README.md)。

## 包能带什么

**一个文件夹就是一个包**，文件夹名就是编号，清单是文件夹里的 `package.toml`。一个包能带下面这些，几样混着带也行：

| 想做的 | 带什么 | 看哪一篇 |
|---|---|---|
| 接一个现成的 MCP 服务 | `[[mcp]]` | [接 MCP 服务](mcp.md) |
| 给 AI 一件小工具，几十行脚本就够 | `scripts/` 下一个脚本 | [脚本、技能、命令模板](scripts-skills.md) |
| 教 AI 一套做事的办法 | `skills/` 下一个技能 | 同上 |
| 一条斜杠命令，展开成一句话发给 AI | `commands/` 下一个模板 | 同上 |
| 常驻的程序：一组工具、接一个聊天平台、往界面上放东西、自己的斜杠命令 | 扩展（`[process]`） | [写扩展](extensions.md) |
| 网页里的一个设置页 | 后台页（`[page]`） | [写扩展](extensions.md) |

清单里每一格怎么写见 [清单](manifest.md)。

**不想做成包的**：技能、脚本、命令模板直接放进 `~/.miyu/home/admin/skills/`、`scripts/`、`commands/`，MCP 服务写进个人设置，一样能用，只是不归包管理。

## 一个包长什么样

```text
hello/
├── package.toml          # 清单
├── bin/
│   └── miyu-hello        # 扩展的程序：编出来的程序（Windows 上是 miyu-hello.exe），或者带 #! 的脚本
├── scripts/
│   └── dice.py           # 脚本
├── skills/
│   └── take-notes/
│       └── SKILL.md      # 技能
└── page/
    └── index.html        # 后台页
```

1. **编号**（文件夹名）：小写字母开头，只用小写字母、数字、`-`、`_`，最多 64 个。不能和随 Miyu 出厂的包撞（`basesystem`、`memory`、`roleplay`、`mermaid`、`net`、`tui`、`web` 这些），也别和别人的扩展撞（同一个编号再装是升级，会换掉人家装着的那一个）。
2. **带的名字也别撞**：工具名、技能、MCP 服务、斜杠命令和装着的别的包撞了的，装不上。名字建议带上包的编号做前缀。
3. **包文件夹装好以后别往里写**：装的时候每个文件都记了哈希，后来写进去的会被 `miyu pkg check` 报出来。运行时的状态写进 `MIYU_PACKAGE_STATE` 指的状态目录。

## 装、试、查

1. **装**：`miyu pkg install ./hello`。先印一份要做什么（带了什么程序、加什么命令、要什么能力、多大），问一句 `[Y/n]`；`--yes` 不问。装好当场生效，不用重启核心。
2. **打开**：装好以后问一句「现在打开？」，答是就把它要的能力一并批了、打开；答不的，以后在终端或网页的「软件包」页里打开。只带技能、脚本的包没有开关，每个技能、脚本在预设里各自开关。
3. **改了再装**：开发时改完再 `miyu pkg install ./hello` 一次就换上了（就是升级）。想在一份单独的数据里试，不碰自己平常用的：`MIYU_HOME=/tmp/miyu-try miyu …`。
4. **看**：`miyu pkg info hello`（名称、版本、大小、装的时间）、`miyu pkg files hello`（装了哪些文件）、`miyu pkg check hello`（装好的文件改没改）、`miyu check`（清单、脚本、技能写没写错）。「软件包」页上看得到扩展在跑、在等着重启、还是停下了（停下的写原因、标准错误的最后几行）。
5. **日志**：扩展的标准错误在 `~/.miyu/state/logs/<编号>.stderr`；核心的运行日志在 `~/.miyu/state/logs/core.log`。
6. **卸**：`miyu pkg remove hello`。包文件夹、它放状态的目录、设置里它的项一起删；密钥文件里的条目不删（别处可能也在用）。

## 发到软件源

1. **打包**：`miyu pkg pack ./hello` 照装的那一套查清单，打成 `hello-<版本>.miyu`，印 SHA-256 和大小。清单写了 `targets` 的，一种系统打一个：`--target linux-x86_64`。
2. **发**：包放在自己的 GitHub 仓库里，用官方的打包模板 `miyu-agent/packages/.github/workflows/pack.yml`：打 tag（`v` 加上清单的 `version`）就打包、传进这个 tag 的 Release。怎么写见 [miyu-agent/packages](https://github.com/miyu-agent/packages)。
3. **收录**：软件源每天收一次各仓库最新的 Release，签名后发出去。现在只收官方仓库，社区包的自动收录以后再开。

## 三个平台

1. 扩展的程序名照清单写，`bin/<名字>` 可以是编出来的程序，也可以是第一行写了 `#!` 的脚本（`#!/usr/bin/env python3` 这样）。Windows 上核心先找 `<名字>.exe`，没有的照 `bin/<名字>` 的 `#!` 找解释器（`python3` 找的是 `python`，写了这台机器上在的绝对路径的照它），三个平台一份文件、一份清单。Miyu 不带解释器：要哪个写进包的说明，请人自己装。脚本工具在 Windows 上没有 `#!` 的照扩展名找（`.py` 用 `python`，`.ps1` 用 PowerShell，`.js` 用 `node`）。
2. Linux、macOS 上程序、脚本要有可执行权限和 `#!` 那一行；装的时候照原样拷，权限跟着走。
3. macOS（只考虑 Apple 芯片）：从网上下载的程序带隔离标记，没签名、没公证的会被 Gatekeeper 拦。发出去的程序请签名并公证。
4. 「请退出」三个平台都是标准输入读到头，别靠信号。

## 给模型看的字

工具和技能的说明、参数说明、你往她上下文里放的字，都是给模型看的，每一轮跟着请求发，越短越省钱：

- 写英文短句。一件工具最多三句：是什么、什么时候用、最容易踩的那一个坑。
- 调用以后才用得上的知识（字段什么意思、怎么翻页）写进工具的**输出**，不写进说明。
- 给人看的名字、说明另有地方写（`display_name`、`summary`，照语言），不混进给模型的字里。
