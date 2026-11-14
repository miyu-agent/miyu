# Miyu

Miyu 是一个跑在你自己电脑上的 AI 助手：可以用来写代码、干活，也可以用来聊天、角色扮演。支持 Linux、macOS 和 Windows。

它由一个没有界面的核心，和几个可以随时换掉的「头」组成：终端界面、网页、托盘、命令行、终端无缝集成，以及 QQ、Telegram 这样的通讯平台。核心仿照 Linux 来设计：内核只提供机制，策略都是数据；一切状态都是事件日志的投影；模型驱动、工具和扩展，都是登记到内核里的模块。

## 安装

Linux、macOS：`curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh | sh`；Windows 在 PowerShell 里：`& ([scriptblock]::Create((irm https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.ps1)))`。AUR、Homebrew、托盘安装包和升级、卸载见 [安装](docs/guide/install.md)。

装好后敲 `miyu`，头一次会带你配好模型，见 [快速上手](docs/guide/quickstart.md)。

## 文档

| 给谁 | 看哪 |
|---|---|
| 头一次用 | [快速上手](docs/guide/quickstart.md) |
| 用 Miyu 的人 | [用户手册](docs/guide/README.md) |
| 改 Miyu 代码的人 | [开发者](docs/dev/README.md) |
| 写软件包、扩展、技能的人 | [作者教程](docs/authors/README.md) |
| 写头、扩展、SDK 的人查协议 | [协议参考](docs/reference/README.md) |

`docs/blueprint/`、`docs/designs/`、`docs/construction/` 是项目内部的图纸、设计和施工记录，用 Miyu 不用看。

## 许可证

核心（这个仓库自己写的代码和资源）用 AGPL-3.0-or-later，见 `LICENSE`：改过的 Miyu 拿去给别人用，包括只在网上提供服务，要把改过的源码交给用它的人。

给别人照着写东西用的这几处用 Apache-2.0（`LICENSES/Apache-2.0.txt`）：`docs/authors/`（写软件包、扩展、脚本、技能、MCP 服务的说明、模板和例子）、`docs/reference/`（协议参考）、`docs/guide/`（用户手册）、`docs/dev/`（开发者文档）、`resources/packages/base-authoring/`（教 AI 写软件包、技能的那两份技能）。开发包 `miyu-sdk` 也用 Apache-2.0。

只经协议和 Miyu 说话的程序是独立的作品：扩展、软件包、脚本、MCP 服务器、另写的终端或网页，用什么许可证都行，AGPL 管不到它们。

不是这个仓库自己写的东西照它们原来的许可证，AGPL 和 Apache 只管这里自己写的部分：Rust 依赖照各自的 `license`；仓库里收的第三方数据（模型目录、分词词表这些）清单和各自的许可证见 `docs/blueprint/licenses.md`「资源里的第三方数据」；发行包里带的字体、ONNX Runtime 这些照各自的许可证，随包附上原文。

供应商、模型的名称和图标是各自所有者的商标，归各自所有者。Miyu 只在运行时从 models.dev、lobe-icons 取来缓存在本机，不随包发；用到它们不表示这些所有者认可或赞助 Miyu。
