# Miyu 文档

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

| 你是 | 读 |
|---|---|
| 头一次用 Miyu | [快速上手](guide/quickstart.md) |
| 在用 Miyu，要查一件事怎么做 | [用户手册](guide/README.md) |
| 要写技能、脚本、软件包、扩展 | [作者教程](authors/README.md) |
| 要写一个头，或者让别的程序连上核心 | [协议参考](reference/README.md)、[写一个头](dev/head.md) |
| 要弄懂 Miyu 的架构，或者给它提改动 | [开发者文档](dev/README.md) |

## 这个目录里有什么

| 目录 | 是什么 | 给谁 |
|---|---|---|
| `guide/` | 用户手册：安装、各个界面、会话、模型、权限、软件包、配置、排错 | 用 Miyu 的人 |
| `authors/` | 作者教程：写软件包、扩展、脚本、技能，接 MCP 服务，清单的格式 | 给 Miyu 写东西的人 |
| `reference/` | 协议参考：核心的每个方法、参数、回应、出错 | 写头、写扩展的人 |
| `dev/` | 开发者文档：架构、写一个头、主题和皮肤、往界面上放东西、贡献代码 | 接入 Miyu、改 Miyu 的人 |
| `blueprint/`、`designs/`、`construction/`、`perf/`、`reviews/` | 项目内部的图纸、设计、施工记录、性能数据、调研 | 维护者。用 Miyu、给它写东西都不用看；各是什么见 [贡献代码](dev/contributing.md#先读什么) |

`authors/`、`reference/` 有中文、英文两份，中文是正本，英文的文件名是 `<页>.en.md`。

`guide/`、`authors/`、`reference/`、`dev/` 用 Apache-2.0（仓库根目录 `LICENSES/Apache-2.0.txt`）：照着写东西、转载、翻译，用什么许可证都行。
