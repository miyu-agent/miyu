<p align="center">
  <img src="docs/assets/miyu-logo.png" alt="Miyu" width="180">
</p>

<p align="center">
  <a href="https://github.com/miyu-agent/miyu/stargazers"><img src="https://img.shields.io/github/stars/miyu-agent/miyu?style=flat&logo=github&label=Stars" alt="GitHub stars"></a>
  <a href="#"><img src="https://img.shields.io/badge/-%E5%AE%98%E7%BD%91-8a63d2?style=flat&logo=homeassistantcommunitystore&logoColor=white" alt="官网"></a>
  <a href="https://space.bilibili.com/9202840"><img src="https://img.shields.io/badge/-bilibili-00A1D6?style=flat&logo=bilibili&logoColor=white" alt="bilibili"></a>
  <a href="https://github.com/SHORiN-KiWATA/.github/blob/main/SPONSOR.md"><img src="https://img.shields.io/badge/-%E8%B5%9E%E5%8A%A9-ea4aaa?style=flat&logo=githubsponsors&logoColor=white" alt="赞助"></a>
</p>

🚧待施工...

## 许可证

核心（这个仓库自己写的代码和资源）用 AGPL-3.0-or-later，见 `LICENSE`：改过的 Miyu 拿去给别人用，包括只在网上提供服务，要把改过的源码交给用它的人。

给别人照着写东西用的这几处用 Apache-2.0（`LICENSES/Apache-2.0.txt`）：`docs/authors/`（写软件包、扩展、脚本、技能、MCP 服务的说明、模板和例子）、`docs/reference/`（协议参考）、`docs/guide/`（用户手册）、`docs/dev/`（开发者文档）、`resources/packages/base-authoring/`（教 AI 写软件包、技能的那两份技能）。开发包 `miyu-sdk` 也用 Apache-2.0。

只经协议和 Miyu 说话的程序是独立的作品：扩展、软件包、脚本、MCP 服务器、另写的终端或网页，用什么许可证都行，AGPL 管不到它们。

不是这个仓库自己写的东西照它们原来的许可证，AGPL 和 Apache 只管这里自己写的部分：Rust 依赖照各自的 `license`；仓库里收的第三方数据（模型目录、分词词表这些）清单和各自的许可证见 `docs/blueprint/licenses.md`「资源里的第三方数据」；发行包里带的字体、ONNX Runtime 这些照各自的许可证，随包附上原文。

供应商、模型的名称和图标是各自所有者的商标，归各自所有者。Miyu 只在运行时从 models.dev、lobe-icons 取来缓存在本机，不随包发；用到它们不表示这些所有者认可或赞助 Miyu。

旧版 Miyu（`legacy-0.7` 分支，`v0.7.x` 及更早的标签和发布）沿用原来的 MIT 许可证，见那个分支里的 `LICENSE`；随旧版附带的字体照各自的许可证（OFL-1.1）。

## Star 趋势

<a href="https://star-history.com/#miyu-agent/miyu&Date">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://api.star-history.com/svg?repos=miyu-agent/miyu&type=Date&theme=dark">
    <img alt="Star 趋势" src="https://api.star-history.com/svg?repos=miyu-agent/miyu&type=Date">
  </picture>
</a>
