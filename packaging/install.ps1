<#
Miyu 的安装脚本，Windows 用（施工 RL-3，设计 59 第三节第 2 条、第四节）。Linux、macOS 用 install.sh。

  & ([scriptblock]::Create((irm https://raw.githubusercontent.com/miyu-agent/miyu/main/packaging/install.ps1)))
  & ([scriptblock]::Create((irm …/install.ps1))) -Prerelease       # 也算预发布
  & ([scriptblock]::Create((irm …/install.ps1))) -Version 2026.11.0    # 装指定的一版
  & ([scriptblock]::Create((irm …/install.ps1))) -Uninstall        # 卸

从 GitHub Releases 下载 Windows 的压缩包。先用写在这里的发版公钥核 SHA256SUMS 的签名：要 PATH 上有认 Ed25519 的 openssl（Git for
Windows 带的就行），Windows 自带的工具不认。再核压缩包的 SHA-256，解开后跑一次新的 `miyu --version`。都对了才装：
  - 压缩包第一层整个放进 %LOCALAPPDATA%\Programs\Miyu\，旁边写 install.toml（channel = "script"，`miyu upgrade` 照它自己升级）；
  - 把这个目录加进当前用户的 PATH（只动用户自己的环境变量）。
不要管理员。数据根（%USERPROFILE%\.miyu）装、卸都不碰。Windows PowerShell 5.1 和 PowerShell 7 都能跑。

编码：这个文件是不带 BOM 的 UTF-8，注释以外只有 ASCII（测试守着）。
  - Windows PowerShell 5.1 用 -File 跑不带 BOM 的脚本时照系统代码页解，中文字节会冒出它认作引号的弯引号，字符串就断了。
  - 带了 BOM，irm 交回的字符串开头留着 BOM 字符，[scriptblock]::Create 又认不出开头的注释块。
  - 所以给人看的中文写成 \uXXXX，照 [regex]::Unescape 解出来再用 {0} 填值（路径里的反斜杠不经过 Unescape）；注释里的中文读乱了也无妨。

设了 GITHUB_TOKEN 的，问 GitHub API 时带上它（只发给 API，下载不带），免得共用出口的机器撞上匿名每小时 60 次的限流。
测试用的两个环境变量：MIYU_INSTALL_API（GitHub 的 API 地址）、MIYU_INSTALL_BASE（下载地址的前半）。
#>
[CmdletBinding()]
param(
    [switch]$Prerelease,
    [string]$Version = '',
    [switch]$Uninstall
)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = 'miyu-agent/miyu'
$Api = if ($env:MIYU_INSTALL_API) { $env:MIYU_INSTALL_API } else { "https://api.github.com/repos/$Repo" }
$Base = if ($env:MIYU_INSTALL_BASE) { $env:MIYU_INSTALL_BASE } else { "https://github.com/$Repo/releases/download" }
$Programs = Join-Path $env:LOCALAPPDATA 'Programs'
$Lib = Join-Path $Programs 'Miyu'
$Target = 'windows-x86_64'

# 发版公钥：编号和 32 字节的 Ed25519 公钥（base64），同 crates/miyu-source/src/sign.rs 的 RELEASE（测试守着两边一样）。
$KeyId = 'ca6918041857843c'
$Key = 'LOj3CWiaD/hcIp5yRUzy6zAGKivZDP5Dl+5O51kupYs='

# 设了 LANG 的照它挑（同 install.sh），没设的照界面语言。名字别和下面函数的参数 $zh 撞：PowerShell 的变量名不分大小写。
$UseChinese = if ($env:LANG) { $env:LANG -like 'zh*' } else { (Get-UICulture).Name -like 'zh*' }

# 照语言挑一句：中文的照 \uXXXX 解，{0}、{1} 填 $values。
function Text([string]$zh, [string]$en, [object[]]$values) {
    $template = if ($UseChinese) { [regex]::Unescape($zh) } else { $en }
    return [string]::Format($template, $values)
}

function Say([string]$zh, [string]$en, [object[]]$values = @()) {
    Write-Host (Text $zh $en $values)
}

# 停下：抛出去。用 -File 跑的退出码是 1；在会话里照上面那样跑的只报错，不关窗口。
function Die([string]$zh, [string]$en, [object[]]$values = @()) {
    throw ((Text 'miyu \u5b89\u88c5\uff1a' 'miyu install: ' @()) + (Text $zh $en $values))
}

# 跑一个外部程序，交回退出码；它往标准错误写的不算 PowerShell 的错。
function Run([string]$exe, [string[]]$arguments) {
    $ErrorActionPreference = 'Continue'
    & $exe @arguments *> $null
    return $LASTEXITCODE
}

function Without-Lib([string]$path) {
    ($path -split ';' | Where-Object { $_ -and ($_.TrimEnd('\') -ne $Lib.TrimEnd('\')) }) -join ';'
}

function Uninstall-Miyu {
    if (-not (Test-Path $Lib)) {
        Say '\u672a\u5b89\u88c5\uff08{0} \u4e0d\u5b58\u5728\uff09' "Not installed ({0} doesn't exist)" @($Lib)
        return
    }
    $marker = Join-Path $Lib 'install.toml'
    if (-not ((Test-Path $marker) -and (Select-String -Path $marker -Pattern '^channel = "script"' -Quiet))) {
        Die '{0} \u4e0d\u662f\u8fd9\u4e2a\u811a\u672c\u5b89\u88c5\u7684\uff08\u6ca1\u6709 install.toml\uff09\uff0c\u6ca1\u6709\u4fee\u6539\u5b83' "{0} wasn't installed by this script (no install.toml); leaving it alone" @($Lib)
    }
    $miyu = Join-Path $Lib 'miyu.exe'
    if (Test-Path $miyu) {
        if ((Run $miyu @('service', 'disable')) -ne 0) {
            Say '\u65e0\u6cd5\u79fb\u9664\u5f00\u673a\u542f\u52a8\u7684\u4efb\u52a1\u8ba1\u5212\uff0c\u7ee7\u7eed\u5378\u8f7d\uff08\u4e4b\u540e\u9700\u8981\u6309 packaging/README.md \u624b\u52a8\u5220\u9664\uff09' "Couldn't remove the startup task; going on (remove it by hand as in packaging/README.md)"
        }
        if ((Run $miyu @('core', 'stop')) -ne 0) {
            Say '\u65e0\u6cd5\u505c\u6b62\u6838\u5fc3\uff0c\u7ee7\u7eed\u5378\u8f7d' "Couldn't stop the core; going on"
        }
    }
    try {
        Remove-Item -Recurse -Force $Lib
    } catch {
        Die '\u65e0\u6cd5\u5220\u9664 {0}\uff08{1}\uff09\uff1a\u8fd8\u6709 Miyu \u7684\u7a0b\u5e8f\u6b63\u5728\u8fd0\u884c\uff0c\u5173\u95ed\u4ee5\u540e\u518d\u5378\u8f7d' "can't remove {0} ({1}): close any running Miyu program and try again" @($Lib, $_.Exception.Message)
    }
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath) {
        [Environment]::SetEnvironmentVariable('Path', (Without-Lib $userPath), 'User')
    }
    $env:Path = Without-Lib $env:Path
    $data = if ($env:MIYU_HOME) { $env:MIYU_HOME } else { Join-Path $env:USERPROFILE '.miyu' }
    Say '\u5df2\u5378\u8f7d Miyu\u3002\u6570\u636e\u4ecd\u7136\u4fdd\u7559\u5728 {0}\uff0c\u5982\u679c\u4e0d\u518d\u9700\u8981\uff0c\u53ef\u4ee5\u81ea\u884c\u5220\u9664\u3002' "Miyu is uninstalled. Your data is still in {0}; delete it yourself if you don't need it." @($data)
}

# 照 API 找标签；没有的空字符串。
function Find-Tag([bool]$pre) {
    $url = if ($pre) { "$Api/releases?per_page=30" } else { "$Api/releases/latest" }
    try {
        $headers = @{ Accept = 'application/vnd.github+json' }
        if ($env:GITHUB_TOKEN) { $headers['Authorization'] = "Bearer $($env:GITHUB_TOKEN)" }
        $answer = Invoke-RestMethod -Uri $url -Headers $headers -UseBasicParsing
    } catch {
        $code = 0
        if ($_.Exception.Response) { $code = [int]$_.Exception.Response.StatusCode }
        if ($code -eq 404) { return '' }
        if ($code -eq 403 -or $code -eq 429) {
            Die 'GitHub \u9650\u6d41\uff1a\u533f\u540d\u8bbf\u95ee\u6bcf\u5c0f\u65f6 60 \u6b21\uff0c\u5df2\u7ecf\u7528\u5b8c\uff0c\u7a0d\u540e\u91cd\u8bd5' 'GitHub rate limit reached (60 an hour without signing in); try again later'
        }
        Die '\u65e0\u6cd5\u8fde\u63a5 GitHub\uff08{0}\uff09\uff1a{1}' "can't reach GitHub ({0}): {1}" @($url, $_.Exception.Message)
    }
    # 列表照新到旧排、匿名看不到草稿：第一个 v 打头的就是最新的。
    foreach ($release in @($answer)) {
        if ($release.tag_name -like 'v*') { return [string]$release.tag_name }
    }
    return ''
}

function Fetch([string]$url, [string]$to) {
    try {
        Invoke-WebRequest -Uri $url -OutFile $to -UseBasicParsing
    } catch {
        Die '\u4e0b\u8f7d\u5931\u8d25\uff1a{0}\uff08{1}\uff09' 'download failed: {0} ({1})' @($url, $_.Exception.Message)
    }
}

# 照写在这里的公钥核 SHA256SUMS 的签名。核得了却核不过的停；没有 openssl、openssl 不认 Ed25519 的说一句、接着装。
function Test-Signature([string]$sums, [string]$sigFile, [string]$work) {
    $sig = Get-Content -Raw $sigFile | ConvertFrom-Json
    if ($sig.key -ne $KeyId) {
        Die 'SHA256SUMS \u7684\u7b7e\u540d\u6765\u81ea\u5176\u4ed6\u5bc6\u94a5\uff08{0}\uff09\uff0c\u4e0d\u5b89\u88c5' 'SHA256SUMS is signed by another key ({0}); not installing' @($sig.key)
    }
    $openssl = Get-Command openssl -ErrorAction SilentlyContinue
    if (-not $openssl) {
        Say '\u8fd9\u53f0\u673a\u5668\u6ca1\u6709 openssl\uff08Windows \u81ea\u5e26\u7684\u5de5\u5177\u4e0d\u652f\u6301 Ed25519\uff09\uff0c\u6ca1\u6709\u6838\u5bf9\u7b7e\u540d\uff08\u4e0d\u662f\u7b7e\u540d\u6709\u95ee\u9898\uff09\uff0c\u53ea\u6838\u5bf9 SHA-256\u3002\u5982\u679c\u8981\u6838\u5bf9\u7b7e\u540d\uff0c\u8fd0\u884c\uff1agh attestation verify <\u6587\u4ef6> -R {0}' `
            "No openssl here (Windows' own tools don't support Ed25519), so the signature wasn't checked (it isn't broken); checking SHA-256 only. To check it: gh attestation verify <file> -R {0}" @($Repo)
        return
    }
    $openssl = $openssl.Source
    # Ed25519 公钥的 DER 前缀（RFC 8410）加上 32 字节的公钥。
    [byte[]]$prefix = 0x30, 0x2a, 0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70, 0x03, 0x21, 0x00
    $der = $prefix + [Convert]::FromBase64String($Key)
    $pem = Join-Path $work 'release.pem'
    [IO.File]::WriteAllText($pem, "-----BEGIN PUBLIC KEY-----`n$([Convert]::ToBase64String($der))`n-----END PUBLIC KEY-----`n")
    $raw = Join-Path $work 'sums.sig'
    [IO.File]::WriteAllBytes($raw, [Convert]::FromBase64String([string]$sig.signature))
    $ErrorActionPreference = 'Continue'
    $help = (& $openssl pkeyutl -help 2>&1 | Out-String)
    $ErrorActionPreference = 'Stop'
    if ((Run $openssl @('pkey', '-pubin', '-in', $pem, '-noout')) -ne 0 -or $help -notmatch '-rawin') {
        Say '\u8fd9\u53f0\u673a\u5668\u7684 openssl \u4e0d\u652f\u6301 Ed25519\uff0c\u6ca1\u6709\u6838\u5bf9\u7b7e\u540d\uff08\u4e0d\u662f\u7b7e\u540d\u6709\u95ee\u9898\uff09\uff0c\u53ea\u6838\u5bf9 SHA-256\u3002\u5982\u679c\u8981\u6838\u5bf9\u7b7e\u540d\uff0c\u8fd0\u884c\uff1agh attestation verify <\u6587\u4ef6> -R {0}' `
            "This machine's openssl doesn't support Ed25519, so the signature wasn't checked (it isn't broken); checking SHA-256 only. To check it: gh attestation verify <file> -R {0}" @($Repo)
        return
    }
    if ((Run $openssl @('pkeyutl', '-verify', '-pubin', '-inkey', $pem, '-rawin', '-in', $sums, '-sigfile', $raw)) -ne 0) {
        Die 'SHA256SUMS \u7684\u7b7e\u540d\u6838\u5bf9\u5931\u8d25\uff0c\u4e0d\u5b89\u88c5' "the signature on SHA256SUMS doesn't match; not installing"
    }
    Say '\u7b7e\u540d\u6838\u5bf9\u901a\u8fc7\uff08\u53d1\u7248\u516c\u94a5 {0}\uff09' 'Signature checked (release key {0})' @($KeyId)
}

function Install-Miyu {
    if ($env:PROCESSOR_ARCHITECTURE -ne 'AMD64') {
        Die '\u6ca1\u6709 {0} \u7684 Windows \u53d1\u884c\u5305\uff08\u76ee\u524d\u53ea\u6709 x86_64 \u7684\u53d1\u884c\u5305\uff09' 'no Windows release for {0} (only x86_64)' @($env:PROCESSOR_ARCHITECTURE)
    }
    $miyu = Join-Path $Lib 'miyu.exe'
    if (Test-Path $miyu) {
        $ErrorActionPreference = 'Continue'
        $now = [string](& $miyu --version 2>$null)
        $ErrorActionPreference = 'Stop'
        Say '\u5df2\u5b89\u88c5 {0}\uff08{1}\uff09\uff1b\u5982\u679c\u8981\u5347\u7ea7\uff0c\u8fd0\u884c miyu upgrade' '{0} is already installed ({1}); use miyu upgrade to upgrade' @($now, $Lib)
        return
    }
    if (Test-Path $Lib) {
        Die '{0} \u5df2\u5b58\u5728\uff0c\u4f46\u91cc\u9762\u6ca1\u6709 miyu.exe\uff0c\u9700\u8981\u5148\u628a\u5b83\u79fb\u8d70' '{0} exists but has no miyu.exe in it; move it away first' @($Lib)
    }

    New-Item -ItemType Directory -Force -Path $Programs | Out-Null
    # 暂存放在 Programs 下：和装的地方同一块盘，最后一步只是改名。
    $work = Join-Path $Programs ('.miyu-install.' + [IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $work | Out-Null
    try {
        $wanted = $Version.TrimStart('v')
        if ($wanted) {
            $tag = "v$wanted"
        } elseif ($Prerelease) {
            $tag = Find-Tag $true
            if (-not $tag) { Die '\u672a\u627e\u5230\u53ef\u7528\u7684\u53d1\u884c\u7248' 'no release found' }
        } else {
            $tag = Find-Tag $false
            if (-not $tag) { Die '\u8fd8\u6ca1\u6709\u6b63\u5f0f\u7248\uff1b\u5982\u679c\u8981\u5b89\u88c5\u9884\u53d1\u5e03\u7248\uff0c\u52a0\u4e0a -Prerelease' 'no stable release yet; add -Prerelease to install a pre-release' }
        }
        $ver = $tag.Substring(1)
        $name = "miyu-$ver-$Target"
        $archive = "$name.zip"

        Say '\u6b63\u5728\u4e0b\u8f7d {0}' 'Downloading {0}' @($archive)
        $sums = Join-Path $work 'SHA256SUMS'
        $sigFile = Join-Path $work 'SHA256SUMS.sig'
        Fetch "$Base/$tag/SHA256SUMS" $sums
        Fetch "$Base/$tag/SHA256SUMS.sig" $sigFile
        Test-Signature $sums $sigFile $work
        $want = ''
        foreach ($line in Get-Content $sums) {
            $parts = $line -split '\s+', 2
            if ($parts.Count -eq 2 -and ($parts[1] -eq $archive -or $parts[1] -eq "*$archive")) { $want = $parts[0].ToLower() }
        }
        if (-not $want) { Die 'SHA256SUMS \u91cc\u6ca1\u6709 {0}' "{0} isn't listed in SHA256SUMS" @($archive) }
        $zip = Join-Path $work $archive
        Fetch "$Base/$tag/$archive" $zip
        $got = (Get-FileHash -Algorithm SHA256 -Path $zip).Hash.ToLower()
        if ($got -ne $want) { Die '{0} \u7684 SHA-256 \u548c\u6e05\u5355\u4e0d\u4e00\u81f4\uff0c\u4e0d\u5b89\u88c5' "the SHA-256 of {0} doesn't match; not installing" @($archive) }

        $unpacked = Join-Path $work 'unpacked'
        try {
            Expand-Archive -Path $zip -DestinationPath $unpacked
        } catch {
            Die '\u65e0\u6cd5\u89e3\u538b {0}\uff08{1}\uff09' "can't unpack {0} ({1})" @($archive, $_.Exception.Message)
        }
        $top = Join-Path $unpacked $name
        if (-not ((Test-Path (Join-Path $top 'miyu.exe')) -and (Test-Path (Join-Path $top 'resources')))) {
            Die '{0} \u91cc\u7f3a\u5c11\u6587\u4ef6' '{0} is missing files' @($archive)
        }
        $ErrorActionPreference = 'Continue'
        $reported = [string](& (Join-Path $top 'miyu.exe') --version 2>$null)
        $ErrorActionPreference = 'Stop'
        if ($reported.Trim() -ne "miyu $ver") {
            Die '\u65b0\u7248 miyu --version \u62a5\u544a\u7684\u662f\u300c{0}\u300d\uff0c\u5e94\u4e3a {1}' 'the new miyu --version says "{0}", not {1}' @($reported, $ver)
        }

        $utf8 = New-Object Text.UTF8Encoding $false
        $marker = [regex]::Unescape('# \u5b89\u88c5\u811a\u672c\u88c5\u7684\uff08\u65bd\u5de5 RL-3\uff09\uff1amiyu upgrade \u7167\u5b83\u81ea\u5df1\u4e0b\u8f7d\u3001\u6838\u5bf9\u3001\u66ff\u6362\u3002') + "`nchannel = `"script`"`n"
        [IO.File]::WriteAllText((Join-Path $top 'install.toml'), $marker, $utf8)
        Move-Item -Path $top -Destination $Lib
    } finally {
        Remove-Item -Recurse -Force $work -ErrorAction SilentlyContinue
    }

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = if ($userPath) { $userPath -split ';' } else { @() }
    if (-not ($entries | Where-Object { $_.TrimEnd('\') -eq $Lib })) {
        $joined = (@($entries | Where-Object { $_ }) + $Lib) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $joined, 'User')
        Say '\u5df2\u628a {0} \u52a0\u5165\u4f60\u7684 PATH\uff1a\u65b0\u6253\u5f00\u7684\u7ec8\u7aef\u91cc\u53ef\u4ee5\u76f4\u63a5\u8fd0\u884c miyu' 'Added {0} to your PATH: new terminals will find miyu' @($Lib)
    }
    if (($env:Path -split ';') -notcontains $Lib) { $env:Path = "$env:Path;$Lib" }

    Say '\u5df2\u5b89\u88c5 Miyu {0}\uff08{1}\uff09' 'Installed Miyu {0} ({1})' @($ver, $Lib)
    Say '\u4e0b\u4e00\u6b65\uff1a\u8fd0\u884c miyu \u5f00\u59cb\u4f7f\u7528\uff1b\u5982\u679c\u8981\u767b\u5f55\u65f6\u542f\u52a8\uff0c\u8fd0\u884c miyu service enable\u3002\u5347\u7ea7\u8fd0\u884c miyu upgrade\uff0c\u5378\u8f7d\u8fd0\u884c\u8fd9\u4e2a\u811a\u672c\u5e76\u52a0\u4e0a -Uninstall\u3002' `
        'Next: run miyu to start; run miyu service enable to start it at sign-in. Upgrade with miyu upgrade; uninstall with this script and -Uninstall.'
}

if ($Uninstall) { Uninstall-Miyu } else { Install-Miyu }
