#!/bin/sh
# install.sh 的实测（施工 RL-3）：在一个干净的家目录里装、核、卸一遍，资产用公开的预发布。CI 的 install-test.yml 和容器里都跑它，
# 别在自己的家目录跑（会装进 ~/.local、卸的时候停核心）。
#
#   sh packaging/test-install.sh <install.sh 的路径>
#
# 查的：macOS 11 上停下、说要 12 或更新的版本；没有正式版时不加 --prerelease 停下、退出码 1；--prerelease 装上，链接、install.toml 在，
# miyu --version 对得上，miyu upgrade --prerelease 说已是最新；再装说已经装了；有 python3 的另起一个本机的下载处和订阅（施工 UP-5）：
# 照版本号从新到旧试、SHA256SUMS 回 404 的跳过，正文里的链接不算，旧版的 v0.7.x 不试；没有正式版的提示加 --prerelease，一个都没有的
# 说没找到，订阅是网页的说读不了；SHA256SUMS 被改过的（openssl 认 Ed25519 时）、压缩包被改过的都停下、什么都不装；卸掉以后安装目录、
# 链接没了，数据根还在。
set -eu

SCRIPT="$1"
LANG=C
LC_ALL=C
export LANG LC_ALL

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  exit 1
}

# macOS 11 上停下、说要 12 或更新的版本，什么都不装（施工 MO-6）：PATH 前面放假的 uname、sw_vers，Linux 的机器上也走得到这一条。
fake=$(mktemp -d)
printf '#!/bin/sh\ncase "$1" in -s) echo Darwin ;; -m) echo arm64 ;; *) exit 1 ;; esac\n' > "${fake}/uname"
printf '#!/bin/sh\necho 11.7.10\n' > "${fake}/sw_vers"
chmod +x "${fake}/uname" "${fake}/sw_vers"
out=$(PATH="${fake}:${PATH}" sh "${SCRIPT}" --prerelease 2>&1) && fail "macOS 11 上装上了：${out}"
printf '%s\n' "${out}" | grep -q 'needs macOS 12 or later; this Mac runs macOS 11.7.10' || fail "macOS 11 上该说要 12：${out}"
[ ! -e "${HOME}/.local/lib/miyu" ] || fail "macOS 11 上不该留东西"
printf '#!/bin/sh\necho 12.0\n' > "${fake}/sw_vers"
# 12.0 不拦：停在后面哪一步都行，只是不能停在系统版本上。
out=$(PATH="${fake}:${PATH}" MIYU_INSTALL_FEED="http://127.0.0.1:9/releases.atom" sh "${SCRIPT}" --prerelease 2>&1) || true
printf '%s\n' "${out}" | grep -q 'needs macOS 12' && fail "macOS 12.0 上不该拦：${out}"
rm -rf "${fake}"
[ ! -e "${HOME}/.local/lib/miyu" ] || fail "停下的不该留东西"

# 正式版还没有的时候，不加 --prerelease 的停下。有了以后这一条照装上算过。
if out=$(sh "${SCRIPT}" 2>&1); then
  printf '%s\n' "${out}" | grep -q 'Installed Miyu' || fail "装上了却没说：${out}"
  sh "${SCRIPT}" --uninstall
else
  printf '%s\n' "${out}" | grep -q 'no stable release yet' || fail "没有正式版时该说加 --prerelease：${out}"
fi
[ ! -e "${HOME}/.local/lib/miyu" ] || fail "停下的不该留东西"

sh "${SCRIPT}" --prerelease
[ -L "${HOME}/.local/bin/miyu" ] || fail "没有链接"
grep -q '^channel = "script"' "${HOME}/.local/lib/miyu/install.toml" || fail "install.toml 不对"
version=$("${HOME}/.local/bin/miyu" --version)
printf 'installed: %s\n' "${version}"
for left in "${HOME}"/.local/lib/.miyu-install.*; do
  [ ! -e "${left}" ] || fail "暂存没删：${left}"
done
# 装上的是公开的预发布，它自己的 miyu upgrade 还在问 GitHub API（读订阅的 UP-4 在它以后，还没发版）：共用出口的 CI 机器上撞了
# 限流的，记一句没验成，别的照查。装上的预发布带了 UP-4 以后这一条连同 install.ps1 那边的一起撤掉。
if up=$("${HOME}/.local/bin/miyu" upgrade --prerelease 2>&1); then
  printf '%s\n' "${up}" | grep -q 'already up to date' || fail "装完 miyu upgrade 该说已是最新：${up}"
else
  printf '%s\n' "${up}" | grep -q 'rate limit' || fail "miyu upgrade --prerelease 没成：${up}"
  echo "WARNING: miyu upgrade hit GitHub's anonymous rate limit; the up-to-date check wasn't verified on this run"
fi
again=$(sh "${SCRIPT}" --prerelease 2>&1) || fail "再装不该报错：${again}"
printf '%s\n' "${again}" | grep -q 'already installed' || fail "再装该说已经装了：${again}"

mkdir -p "${HOME}/.miyu"
echo keep > "${HOME}/.miyu/keep"

if command -v python3 >/dev/null 2>&1; then
  tag="v${version#miyu }"
  case "$(uname -s)-$(uname -m)" in
    Linux-x86_64) t=linux-x86_64 ;;
    Linux-aarch64) t=linux-aarch64 ;;
    *) t=macos-aarch64 ;;
  esac
  archive="miyu-${version#miyu }-${t}.tar.gz"
  srv=$(mktemp -d)
  mkdir -p "${srv}/${tag}"
  for f in SHA256SUMS SHA256SUMS.sig "${archive}"; do
    curl -fsSL -o "${srv}/${tag}/${f}" "https://github.com/miyu-agent/miyu/releases/download/${tag}/${f}"
  done
  port=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1", 0)); print(s.getsockname()[1])')
  # 下载处每收到一个请求往标准错误记一行（"GET /<路径> HTTP/1.1" 404 这样），照它查试了哪几个、先后。
  log=$(mktemp)
  python3 -m http.server "${port}" --bind 127.0.0.1 --directory "${srv}" >/dev/null 2>"${log}" &
  server=$!
  sleep 1
  sh "${SCRIPT}" --uninstall >/dev/null
  here="http://127.0.0.1:${port}"

  # feed <标签>…：照 GitHub 的 releases.atom 的样子写一份订阅。每个 entry 的正文里都有一个转义过的、指向 v2099.2.0 的链接，
  # 不该被认成标签（认了就会去下载它的 SHA256SUMS，下载处的记录里查得到）。
  feed() {
    printf '<?xml version="1.0" encoding="UTF-8"?>\n<feed xmlns="http://www.w3.org/2005/Atom" xml:lang="en-US">\n'
    printf '  <link type="application/atom+xml" rel="self" href="https://github.com/miyu-agent/miyu/releases.atom"/>\n'
    for each in "$@"; do
      printf '  <entry>\n    <id>tag:github.com,2008:Repository/1/%s</id>\n' "${each}"
      printf '    <link rel="alternate" type="text/html" href="https://github.com/miyu-agent/miyu/releases/tag/%s"/>\n' "${each}"
      printf '    <title>%s</title>\n' "${each}"
      printf '    <content type="html">&lt;a href=&quot;https://github.com/miyu-agent/miyu/releases/tag/v2099.2.0&quot;&gt;v2099.2.0&lt;/a&gt;</content>\n  </entry>\n'
    done
    printf '</feed>\n'
  }
  # 下载处的记录里第一次请求 $1 是第几行；没有的空。
  asked() {
    grep -n -F "GET $1 " "${log}" | head -n 1 | cut -d : -f 1
  }

  # 订阅的先后是乱的：照版本号从新到旧试，rc.10 在 rc.9 前面；它们都没传包（SHA256SUMS 回 404），跳过以后装上真的那一版。
  feed "${tag}" v2099.1.0-rc.9 v2099.1.0-rc.10 v0.7.4 > "${srv}/releases.atom"
  out=$(MIYU_INSTALL_FEED="${here}/releases.atom" MIYU_INSTALL_BASE="${here}" sh "${SCRIPT}" --prerelease 2>&1) \
    || fail "跳过没传包的以后该装上 ${tag}：${out}"
  printf '%s\n' "${out}" | grep -q "Installed Miyu ${version#miyu }" || fail "该装上 ${tag}：${out}"
  ten=$(asked /v2099.1.0-rc.10/SHA256SUMS)
  nine=$(asked /v2099.1.0-rc.9/SHA256SUMS)
  real=$(asked "/${tag}/SHA256SUMS")
  [ -n "${ten}" ] && [ -n "${nine}" ] && [ -n "${real}" ] && [ "${ten}" -lt "${nine}" ] && [ "${nine}" -lt "${real}" ] \
    || fail "该照 rc.10、rc.9、${tag} 的先后试：$(cat "${log}")"
  ! grep -q -F "GET /v2099.2.0/" "${log}" || fail "正文里的链接被认成了标签：$(cat "${log}")"
  ! grep -q -F "GET /v0.7.4/" "${log}" || fail "旧版的 v0.7.4 不该试：$(cat "${log}")"
  sh "${SCRIPT}" --uninstall >/dev/null
  # 只有预发布：不加 --prerelease 的提示加上。
  feed v2099.1.0-rc.1 v0.7.4 > "${srv}/releases.atom"
  out=$(MIYU_INSTALL_FEED="${here}/releases.atom" MIYU_INSTALL_BASE="${here}" sh "${SCRIPT}" 2>&1) && fail "没有正式版的装上了：${out}"
  printf '%s\n' "${out}" | grep -q 'no stable release yet' || fail "没有正式版时该说加 --prerelease：${out}"
  # 有正式版、没传包，也没有预发布：说没找到。
  feed v2099.1.0 > "${srv}/releases.atom"
  out=$(MIYU_INSTALL_FEED="${here}/releases.atom" MIYU_INSTALL_BASE="${here}" sh "${SCRIPT}" 2>&1) && fail "没传包的装上了：${out}"
  printf '%s\n' "${out}" | grep -q 'no release found' || fail "一个都不成的该说没找到：${out}"
  # 订阅是网页（门户、代理换掉的）：说读不了。
  printf '<html><body>sign in</body></html>\n' > "${srv}/releases.atom"
  out=$(MIYU_INSTALL_FEED="${here}/releases.atom" MIYU_INSTALL_BASE="${here}" sh "${SCRIPT}" --prerelease 2>&1) && fail "订阅是网页的装上了：${out}"
  printf '%s\n' "${out}" | grep -q "isn't an Atom feed" || fail "订阅是网页的该说读不了：${out}"
  [ ! -e "${HOME}/.local/lib/miyu" ] || fail "停下的不该留东西"

  # 压缩包被改过：哪台都该停在 SHA-256。
  cp "${srv}/${tag}/${archive}" "${srv}/good"
  printf 'x' >> "${srv}/${tag}/${archive}"
  out=$(MIYU_INSTALL_BASE="http://127.0.0.1:${port}" sh "${SCRIPT}" --version "${tag}" 2>&1) && fail "压缩包改过的装上了：${out}"
  printf '%s\n' "${out}" | grep -q 'SHA-256 of' || fail "该停在 SHA-256：${out}"
  mv "${srv}/good" "${srv}/${tag}/${archive}"
  # SHA256SUMS 被改过：openssl 认 Ed25519 的停在签名；不认的说核不了（这一条照 SHA-256 本身还对，会装上，再卸掉）。
  printf '0000  extra\n' >> "${srv}/${tag}/SHA256SUMS"
  if out=$(MIYU_INSTALL_BASE="http://127.0.0.1:${port}" sh "${SCRIPT}" --version "${tag}" 2>&1); then
    printf '%s\n' "${out}" | grep -q "doesn't support Ed25519" || fail "签名没核却没说：${out}"
    sh "${SCRIPT}" --uninstall >/dev/null
  else
    printf '%s\n' "${out}" | grep -q "signature on SHA256SUMS doesn't match" || fail "该停在签名：${out}"
  fi
  [ ! -e "${HOME}/.local/lib/miyu" ] || fail "停下的不该留东西"
  kill "${server}"
  rm -rf "${srv}" "${log}"
  sh "${SCRIPT}" --prerelease >/dev/null
else
  echo "no python3: skipping the local feed and the tampered downloads"
fi

sh "${SCRIPT}" --uninstall
[ ! -e "${HOME}/.local/lib/miyu" ] || fail "安装目录还在"
[ ! -e "${HOME}/.local/bin/miyu" ] && [ ! -L "${HOME}/.local/bin/miyu" ] || fail "链接还在"
[ -f "${HOME}/.miyu/keep" ] || fail "数据根被动了"
again=$(sh "${SCRIPT}" --uninstall 2>&1)
printf '%s\n' "${again}" | grep -q 'Not installed' || fail "再卸该说没装：${again}"
echo "install.sh: all checks passed"
