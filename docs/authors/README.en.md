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

**Without a package**: put skills, scripts and command templates straight into `~/.miyu/root/skills/`, `scripts/`, `commands/`, and MCP servers into the config. They work the same; they just aren't managed as a package.

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

1. **Id** (the folder name): starts with a lowercase letter; only lowercase letters, digits, `-`, `_`; at most 64 characters. A short name that says what it is will do: no `miyu-`, and no kind of package (`scripts-`, `skills-` and the like). It can't clash with packages shipped with Miyu (they are all `base-<name>`, such as `base-read` and `base-memory`, so don't start yours with `base-`), and shouldn't clash with the official packages (`tui`, `webui`, `embed`, `im` and so on) or other people's extensions (installing the same id again is an upgrade and replaces theirs).
2. **Names it brings must not clash either**: tools, skills, MCP servers and slash commands that clash with an installed package won't install. Prefixing names with the package id helps.
3. **Don't write into the package folder after installing**: every file's hash is recorded at install time, and later additions show up in `miyu pkg check`. Write runtime state into the state directory `MIYU_STATE_DIR` names, and configuration people edit into the config directory `MIYU_CONFIG_DIR` names (the other folders are listed on the extensions page).
4. **Don't keep people's data in the state directory**: removing the package deletes the whole state directory, and the screen before removing says `Will delete <path> (<size>)`. Put what belongs to the person (exported records, data carried over from elsewhere, files made for them to read) in `MIYU_DOCUMENTS_DIR` (`root/documents/`), `MIYU_PICTURES_DIR` and the like, where people can see it and removing the package leaves it alone; keep only what your package uses itself (databases, logins) in the state directory; it goes away with the package.
5. **Put what people edit in the config directory**: `MIYU_CONFIG_DIR` (`etc/<id>/`) is kept when the package is removed, as pacman does; the screen before removing says `Config kept in <path>`, and a reinstall of the same id reads it again. Only `miyu pkg remove --purge` deletes it too. The settings your manifest declares (`[settings.<name>]`, written in `etc/config.toml`) work the same way: kept on removal without taking effect, `miyu check` only notes that your package left them, a reinstall uses them as they were, and only `--purge` deletes them. So: settings files people edit go in the config directory and stay on removal; your own state goes in the state directory and goes with the package. Don't write into the config directory what should leave with the package.

## Install, try, check

1. **Install**: `miyu pkg install ./hello`. It first prints what will happen (the program, commands it adds, the capabilities it wants with a line on each, where it comes from, size) and asks `[Y/n]`: yes installs it and approves those capabilities; `--yes` skips the question and approves just the same. It takes effect immediately, no core restart; one that always runs (`start = "always"`) starts right away.
2. **Turn on**: for a manual one it then asks "Turn it on now?"; yes turns it on, without asking about capabilities again (they were approved while installing); no leaves that for later, in the Packages page of the terminal or web UI. Packages with only skills and scripts have no switch; each skill and script is switched in presets.
3. **Change and reinstall**: while developing, run `miyu pkg install ./hello` again after a change (it's an upgrade). To try things in a separate data directory without touching your everyday one: `MIYU_HOME=/tmp/miyu-try miyu …`.
4. **Look**: `miyu pkg info hello` (name, version, size, install time), `miyu pkg files hello` (installed files), `miyu pkg check hello` (whether installed files changed), `miyu check` (mistakes in manifests, scripts, skills). The Packages page shows whether an extension is running, waiting to restart, or stopped (with the reason and the last lines of stderr).
5. **Logs**: an extension's stderr is in `~/.miyu/var/log/<id>/stderr`; the core's log is `~/.miyu/var/log/core.log`.
   - **Keeping the core running**: once started, the core no longer quits when idle (step Z-1), so extensions stay up with it and chat-platform bridges and scheduled work don't wait for someone to open `miyu`. To start it at boot or login, install a service as described in `packaging/README.md` in the repository (a systemd user service on Linux, launchd on macOS, a scheduled task on Windows). Design: `docs/designs/12-进程形态与分发.md`, section 2.
6. **Remove**: `miyu pkg remove hello`. The package folder, its state directory and its settings are removed together; entries in the secrets file stay (something else may use them).

## Publishing to the package source

1. **Pack**: `miyu pkg pack ./hello` checks the manifest the way installing does and writes `hello-<version>.miyu`, printing its SHA-256 and size. If the manifest lists `targets`, pack once per system: `--target linux-x86_64`. Version control and caches are left out at any depth: `.git`, `.github`, `.hg`, `.svn`, `__pycache__`, `.DS_Store`, `*.pyc`; everything else goes in, so move anything you want kept out before packing.
2. **Publish**: keep the package in its own GitHub repository and use the official template `miyu-agent/packages/.github/workflows/pack.yml`. Pushing a tag (`v` followed by the manifest's `version`) packs it and uploads it to that tag's release. See [miyu-agent/packages](https://github.com/miyu-agent/packages).
3. **Collection**: add the topic `miyu-package` to your repository. The package source collects each repository's latest release once a day and publishes a signed index, so your package shows up after the next collection, marked as community. Package ids are first come, first served: the first repository to publish an id keeps it for good, and other repositories using that id are skipped. Forks and archived repositories are not collected.

## Publishing through a system package manager

You can also ship through a system package manager (AUR, Homebrew and the like); Miyu's own terminal and web interfaces ship this way (the `miyu-tui` and `miyu-webui` packages). Miyu adds no review of its own: each package manager reviews its packages, and the core only reads and checks what is installed, by the usual rules.

1. **Where**: install the package folder as it is into the place Miyu reads as "system-installed", so that it becomes `<that place>/<id>/package.toml`:
   - Linux: `miyu/packages/` under each entry of `$XDG_DATA_DIRS` (`/usr/local/share` and `/usr/share` when unset, earlier entries first), so a system package goes into `/usr/share/miyu/packages/<id>/`. Linuxbrew installs under its own `share`, which users add to `XDG_DATA_DIRS`.
   - macOS: `share/miyu/packages/` under the Homebrew prefix (the folder above the one `miyu` is in).
   - Windows: there is no such place; publish through the package source only.
2. **The folder**: the same folder `miyu pkg pack` puts into a `.miyu` file, with the manifest unchanged and the programs in the folder's `bin/` with their executable bit. Don't put it in the core's own folder (`/usr/lib/miyu/`, Homebrew's `libexec/`): that is the core's built-in layer, owned by `miyu-core`.
3. **Name and dependencies**: call the package `miyu-<id>`. Depend on `miyu-core` (the core package; `miyu-agent` is the bundle, don't depend on it), with the first core version that speaks your manifest's `protocol` as the lower bound (the protocol is only `1` for now and every released core speaks it, so no bound is needed yet). If you depend on other Miyu packages, list them both in the manifest's `depends` and in the system package's dependencies.
4. **After installing**: the core reads the folder when it starts (restart a running core to pick it up). `miyu pkg list` shows the package in the `distro` layer. A manifest whose protocol, systems or dependencies don't fit is not loaded, and `miyu check` says why; a mistake breaks only that package.
5. **Both channels**: the same package can be in the package source too. What a user installs with `miyu pkg install` goes into the data root and covers the system copy. When the system copy is newer, `miyu check` warns `shadowed` and says how to switch back.
6. **Removing**: `miyu pkg remove <id>` on a system package only records it and stops loading it. The files stay; removing them is the package manager's job. `miyu pkg install <id>` brings a recorded one back.
7. **Try it first**: set `MIYU_DISTRO_PACKAGES` to a folder and the core uses only that folder as the system-installed layer. With a scratch data root in `MIYU_HOME`, `miyu pkg list` should show the `distro` layer, and `miyu pkg check <id>` shows whether its programs are found.

An AUR `PKGBUILD` looks roughly like this (adapt it to your package):

```sh
pkgname=miyu-hello
depends=(miyu-core)
package() {
  install -d "$pkgdir/usr/share/miyu/packages"
  cp -a hello "$pkgdir/usr/share/miyu/packages/hello"
}
```

A Homebrew formula:

```ruby
depends_on "miyu-core"

def install
  (share/"miyu/packages/hello").install Dir["hello/*"]
end
```

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
