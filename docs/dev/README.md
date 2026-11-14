# 开发者文档

> <sub>本文档由 Anthropic 的 Claude Fable 5.2 生成。</sub>

给要扩展 Miyu、接入 Miyu 或给它改代码的开发者。文档在三处：`authors/` 是写软件包的教程，`reference/` 是协议参考，`dev/` 是架构和贡献代码。照你要做的事找：

| 我想做的事 | 读哪一篇 |
|---|---|
| 写一个技能、一个脚本工具、一条斜杠命令模板 | [放文件就能用的](../authors/scripts-skills.md) |
| 接一个现成的 MCP 服务 | [接 MCP 服务](../authors/mcp.md) |
| 写一个带工具的扩展 | [写扩展](../authors/extensions.md)；每个方法见 [协议参考：扩展](../reference/extensions.md) |
| 写一个模型驱动 | [写扩展：写一个模型驱动](../authors/extensions.md#写一个模型驱动) |
| 做一个软件包、发到软件源 | [写一个 Miyu 软件包](../authors/README.md)；清单每一格见 [清单](../authors/manifest.md) |
| 往终端、网页的界面上放东西 | [往界面上放东西](ui-surfaces.md) |
| 写主题或皮肤 | [写主题和外观](themes.md) |
| 写一个新的头 | [写一个头](head.md) |
| 写通讯平台的插件 | `miyu-im` 仓库的 [给接入通讯平台写插件](https://github.com/miyu-agent/miyu-im/blob/main/docs/authors/README.md) |
| 用 Python、Rust 的开发包写扩展 | [miyu-agent/miyu-sdk](https://github.com/miyu-agent/miyu-sdk) |
| 查某个协议方法的参数、回应、出错 | [协议参考](../reference/README.md) |
| 弄懂 Miyu 由哪些进程、crate、仓库组成 | [架构](architecture.md) |
| 给核心或某个头提改动 | [贡献代码](contributing.md) |

协议参考和作者教程各有中文、英文两份，中文是正本。`reference/`、`authors/`、`dev/` 和用户手册 `guide/` 用 Apache-2.0（仓库根目录 `LICENSES/Apache-2.0.txt`），照着写的东西用什么许可证都行；只经协议和 Miyu 说话的软件包、扩展、头是独立的作品，核心的 AGPL 管不到它们。
