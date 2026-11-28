#!/bin/sh
# Miyu 的安装脚本，Linux、macOS 用（施工 RL-3，设计 59 第三节第 2 条、第四节）。Windows 用 install.ps1。
#
#   curl -fsSL https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.sh | sh
#   curl -fsSL …/install.sh | sh -s -- --prerelease      # 也算预发布
#   curl -fsSL …/install.sh | sh -s -- --version 2026.11.0   # 装指定的一版
#   curl -fsSL …/install.sh | sh -s -- --uninstall       # 卸
#
# 从 GitHub Releases 下载这一种系统的压缩包，先用写在这里的发版公钥核 SHA256SUMS 的签名（openssl 认 Ed25519 的才核得了），
# 再核压缩包的 SHA-256，解开后跑一次新的 `miyu --version`。都对了才装：
#   - 压缩包第一层整个放进 ~/.local/lib/miyu/，旁边写 install.toml（channel = "script"，`miyu upgrade` 照它自己升级）；
#   - ~/.local/bin/miyu 链过去；PATH 里没有 ~/.local/bin 的只说怎么加，不改 shell 的配置；
#   - Linux 上装 .miyu 包文件的类型（~/.local/share/mime/packages/miyu.xml）。
# 不要管理员、不用 sudo。数据根（~/.miyu）装、卸都不碰。
#
# 挑哪一版照发行版订阅（releases.atom，施工 UP-5，同 `miyu upgrade` 的 UP-4）：它是普通网页，不算 GitHub API 的匿名额度，
# 共用出口的机器也不会因为限流装不上。不问 GitHub API。
# 测试用的两个环境变量：MIYU_INSTALL_FEED（订阅的地址）、MIYU_INSTALL_BASE（下载地址的前半）。
set -eu

REPO="miyu-agent/miyu"
FEED="${MIYU_INSTALL_FEED:-https://github.com/${REPO}/releases.atom}"
BASE="${MIYU_INSTALL_BASE:-https://github.com/${REPO}/releases/download}"
LIB="${HOME}/.local/lib/miyu"
BIN="${HOME}/.local/bin"
MIME="${XDG_DATA_HOME:-${HOME}/.local/share}/mime"

# 发版公钥：编号和 32 字节的 Ed25519 公钥（base64），同 crates/miyu-source/src/sign.rs 的 RELEASE（测试守着两边一样）。
KEY_ID="ca6918041857843c"
KEY="LOj3CWiaD/hcIp5yRUzy6zAGKivZDP5Dl+5O51kupYs="

# .miyu 包文件的类型，同 packaging/mime/miyu.xml（测试守着两边一样）。
MIME_XML='<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="application/x-zstd-compressed-tar">
    <glob pattern="*.miyu" weight="80"/>
  </mime-type>
</mime-info>'

case "${LC_ALL:-${LC_MESSAGES:-${LANG:-}}}" in
  zh*) ZH=1 ;;
  *) ZH=0 ;;
esac

# say <中文> <英文>：照语言挑一句印出来。
say() {
  if [ "${ZH}" = 1 ]; then printf '%s\n' "$1"; else printf '%s\n' "$2"; fi
}

# die <中文> <英文>：印到标准错误，退出码 1。
die() {
  if [ "${ZH}" = 1 ]; then printf 'miyu 安装：%s\n' "$1" >&2; else printf 'miyu install: %s\n' "$2" >&2; fi
  exit 1
}

usage() {
  say "用法：install.sh [--prerelease] [--version <版本>] [--uninstall]" \
      "Usage: install.sh [--prerelease] [--version <version>] [--uninstall]"
}

WANT=""
PRE=0
UNINSTALL=0
while [ $# -gt 0 ]; do
  case "$1" in
    --prerelease) PRE=1 ;;
    --uninstall) UNINSTALL=1 ;;
    --version)
      [ $# -ge 2 ] || die "--version 后面需要写明版本" "--version needs a version"
      WANT="$2"
      shift
      ;;
    --version=*) WANT="${1#--version=}" ;;
    -h | --help) usage; exit 0 ;;
    *) usage >&2; exit 1 ;;
  esac
  shift
done
WANT="${WANT#v}"

# 这台是哪一种：同 `cargo xtask dist` 的名字。
target() {
  os=$(uname -s)
  arch=$(uname -m)
  case "${os}-${arch}" in
    Linux-x86_64 | Linux-amd64) echo "linux-x86_64" ;;
    Linux-aarch64 | Linux-arm64) echo "linux-aarch64" ;;
    Darwin-arm64 | Darwin-aarch64) macos; echo "macos-aarch64" ;;
    *) die "没有 ${os} ${arch} 的发行包（目前有 Linux x86_64、aarch64 和 macOS Apple 芯片的发行包）" \
           "no release for ${os} ${arch} (there are Linux x86_64 and aarch64, and macOS on Apple silicon)" ;;
  esac
}

# macOS 的发行包照最低系统 12.0 编（施工 MO-6，xtask/src/dist.rs 的 MACOS_MINIMUM）：更老的系统上程序起不来，不装。版本读不出来
# 的不拦，后面照样跑一次新的 miyu --version。
macos() {
  release=$(sw_vers -productVersion 2>/dev/null || true)
  case "${release%%.*}" in
    '' | *[!0-9]*) return 0 ;;
  esac
  [ "${release%%.*}" -ge 12 ] \
    || die "需要 macOS 12 或更新的版本，这台是 macOS ${release}" "needs macOS 12 or later; this Mac runs macOS ${release}"
}

sha256() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d ' ' -f 1
  else
    shasum -a 256 "$1" | cut -d ' ' -f 1
  fi
}

# 卸：先停服务、停核心，再删安装目录、链接、文件类型。数据根不碰。
uninstall() {
  if [ ! -e "${LIB}" ]; then
    say "未安装（${LIB} 不存在）" "Not installed (${LIB} doesn't exist)"
    exit 0
  fi
  grep -q '^channel = "script"' "${LIB}/install.toml" 2>/dev/null \
    || die "${LIB} 不是这个脚本安装的（没有 install.toml），没有修改它" \
           "${LIB} wasn't installed by this script (no install.toml); leaving it alone"
  if [ -x "${LIB}/miyu" ]; then
    "${LIB}/miyu" service disable >/dev/null 2>&1 \
      || say "无法移除开机启动的服务，继续卸载（卸载以后不能再运行 miyu service disable，需要按 packaging/README.md 手动删除）" \
             "Couldn't remove the startup service; going on (remove it by hand as in packaging/README.md)"
    "${LIB}/miyu" core stop >/dev/null 2>&1 \
      || say "无法停止核心，继续卸载" "Couldn't stop the core; going on"
  fi
  rm -rf "${LIB}"
  if [ -L "${BIN}/miyu" ] && [ "$(readlink "${BIN}/miyu")" = "${LIB}/miyu" ]; then
    rm -f "${BIN}/miyu"
  fi
  if [ -f "${MIME}/packages/miyu.xml" ]; then
    rm -f "${MIME}/packages/miyu.xml"
    if command -v update-mime-database >/dev/null 2>&1; then
      update-mime-database "${MIME}" >/dev/null 2>&1 || true
    fi
  fi
  say "已卸载 Miyu。数据仍然保留在 ${MIYU_HOME:-${HOME}/.miyu}，如果不再需要，可以自行删除。" \
      "Miyu is uninstalled. Your data is still in ${MIYU_HOME:-${HOME}/.miyu}; delete it yourself if you don't need it."
}

# 标准输入是订阅，印出每个 entry 的标签，照订阅的先后，一行一个（同 crates/miyu-cli/src/upgrade/feed.rs）：只认每个 <entry>
# 里第一个 href 带 /releases/tag/ 的 <link>。正文 <content> 是转义过的网页，里面的 <、" 写成了 &lt;、&quot;，不会被认成链接。
# 没有 <feed、没有 </feed> 的（网页、空的、读到一半的）不是订阅，退出码 3。不用 jq、不用 XML 的工具：mawk、macOS 的 awk 都认。
feed_tags() {
  awk '
    { text = text $0 "\n" }
    END {
      if (index(text, "<feed") == 0 || index(text, "</feed>") == 0) exit 3
      rest = text
      while ((at = index(rest, "<entry")) > 0) {
        after = substr(rest, at + 6, 1)
        rest = substr(rest, at + 6)
        # <entries 这样的别的元素不算。
        if (after != ">" && after != " " && after != "\t" && after != "\r" && after != "\n") continue
        stop = index(rest, "</entry>")
        if (stop == 0) stop = length(rest) + 1
        entry = substr(rest, 1, stop - 1)
        rest = substr(rest, stop)
        while ((at = index(entry, "<link")) > 0) {
          entry = substr(entry, at)
          gt = index(entry, ">")
          if (gt == 0) gt = length(entry) + 1
          link = substr(entry, 1, gt - 1)
          entry = substr(entry, gt)
          start = index(link, "href=\"")
          if (start == 0) continue
          href = substr(link, start + 6)
          quote = index(href, "\"")
          if (quote == 0) continue
          href = substr(href, 1, quote - 1)
          start = index(href, "/releases/tag/")
          if (start == 0) continue
          tag = substr(href, start + 14)
          if (tag != "" && index(tag, "/") == 0) { print tag; break }
        }
      }
    }
  '
}

# 标准输入是标签，印出能算的，新的在前，一行一个（同 crates/miyu-cli/src/upgrade/plan.rs 的 candidates）。$1 是 stable（只要
# 正式版）或 any（预发布也算）。
#   - 只认现在的版本号 v<年>.<月>.<第几版>，可以接 -<预发布>（设计 59）：仓库里还留着旧版 Miyu 的发行版（v0.7.x）和早先演练的
#     v0.0.0，它们的包不是这个脚本装得了的，一律跳过（施工 WD-8）。
#   - 订阅里没有「预发布」的标记，照标签认：带 - 的（v2026.11.0-rc.1）是预发布。
#   - 照版本号排（semver）：数字逐段比；带预发布的排在同号正式版前面；预发布逐段比，都是数字的照数字（rc.10 在 rc.9 后面），
#     数字排在字前面，前面都一样的段少的在前。办法是给每个标签拼一个按字节排就对的键，再 sort。同一个版本只算一次。
candidates() {
  awk -v want="$1" '
    function pad(digits) {
      while (length(digits) < 20) digits = "0" digits
      return digits
    }
    /^v[0-9][0-9][0-9][0-9]\.[0-9]+\.[0-9]+(-[0-9A-Za-z]+(\.[0-9A-Za-z]+)*)?$/ {
      version = substr($0, 2)
      dash = index(version, "-")
      if (dash > 0) {
        core = substr(version, 1, dash - 1)
        pre = substr(version, dash + 1)
      } else {
        core = version
        pre = ""
      }
      if (pre != "" && want != "any") next
      split(core, number, ".")
      key = pad(number[1]) "." pad(number[2]) "." pad(number[3])
      if (pre == "") {
        # 正式版的 ~ 排在预发布的 - 后面。
        key = key "~"
      } else {
        key = key "-"
        count = split(pre, field, ".")
        # 每段后面跟一个 !（比数字、字母都小），段少的就排在前面。
        for (i = 1; i <= count; i++) {
          if (field[i] ~ /^[0-9]+$/) key = key "0" pad(field[i]) "!"
          else key = key "1" field[i] "!"
        }
      }
      print key " " $0
    }
  ' | LC_ALL=C sort -r | awk '$1 != last { print $2 } { last = $1 }'
}

# 挑一个能装的发布，印出它的标签，它的 SHA256SUMS 留在 ${WORK}/SHA256SUMS；都不成的什么都不印。$1 同 candidates。
# 同 miyu upgrade（crates/miyu-cli/src/upgrade/flow.rs 的 usable，施工 UP-4）：读订阅，下载地址照标签拼，从新到旧一个一个试，
# SHA256SUMS 回 404 的（作废的 rc.3 那样：订阅里有、没传包）跳过、接着试更旧的；回别的错的停下。
find_release() {
  code=$(curl -sS -L -o "${WORK}/releases.atom" -w '%{http_code}' "${FEED}") \
    || die "无法连接 GitHub（${FEED}）" "can't reach GitHub (${FEED})"
  [ "${code}" = 200 ] || die "GitHub 返回 HTTP ${code}（${FEED}）" "GitHub answered ${code} (${FEED})"
  feed_tags < "${WORK}/releases.atom" > "${WORK}/tags" \
    || die "无法读取发行版列表：${FEED} 返回的不是 Atom 订阅，检查网络或代理后重试" \
           "can't read the release list: ${FEED} isn't an Atom feed; check your network or proxy and try again"
  candidates "$1" < "${WORK}/tags" > "${WORK}/candidates"
  while read -r tag; do
    url="${BASE}/${tag}/SHA256SUMS"
    code=$(curl -sS -L -o "${WORK}/SHA256SUMS" -w '%{http_code}' "${url}" < /dev/null) \
      || die "无法连接 GitHub（${url}）" "can't reach GitHub (${url})"
    case "${code}" in
      200) printf '%s\n' "${tag}"; return 0 ;;
      404) ;;
      *) die "GitHub 返回 HTTP ${code}（${url}）" "GitHub answered ${code} (${url})" ;;
    esac
  done < "${WORK}/candidates"
  rm -f "${WORK}/SHA256SUMS"
}

# 下载 $1 到 $2；不成的停。
fetch() {
  curl -fsSL -o "$2" "$1" || die "下载失败：$1" "download failed: $1"
}

# 照写在这里的公钥核 SHA256SUMS 的签名。核得了却核不过的停；这台的 openssl 不认 Ed25519 的说一句、接着装。
verify_signature() {
  sums="$1"
  sig="$2"
  key_of_sig=$(tr ',' '\n' < "${sig}" | sed -n 's/.*"key"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
  [ "${key_of_sig}" = "${KEY_ID}" ] \
    || die "SHA256SUMS 的签名来自其他密钥（${key_of_sig}），不安装" "SHA256SUMS is signed by another key (${key_of_sig}); not installing"
  if ! command -v openssl >/dev/null 2>&1; then
    say "这台机器没有 openssl，无法核对签名，只核对 SHA-256。如果要核对签名，运行：gh attestation verify <文件> -R ${REPO}" \
        "No openssl here, so the signature can't be checked; checking SHA-256 only. To check it: gh attestation verify <file> -R ${REPO}"
    return 0
  fi
  pem="${WORK}/release.pem"
  {
    echo "-----BEGIN PUBLIC KEY-----"
    # Ed25519 公钥的 DER 前缀（RFC 8410）加上 32 字节的公钥。
    { printf '\060\052\060\005\006\003\053\145\160\003\041\000'; printf '%s' "${KEY}" | openssl base64 -d -A; } | openssl base64 -A
    echo
    echo "-----END PUBLIC KEY-----"
  } > "${pem}"
  tr ',' '\n' < "${sig}" | sed -n 's/.*"signature"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1 \
    | openssl base64 -d -A > "${WORK}/sums.sig"
  # 认不认 Ed25519：读得了这把公钥、pkeyutl 有 -rawin（OpenSSL 3 起）。macOS 自带的 LibreSSL 两样都不行。
  if ! openssl pkey -pubin -in "${pem}" -noout >/dev/null 2>&1 \
    || ! openssl pkeyutl -help 2>&1 | grep -q -- '-rawin'; then
    say "这台机器的 openssl 不支持 Ed25519，没有核对签名（不是签名有问题），只核对 SHA-256。如果要核对签名，运行：gh attestation verify <文件> -R ${REPO}" \
        "This machine's openssl doesn't support Ed25519, so the signature wasn't checked (it isn't broken); checking SHA-256 only. To check it: gh attestation verify <file> -R ${REPO}"
    return 0
  fi
  openssl pkeyutl -verify -pubin -inkey "${pem}" -rawin -in "${sums}" -sigfile "${WORK}/sums.sig" >/dev/null 2>&1 \
    || die "SHA256SUMS 的签名核对失败，不安装" "the signature on SHA256SUMS doesn't match; not installing"
  say "签名核对通过（发版公钥 ${KEY_ID}）" "Signature checked (release key ${KEY_ID})"
}

install() {
  command -v curl >/dev/null 2>&1 || die "需要 curl" "curl is needed"
  command -v tar >/dev/null 2>&1 || die "需要 tar" "tar is needed"
  t=$(target) || exit 1
  if [ -e "${LIB}/miyu" ]; then
    now=$("${LIB}/miyu" --version 2>/dev/null || echo miyu)
    say "已安装 ${now}（${LIB}）；如果要升级，运行 miyu upgrade" "${now} is already installed (${LIB}); use miyu upgrade to upgrade"
    exit 0
  fi
  [ ! -e "${LIB}" ] || die "${LIB} 已存在，但里面没有 miyu，需要先把它移走" "${LIB} exists but has no miyu in it; move it away first"
  if [ -e "${BIN}/miyu" ] || [ -L "${BIN}/miyu" ]; then
    [ -L "${BIN}/miyu" ] && [ "$(readlink "${BIN}/miyu")" = "${LIB}/miyu" ] \
      || die "${BIN}/miyu 已存在（不是这个脚本创建的），需要先把它移走" "${BIN}/miyu already exists (not placed by this script); move it away first"
  fi

  mkdir -p "${HOME}/.local/lib"
  # 暂存放在 ~/.local/lib 下：和装的地方同一块盘，最后一步只是改名。
  WORK=$(mktemp -d "${HOME}/.local/lib/.miyu-install.XXXXXX")
  trap 'rm -rf "${WORK}"' EXIT INT TERM

  if [ -n "${WANT}" ]; then
    # 指定了版本的不读订阅。
    tag="v${WANT}"
    fetch "${BASE}/${tag}/SHA256SUMS" "${WORK}/SHA256SUMS"
  elif [ "${PRE}" = 1 ]; then
    tag=$(find_release any) || exit 1
    [ -n "${tag}" ] || die "未找到可用的发行版" "no release found"
  else
    tag=$(find_release stable) || exit 1
    if [ -z "${tag}" ]; then
      # 订阅里有预发布的提示加 --prerelease（同 plan::has_pre）。
      if candidates any < "${WORK}/tags" | grep -q -- '-'; then
        die "还没有正式版；如果要安装预发布版，加上 --prerelease" "no stable release yet; add --prerelease to install a pre-release"
      fi
      die "未找到可用的发行版" "no release found"
    fi
  fi
  version="${tag#v}"
  name="miyu-${version}-${t}"
  archive="${name}.tar.gz"

  say "正在下载 ${archive}" "Downloading ${archive}"
  fetch "${BASE}/${tag}/SHA256SUMS.sig" "${WORK}/SHA256SUMS.sig"
  verify_signature "${WORK}/SHA256SUMS" "${WORK}/SHA256SUMS.sig"
  want=$(awk -v f="${archive}" '$2 == f || $2 == "*" f { print $1 }' "${WORK}/SHA256SUMS")
  [ -n "${want}" ] || die "SHA256SUMS 里没有 ${archive}" "${archive} isn't listed in SHA256SUMS"
  fetch "${BASE}/${tag}/${archive}" "${WORK}/${archive}"
  got=$(sha256 "${WORK}/${archive}")
  [ "${got}" = "${want}" ] || die "${archive} 的 SHA-256 和清单不一致，不安装" "the SHA-256 of ${archive} doesn't match; not installing"

  mkdir "${WORK}/unpacked"
  tar -xzf "${WORK}/${archive}" -C "${WORK}/unpacked" || die "无法解压 ${archive}" "can't unpack ${archive}"
  top="${WORK}/unpacked/${name}"
  [ -x "${top}/miyu" ] && [ -d "${top}/resources" ] || die "${archive} 里缺少文件" "${archive} is missing files"
  reported=$("${top}/miyu" --version 2>/dev/null || true)
  [ "${reported}" = "miyu ${version}" ] \
    || die "新版 miyu --version 报告的是「${reported}」，应为 ${version}" "the new miyu --version says \"${reported}\", not ${version}"

  printf '# 安装脚本装的（施工 RL-3）：miyu upgrade 照它自己下载、核对、替换。\nchannel = "script"\n' > "${top}/install.toml"
  mv "${top}" "${LIB}"
  mkdir -p "${BIN}"
  ln -sf "${LIB}/miyu" "${BIN}/miyu"
  if [ "${t#linux}" != "${t}" ]; then
    mkdir -p "${MIME}/packages"
    printf '%s\n' "${MIME_XML}" > "${MIME}/packages/miyu.xml"
    if command -v update-mime-database >/dev/null 2>&1; then
      update-mime-database "${MIME}" >/dev/null 2>&1 || true
    fi
  fi

  say "已安装 Miyu ${version}（${LIB}）" "Installed Miyu ${version} (${LIB})"
  case ":${PATH}:" in
    *":${BIN}:"*) ;;
    *) say "提示：${BIN} 不在 PATH 中，需要在 shell 的配置文件里加一行：export PATH=\"${BIN}:\$PATH\"" \
           "Note: ${BIN} isn't on PATH; add this line to your shell's config: export PATH=\"${BIN}:\$PATH\"" ;;
  esac
  other=$(command -v miyu 2>/dev/null || true)
  if [ -n "${other}" ] && [ "${other}" != "${BIN}/miyu" ]; then
    say "提示：PATH 中先找到的是 ${other}，不是刚安装的这个版本" "Note: ${other} comes first on PATH, not the one just installed"
  fi
  say "下一步：运行 miyu 开始使用；如果要开机启动，运行 miyu service enable。升级运行 miyu upgrade，卸载运行这个脚本并加上 --uninstall。" \
      "Next: run miyu to start; run miyu service enable to start it at login. Upgrade with miyu upgrade; uninstall with this script and --uninstall."
}

if [ "${UNINSTALL}" = 1 ]; then
  uninstall
else
  install
fi
