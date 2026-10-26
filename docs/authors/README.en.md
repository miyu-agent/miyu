# Writing a Miyu package

For third-party authors. Chinese version: [README.md](README.md). The guides, templates and examples in this directory are under Apache-2.0 (`LICENSES/Apache-2.0.txt` at the repository root), so what you write from them can use any license; packages and extensions that only talk to Miyu over its protocol are separate works and the core's AGPL does not reach them. Every protocol method is in the [protocol reference](../reference/README.en.md).

## What a package can bring

**A folder is a package**: the folder name is its id, and the manifest is `package.toml` inside it. A package can bring any mix of:

| You want to | Bring | Read |
|---|---|---|
| Hook up an existing MCP server | `[[mcp]]` | [MCP servers](mcp.en.md) |
| Give the AI a small tool; a short script is enough | a script in `scripts/` | [Scripts, skills, command templates](scripts-skills.en.md) |
| Teach the AI a way of doing something | a skill in `skills/` | same |
| A slash command that expands into a message to the AI | a template in `commands/` | same |
| A long-running program: a set of tools, a chat platform bridge, content in the UI, your own slash commands | an extension (`[process]`) | [Writing an extension](extensions.en.md) |
| A settings page in the web UI | a package page (`[page]`) | [Writing an extension](extensions.en.md) |

Every manifest field is in [The manifest](manifest.en.md). For extensions in Python or Rust there are ready-made development kits ([Using a development kit](extensions.en.md#using-a-development-kit)).

**Without a package**: put skills, scripts and command templates straight into `~/.miyu/home/admin/skills/`, `scripts/`, `commands/`, and MCP servers into personal settings. They work the same; they just aren't managed as a package.

## What a package looks like

```text
hello/
├── package.toml          # the manifest
├── bin/
│   └── miyu-hello        # the extension's program: a compiled program (miyu-hello.exe on Windows) or a script with #!
├── scripts/
│   └── dice.py           # a script
├── skills/
│   └── take-notes/
│       └── SKILL.md      # a skill
└── page/
    └── index.html        # a package page
```

1. **Id** (the folder name): starts with a lowercase letter; only lowercase letters, digits, `-`, `_`; at most 64 characters. It can't clash with packages shipped with Miyu (`basesystem`, `memory`, `roleplay`, `mermaid`, `net`, `tui`, `web` and so on), and shouldn't clash with other people's extensions (installing the same id again is an upgrade and replaces theirs). Package ids and usernames share one set of names (a package with a process uses its id as its account name), so a package whose id matches an existing account won't install (`package_id_taken`).
2. **Names it brings must not clash either**: tools, skills, MCP servers and slash commands that clash with an installed package won't install. Prefixing names with the package id helps.
3. **Don't write into the package folder after installing**: every file's hash is recorded at install time, and later additions show up in `miyu pkg check`. Write runtime state into the state directory `MIYU_PACKAGE_STATE` names.

## Install, try, check

1. **Install**: `miyu pkg install ./hello`. It first prints what will happen (the program, commands it adds, capabilities it wants, size) and asks `[Y/n]`; `--yes` skips the question. It takes effect immediately, no core restart.
2. **Turn on**: after installing it asks "Turn it on now?"; yes approves the capabilities it asks for and turns it on; no leaves that for later, in the Packages page of the terminal or web UI. Packages with only skills and scripts have no switch; each skill and script is switched in presets.
3. **Change and reinstall**: while developing, run `miyu pkg install ./hello` again after a change (it's an upgrade). To try things in a separate data directory without touching your everyday one: `MIYU_HOME=/tmp/miyu-try miyu …`.
4. **Look**: `miyu pkg info hello` (name, version, size, install time), `miyu pkg files hello` (installed files), `miyu pkg check hello` (whether installed files changed), `miyu check` (mistakes in manifests, scripts, skills). The Packages page shows whether an extension is running, waiting to restart, or stopped (with the reason and the last lines of stderr).
5. **Logs**: an extension's stderr is in `~/.miyu/state/logs/<id>.stderr`; the core's log is `~/.miyu/state/logs/core.log`.
   - **Keeping the core running**: once started, the core no longer quits when idle (step Z-1), so extensions stay up with it and chat-platform bridges and scheduled work don't wait for someone to open `miyu`. To start it at boot or login, install a service as described in `packaging/README.md` in the repository (a systemd user service on Linux, launchd on macOS, a scheduled task on Windows). Design: `docs/designs/12-进程形态与分发.md`, section 2.
6. **Remove**: `miyu pkg remove hello`. The package folder, its state directory and its settings are removed together; entries in the secrets file stay (something else may use them).

## Publishing to the package source

1. **Pack**: `miyu pkg pack ./hello` checks the manifest the way installing does and writes `hello-<version>.miyu`, printing its SHA-256 and size. If the manifest lists `targets`, pack once per system: `--target linux-x86_64`. Version control and caches are left out at any depth: `.git`, `.github`, `.hg`, `.svn`, `__pycache__`, `.DS_Store`, `*.pyc`; everything else goes in, so move anything you want kept out before packing.
2. **Publish**: keep the package in its own GitHub repository and use the official template `miyu-agent/packages/.github/workflows/pack.yml`. Pushing a tag (`v` followed by the manifest's `version`) packs it and uploads it to that tag's release. See [miyu-agent/packages](https://github.com/miyu-agent/packages).
3. **Collection**: the package source collects each repository's latest release once a day and publishes a signed index. Only official repositories are collected for now. Automatic collection of community packages comes later.

## Three platforms

1. The extension's program is named in the manifest. `bin/<name>` can be a compiled program or a script whose first line is `#!` (like `#!/usr/bin/env python3`). On Windows the core looks for `<name>.exe` first, and otherwise finds the interpreter from the `#!` line of `bin/<name>` (`python3` means `python`; an absolute path that exists on this machine is used as is), so one file and one manifest work on all three platforms. Miyu doesn't ship interpreters: say which one your package needs in its description, and people install it themselves. Script tools without a `#!` line find their interpreter by extension on Windows (`.py` uses `python`, `.ps1` PowerShell, `.js` `node`).
2. On Linux and macOS, programs and scripts need the executable bit and a `#!` line; installing copies them as they are, permissions included.
3. macOS (Apple silicon only): programs downloaded from the internet are quarantined, and Gatekeeper blocks unsigned, unnotarized ones. Sign and notarize programs you distribute.
4. On every platform, "please exit" is stdin reaching its end; don't rely on signals.

## Text for the model

Tool and skill descriptions, parameter descriptions, and anything you put into her context are read by the model and sent with every request, so shorter is cheaper:

- Write short English sentences. At most three per tool: what it is, when to use it, and the one pitfall people hit most.
- Knowledge needed only after a call (what fields mean, how to page) goes in the tool's **output**, not its description.
- Names and descriptions for people have their own fields (`display_name`, `summary`, per language); keep them out of the model's text.
