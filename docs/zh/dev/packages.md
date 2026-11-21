# 写一个 Miyu 软件包

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

给第三方作者。这个目录里的说明、模板和例子用 Apache-2.0（仓库根目录 `LICENSES/Apache-2.0.txt`），照着写的东西用什么许可证都行；只经协议和 Miyu 说话的软件包、扩展是独立的作品，核心的 AGPL 管不到它们。协议的每个方法查 [协议参考](../reference/README.md)。

## 包能带什么

**一个文件夹就是一个包**，文件夹名就是软件包的编号，清单是文件夹里的 `package.toml`。一个包可以带下面这些东西，也可以几样混在一起带：

| 想做的 | 带什么 | 看哪一篇 |
|---|---|---|
| 接一个现成的 MCP 服务 | `[[mcp]]` | [接 MCP 服务](mcp.md) |
| 给 AI 一件小工具，几十行脚本就够 | `scripts/` 下的一个脚本 | [放文件就能用的](scripts-skills.md) |
| 教 AI 一套做事的办法 | `skills/` 下的一个技能 | 同上 |
| 一条斜杠命令，展开成一句话发给 AI | `commands/` 下的一个模板 | 同上 |
| 一套给 AI 查询的资料（设定集、手册、笔记） | `kb/` 下的一个知识库 | 同上 |
| 一个人格、一个预设 | 放在 `personas/`、`presets/` 下 | 同上 |
| 常驻的程序：一组工具、接一个聊天平台、往界面上放东西、自己的斜杠命令 | 扩展（`[process]`） | [写扩展](extensions.md) |
| 网页里的一个设置页 | 后台页（`[page]`） | [写扩展](extensions.md) |

`scripts/`、`skills/`、`commands/`、`kb/`、`personas/`、`presets/` 按约定的位置放，清单里不需要写；清单里每一项怎么写，可以看 [清单](manifest.md)。用 Python、Rust 写扩展时，有现成的开发包可以用（见 [用开发包写](extensions.md#用开发包写)）。

**不想做成包的**：技能、脚本、命令模板可以直接放进 `~/.miyu/root/skills/`、`scripts/`、`commands/`，MCP 服务写进配置，同样能用，只是不归软件包管理。

**从模板开始**：在 [miyu-package-template](https://github.com/miyu-agent/miyu-package-template) 上点击 Use this template，就能得到一个可以打包、可以发版的仓库。里面的包文件夹 `my-package` 就是编号，改名时按那个仓库 README 里列出的清单修改五处。

## 一个包长什么样

```text
hello/
├── package.toml          # 清单
├── bin/
│   └── miyu-hello        # 扩展的程序：编译出来的程序（Windows 上是 miyu-hello.exe），或者带 #! 的脚本
├── scripts/
│   └── dice.py           # 脚本
├── skills/
│   └── take-notes/
│       └── SKILL.md      # 技能
├── kb/
│   └── hello-docs/       # 知识库：里面是 kb.toml 和文档
└── page/
    └── index.html        # 后台页
```

1. **编号**（文件夹名）：以小写字母开头，只用小写字母、数字、`-`、`_`，最多 64 个字符；不能是 Windows 的设备名（`con`、`nul`、`com1` 这类），也不能是数据根里保留的名字（例如 `config`、`packages`、`models`、`scripts`、`personas`，用了会报 `reserved_id`）。起一个短的、能说清它是什么的名字就可以，不需要带 `miyu-`，也不需要带包的种类（`scripts-`、`skills-` 这类）。编号不能和随 Miyu 出厂的包重复（出厂的包都叫 `base-<名字>`，例如 `base-read`、`base-memory`，所以不要用 `base-` 开头），也不要和官方的包（`tui`、`webui`、`embed`、`im` 等）、其他人的扩展重复（因为再安装同一个编号就是升级，会替换掉别人已经安装的那一个）。如果要发布到软件源，起名之前先运行 `miyu pkg search <词>`，看看这个编号有没有被占用：软件源的编号先到先得（见下面的「发到软件源」）；这几篇里的 `hello`、`dice` 只是例子，官方源里已经有 `dice` 了。
2. **带的名字也不要重复**：如果工具名、技能、MCP 服务、斜杠命令和已经安装的其他包重复，这个包就无法安装。建议在名字前面加上包的编号作为前缀。
3. **包文件夹安装好以后不要往里面写东西**：安装时每个文件都记录了哈希，后来写进去的文件会被 `miyu pkg check` 报告出来。运行时的状态写进 `MIYU_STATE_DIR` 指向的状态目录，给人修改的配置放进 `MIYU_CONFIG_DIR` 指向的配置目录（其余几个目录见扩展那一页）。
4. **人的数据不要放进状态目录**：卸载软件包时，状态目录会被整个删除，卸载之前的确认画面会写 `将删除 <路径>（<大小>）`。人自己的东西（导出的记录、迁移过来的资料、生成给人看的文件）放进 `MIYU_DOCUMENTS_DIR`（`root/documents/`）、`MIYU_PICTURES_DIR` 这类人看得见的地方，卸载软件包时不会动它们；状态目录只放你这个包自己使用的东西（数据库、登录状态），卸载时会跟着包一起删除。
5. **给人修改的东西放进配置目录**：`MIYU_CONFIG_DIR`（`etc/<编号>/`）和 pacman 一样，卸载软件包时默认保留，卸载之前的确认画面会写 `配置保留在 <路径>`，重新安装同一个编号时仍然能读到；只有人运行 `miyu pkg remove --purge` 时才会把它一起删除。清单里声明的设置项（`[settings.<名字>]`，写在 `etc/config.toml` 里）也一样：卸载软件包时默认保留、但不生效，`miyu check` 只会提示一句这是你的包留下的，重新安装后按原样生效，只有 `--purge` 才会删除。所以，人手动修改的设置文件放进配置目录，卸载时会保留；你自己的状态放进状态目录，卸载时会删除。不要把应该跟着包一起删除的东西写进配置目录。

## 装、试、查

1. **先看一眼**：`miyu pkg info ./hello` 按安装时的那一套规则检查清单，列出包里带了什么（脚本、技能、人格、命令、设置项、需要的能力），但什么都不安装；清单写错时在这里就会报告，并指出是第几行。
2. **安装**：运行 `miyu pkg install ./hello`。它会先输出一份将要做的事（带了什么程序、添加什么命令、需要什么能力以及每一项的说明、来源、大小），然后询问 `[Y/n]`：回答是就安装，同时批准它需要的能力；加上 `--yes` 不询问，同样算作批准。安装好以后立即生效，不需要重启核心；设置为一直运行的扩展（`start = "always"`）安装后就会开始运行。
3. **打开**：需要手动打开的扩展，安装好以后会询问一句「现在打开？」，回答是就打开，因为能力在安装时已经批准过，所以不会再问；回答否的话，以后可以在终端界面或网页界面的「软件包」页里打开。只带技能、脚本、知识库、人格的包没有开关，每个技能、脚本在预设里单独开关。
4. **修改后重新安装**：开发时修改完，再运行一次 `miyu pkg install ./hello` 就能换上新的版本（也就是升级）。如果想在一份单独的数据里试用，不影响自己平常使用的数据：在每条命令前面都加上 `MIYU_HOME=/tmp/miyu-try`，试完以后运行 `MIYU_HOME=/tmp/miyu-try miyu core stop` 停掉那一份数据的核心，再删除目录。
5. **查看**：`miyu pkg list`（安装了哪些包）、`miyu pkg info hello`（名称、版本、带了什么、大小、安装时间）、`miyu pkg files hello`（安装了哪些文件）、`miyu pkg check hello`（安装好的文件有没有被改过）、`miyu check`（清单、脚本、技能有没有写错）、`miyu kb list`（知识库，包带的知识库写着 `来自 <包名>`）。在「软件包」页上可以看到扩展是正在运行、在等待重启，还是已经停止（停止的会写明原因和标准错误的最后几行）。
6. **日志**：扩展的标准错误在 `~/.miyu/var/log/<编号>/stderr`；核心的运行日志在 `~/.miyu/var/log/core.log`。
   - **让核心一直运行**：核心启动以后就不会因为空闲而退出，扩展跟着它一直运行，通讯平台的桥、定时的任务不需要等人打开 `miyu`。如果要在开机、登录时就启动，运行 `miyu service enable`（在 Linux 上是 systemd 用户服务，在 macOS 上是 launchd，在 Windows 上是任务计划）。
7. **卸载**：运行 `miyu pkg remove hello`。包文件夹和它存放状态的目录会一起删除；配置目录和设置里它的项默认保留（见上面第 5 条），只有运行 `miyu pkg remove --purge hello` 才会一起删除；密钥文件里的条目不删除（因为其他地方可能也在使用）。

## 常见报错

| 看到的 | 怎么办 |
|---|---|
| 无法打包：清单没有写版本 | 在 `[package]` 里写上 `version = "0.1.0"`。 |
| 无法打包：package.toml:<行>: … | 清单的这一行写错了，后面是原因（例如 `未知的键：package.id`），和安装时报的是同一句。按照原因修改清单，见 [清单](manifest.md)。 |
| 无法打包：… 把 enum 写在了不是字符串的类型上 | 包里 `tools/<名字>.json` 的参数把 `enum` 写在了整数、数字或真假上。把这几个选项改成字符串，或者去掉 `enum`、写进说明（见 [写扩展](extensions.md)）。 |
| 需要指定目标系统 | 清单里写了 `targets`：运行 `miyu pkg pack --target <系统>`，每种系统打一个包。 |
| `[[slash]] needs the commands capability`（`bad_slash`）这类「要某项能力」 | 带了 `[[slash]]`、`[[search]]`、`[guard]`、`[after]`、`[[driver]]` 的包，要在 `[process] capabilities` 里写上对应的能力（见 [清单](manifest.md) 的各节）。 |
| `package_conflict` | 包带的名字（命令、工具、技能、知识库、MCP 服务、前缀等）和已经安装的其他包重复了，每一处重复写一行，写明和谁重复；改名以后再安装。 |
| `depends_missing` | `[depends]` 里的包没有安装，或者版本不够，先安装它。 |
| `needs_newer_miyu` | 清单里的 `[package] miyu` 比正在运行的 Miyu 新，需要更新 Miyu。 |
| 调方法回 `needs_approval` | 清单没有声明那项能力，或者人还没有批准：把它加进 `capabilities`，再安装一次。 |
| 扩展停下了 | 先在「软件包」页里查看原因，再看 `var/log/<编号>/stderr`；退出码 1 表示「重启也没用」，核心不会再启动它。 |
| 脚本、技能没出现 | 运行 `miyu check`：开头写错的、名字重复的，都会在这里说明为什么被跳过。 |

## 发到软件源

1. **写版本**：清单里的 `[package] version` 按 semver 写（例如 `0.1.0`）。不写版本的包可以安装，但是无法打包，也无法进入软件源。每发布一次都要提高一次版本：软件源只看版本，同一个版本重新发布不算更新。
2. **打包**：`miyu pkg pack ./hello` 按安装时的那一套规则检查清单，打成 `hello-<版本>.miyu`，并输出 SHA-256 和大小；用 `-o <目录>` 可以换输出的位置。如果清单里写了 `targets`，每种系统打一个包：例如 `--target linux-x86_64`，文件名里会多一段系统名。版本库和缓存不会打进去：`.git`、`.github`、`.hg`、`.svn`、`__pycache__`、`.DS_Store`、`*.pyc`，在哪一层都一样；其他文件都会打进去，所以要留在包外面的东西，打包之前先挪走。打出来的文件可以先在一份单独的数据里试着安装：`miyu pkg install ./hello-0.1.0.miyu`。
3. **发布**：把包放在你自己的 GitHub 仓库里，加一份 `.github/workflows/release.yml`，使用官方的打包模板：

   ```yaml
   name: release
   on:
     push:
       tags: ["v*"]
   jobs:
     pack:
       uses: miyu-agent/packages/.github/workflows/pack.yml@main
       permissions:
         contents: write
       with:
         package: hello
   ```

   打一个 tag（`v` 加上清单里的 `version`，例如 `v0.1.0`）并推送上去，模板就会按 Miyu 安装软件包时的那一套规则检查清单、打成 `.miyu`，并上传到这个 tag 的 Release；tag 和 `version` 对不上时会当场报错。如果包文件夹不在仓库根目录，就写上 `folder`；如果打包之前要先编译程序，就写上 `build`（一段 bash）；如果写了 `targets`，就用 `strategy.matrix` 每种系统运行一遍（`runs-on`、`target`）。详细的见 [miyu-agent/packages](https://github.com/miyu-agent/packages)。也可以不用模板，自己运行 `miyu pkg pack`，再把 `.miyu` 上传到 Release：此时不要改文件名，因为收录时会根据文件名核对编号、版本和系统。
4. **进入社区收录**：给仓库加上话题（topic）`miyu-package`。软件源每天收录一次（UTC 03:17 前后），每个仓库只看最新的那个正式 Release（草稿、预发布的不算），收录里面的 `.miyu`，签名以后发布出去，标为「社区」：所以发布以后最多要等一天。一个 Release 里可以放好几个包，同一个包的几份文件版本必须相同。分叉出来的仓库、已经存档的仓库不会被收录；如果 Release 里有一个 `.miyu` 无法读取，这个仓库这一次整个都不会被收录。
5. **编号先到先得**：第一个被收录了某个编号的仓库会一直占用这个编号（记录在软件源的 `names.toml` 里），其他仓库再用这个编号时不会被收录；如果同一次收录里有两个仓库第一次争同一个编号，创建得早的那个仓库得到它。改名就等于换一个新的编号。
6. **确认有没有收录进来**：先运行 `miyu pkg update`，再运行 `miyu pkg search <编号>`。如果没有找到，就看软件源仓库的 Actions 里最近一次 collect 的摘要，每个没有收录的仓库都有一行原因（`skipped, a fork`、`no package file in the latest release`、`hello-0.1.0.miyu should be …` 这类）。作恶的包和仓库会被拉黑（记在 `blocked.toml` 里）；进入官方列表需要项目主人同意。

## 经系统源发

也可以通过系统的包管理器（AUR、Homebrew 等）发布，Miyu 自己的终端界面、网页界面就是这样发布的（`miyu-tui`、`miyu-webui` 两个包）。Miyu 不另外设置审核：系统源由各家自己审核，核心只负责在安装以后按规则读取、检查。

1. **装到哪**：把包文件夹按原样安装到 Miyu 识别的「系统加装」那一处，成为 `<那一处>/<编号>/package.toml`：
   - Linux：`$XDG_DATA_DIRS` 里每一个目录下的 `miyu/packages/`（没有设置时是 `/usr/local/share`、`/usr/share`，排在前面的优先），系统包安装到 `/usr/share/miyu/packages/<编号>/`；Linuxbrew 装在它自己的 `share` 下面，用户需要把它写进 `XDG_DATA_DIRS`；
   - macOS：Homebrew 前缀下的 `share/miyu/packages/`（前缀按 `miyu` 所在目录的上一级计算）；
   - Windows：没有这个位置，只能通过软件源发布。
2. **包文件夹的样子**：和 `miyu pkg pack` 打进 `.miyu` 的那一份相同，清单保持原样，程序放在包文件夹的 `bin/` 里，并且带有可执行权限。不要放进核心自己的目录（`/usr/lib/miyu/`、Homebrew 的 `libexec/`）：那是核心的出厂层，归 `miyu-core` 管理。
3. **包名和依赖**：包名建议写成 `miyu-<编号>`。依赖写 `miyu-core`（核心的包；`miyu-agent` 是合集，不要依赖它），版本下限写第一个支持你清单里 `protocol` 的核心版本（现在协议只有 `1`，已经发布的核心都支持，所以不写下限也可以）。如果依赖其他 Miyu 包，清单里的 `depends` 和系统包的依赖都要写上。
4. **装好以后**：核心在启动时读取一次（如果核心已经在运行，需要重启核心才能识别）。`miyu pkg list` 里这个包所在的层是 `distro`。如果清单的协议、适用系统、依赖对不上，这个包不会加载，`miyu check` 会报告原因；写错时，坏掉的只是这一个包。
5. **和软件源一起发**：同一个包可以两边都有。用户用 `miyu pkg install` 安装的那一份放进数据根，会覆盖系统安装的那一份；系统安装的那一份更新时，`miyu check` 会报一条 `shadowed` 警告，并说明怎么换回去。
6. **卸载**：对系统安装的包运行 `miyu pkg remove <编号>`，只会记录下来、不再加载，文件不会被删除，删除文件要用系统的包管理器；记录过的包可以按编号用 `miyu pkg install <编号>` 装回来。
7. **自己先试**：把环境变量 `MIYU_DISTRO_PACKAGES` 指向一个目录，核心就只把那个目录当作系统加装的那一层；再配置一个临时的数据根 `MIYU_HOME`，运行 `miyu pkg list` 看层是不是 `distro`，运行 `miyu pkg check <编号>` 看程序能不能找到。

AUR 的 `PKGBUILD` 大致是下面这样（按自己的包修改）：

```sh
pkgname=miyu-hello
depends=(miyu-core)
package() {
  install -d "$pkgdir/usr/share/miyu/packages"
  cp -a hello "$pkgdir/usr/share/miyu/packages/hello"
}
```

Homebrew 的 formula：

```ruby
depends_on "miyu-core"

def install
  (share/"miyu/packages/hello").install Dir["hello/*"]
end
```

## 三个平台

1. 扩展的程序名按清单里写的来，`bin/<名字>` 可以是编译出来的程序，也可以是第一行写了 `#!` 的脚本（例如 `#!/usr/bin/env python3`）。在 Windows 上，核心先找 `<名字>.exe`，找不到时根据 `bin/<名字>` 的 `#!` 查找解释器（`python3` 找的是 `python`；如果写的是这台机器上存在的绝对路径，就使用这个路径），这样三个平台用一份文件、一份清单。Miyu 不附带解释器：需要哪一个解释器，要写进包的说明，请人自己安装。脚本工具在 Windows 上如果没有 `#!`，就根据扩展名查找解释器（`.py` 用 `python`，`.ps1` 用 PowerShell，`.js` 用 `node`）。
2. 在 Linux、macOS 上，程序和脚本都要有可执行权限和 `#!` 那一行；安装时按原样拷贝，权限也会跟着保留。
3. macOS（只考虑 Apple 芯片）：从网上下载的程序带有隔离标记，没有签名、没有公证的程序会被 Gatekeeper 拦截。发布出去的程序请签名并公证。
4. 在三个平台上，「请退出」都是用标准输入读到结尾来表示的，不要依赖信号。

## 给模型看的字

工具和技能的说明、参数说明、你放进 AI 上下文里的文字，都是给模型看的，每一轮都会跟着请求发送，所以越短越省钱：

- 写英文短句。一件工具最多写三句：它是什么、什么时候用、最容易踩的那一个坑。
- 调用以后才用得上的知识（字段是什么意思、怎么翻页）写进工具的**输出**，不写进说明。
- 给人看的名字、说明另外有地方写（`display_name`、`summary`，按语言分别写），不要混进给模型看的文字里。
