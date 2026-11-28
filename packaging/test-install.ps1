<#
install.ps1 的实测（施工 RL-3），同 test-install.sh：在干净的用户目录里装、核、卸一遍，资产用公开的预发布。CI 的 install-test.yml 跑它，
别在自己的机器上跑（会装进 %LOCALAPPDATA%、改用户的 PATH、卸的时候停核心）。

  pwsh -File packaging/test-install.ps1 -Script packaging/install.ps1

主要的几步用 Windows 自带的 PowerShell 5.1（powershell.exe）跑，最后用 PowerShell 7 再装、卸一遍。
#>
param([Parameter(Mandatory = $true)][string]$Script)

$ErrorActionPreference = 'Stop'
$Script = (Resolve-Path $Script).Path
$Lib = Join-Path $env:LOCALAPPDATA 'Programs\Miyu'

function Fail([string]$why) {
    Write-Host "FAIL: $why"
    exit 1
}

# 用 $shell（powershell 或 pwsh）跑 install.ps1，交回退出码和全部输出。
function Invoke-Install([string]$shell, [string[]]$arguments, [hashtable]$environment = @{}) {
    $saved = @{}
    foreach ($name in $environment.Keys) {
        $saved[$name] = [Environment]::GetEnvironmentVariable($name)
        [Environment]::SetEnvironmentVariable($name, $environment[$name])
    }
    $ErrorActionPreference = 'Continue'
    $out = (& $shell -NoProfile -ExecutionPolicy Bypass -File $Script @arguments 2>&1 | Out-String)
    $code = $LASTEXITCODE
    foreach ($name in $saved.Keys) { [Environment]::SetEnvironmentVariable($name, $saved[$name]) }
    Write-Host $out
    return @{ Code = $code; Out = $out }
}

$first = Invoke-Install 'powershell' @()
if ($first.Code -eq 0) {
    if ($first.Out -notmatch 'Installed Miyu') { Fail "装上了却没说：$($first.Out)" }
    Invoke-Install 'powershell' @('-Uninstall') | Out-Null
} elseif ($first.Out -notmatch 'no stable release yet') {
    Fail "没有正式版时该说加 -Prerelease：$($first.Out)"
}
if (Test-Path $Lib) { Fail '停下的不该留东西' }

$done = Invoke-Install 'powershell' @('-Prerelease')
if ($done.Code -ne 0) { Fail '-Prerelease 没装上' }
$miyu = Join-Path $Lib 'miyu.exe'
if (-not (Select-String -Path (Join-Path $Lib 'install.toml') -Pattern '^channel = "script"' -Quiet)) { Fail 'install.toml 不对' }
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -notcontains $Lib) { Fail "用户的 PATH 里没有 ${Lib}：$userPath" }
$version = [string](& $miyu --version)
Write-Host "installed: $version"
if (Get-ChildItem -Force (Join-Path $env:LOCALAPPDATA 'Programs') -Filter '.miyu-install.*') { Fail '暂存没删' }
$ErrorActionPreference = 'Continue'
$up = (& $miyu upgrade --prerelease 2>&1 | Out-String)
$upCode = $LASTEXITCODE
$ErrorActionPreference = 'Stop'
# 装上的是公开的预发布，它自己的 miyu upgrade 还在问 GitHub API（读订阅的 UP-4 在它以后，还没发版）：共用出口的 CI 机器上撞了
# 限流的，记一句没验成，别的照查。装上的预发布带了 UP-4 以后这一条连同 test-install.sh 那边的一起撤掉。
if ($upCode -eq 0) {
    if ($up -notmatch 'already up to date') { Fail "装完 miyu upgrade 该说已是最新：$up" }
} elseif ($up -match 'rate limit') {
    Write-Host "WARNING: miyu upgrade hit GitHub's anonymous rate limit; the up-to-date check wasn't verified on this run"
} else {
    Fail "miyu upgrade --prerelease 没成：$up"
}
$again = Invoke-Install 'powershell' @('-Prerelease')
if ($again.Code -ne 0 -or $again.Out -notmatch 'already installed') { Fail "再装该说已经装了：$($again.Out)" }

$data = Join-Path $env:USERPROFILE '.miyu'
New-Item -ItemType Directory -Force -Path $data | Out-Null
Set-Content -Path (Join-Path $data 'keep') -Value 'keep'

# 本机起一个下载处和订阅（施工 UP-5，同 test-install.sh）：照版本号从新到旧试、SHA256SUMS 回 404 的跳过，正文里的链接不算，旧版的
# v0.7.x 不试；没有正式版的提示加 -Prerelease，一个都没有的说没找到，订阅是网页的说读不了；压缩包被改过的停在 SHA-256、什么都不装。
$python = Get-Command python -ErrorAction SilentlyContinue
if ($python) {
    $ver = $version -replace '^miyu ', ''
    $tag = "v$ver"
    $archive = "miyu-$ver-windows-x86_64.zip"
    $srv = Join-Path $env:RUNNER_TEMP ('srv-' + [IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Force -Path (Join-Path $srv $tag) | Out-Null
    foreach ($f in @('SHA256SUMS', 'SHA256SUMS.sig', $archive)) {
        Invoke-WebRequest -UseBasicParsing -Uri "https://github.com/miyu-agent/miyu/releases/download/$tag/$f" -OutFile (Join-Path (Join-Path $srv $tag) $f)
    }
    # 下载处每收到一个请求往标准错误记一行（"GET /<路径> HTTP/1.1" 404 这样），照它查试了哪几个、先后。
    $log = Join-Path $env:RUNNER_TEMP ('srv-log-' + [IO.Path]::GetRandomFileName())
    $server = Start-Process -PassThru -NoNewWindow -FilePath $python.Source -ArgumentList @('-m', 'http.server', '18731', '--bind', '127.0.0.1', '--directory', $srv) `
        -RedirectStandardError $log -RedirectStandardOutput "$log.out"
    Start-Sleep -Seconds 2
    Invoke-Install 'powershell' @('-Uninstall') | Out-Null
    $here = 'http://127.0.0.1:18731'
    $served = @{ MIYU_INSTALL_FEED = "$here/releases.atom"; MIYU_INSTALL_BASE = $here }
    $feedFile = Join-Path $srv 'releases.atom'
    $utf8 = New-Object Text.UTF8Encoding $false
    # 照 GitHub 的 releases.atom 的样子写一份订阅。每个 entry 的正文里都有一个转义过的、指向 v2099.2.0 的链接，不该被认成标签。
    function Write-Feed([string[]]$tags) {
        $text = "<?xml version=`"1.0`" encoding=`"UTF-8`"?>`n<feed xmlns=`"http://www.w3.org/2005/Atom`" xml:lang=`"en-US`">`n"
        $text += "  <link type=`"application/atom+xml`" rel=`"self`" href=`"https://github.com/miyu-agent/miyu/releases.atom`"/>`n"
        foreach ($each in $tags) {
            $text += "  <entry>`n    <id>tag:github.com,2008:Repository/1/$each</id>`n"
            $text += "    <link rel=`"alternate`" type=`"text/html`" href=`"https://github.com/miyu-agent/miyu/releases/tag/$each`"/>`n"
            $text += "    <title>$each</title>`n"
            $text += "    <content type=`"html`">&lt;a href=&quot;https://github.com/miyu-agent/miyu/releases/tag/v2099.2.0&quot;&gt;v2099.2.0&lt;/a&gt;</content>`n  </entry>`n"
        }
        [IO.File]::WriteAllText($feedFile, $text + "</feed>`n", $utf8)
    }
    # 下载处的记录里第一次请求 $path 是第几行；没有的 0。
    function Get-Asked([string]$path) {
        $lines = @(Get-Content $log)
        for ($i = 0; $i -lt $lines.Count; $i++) {
            if ($lines[$i].Contains("GET $path ")) { return $i + 1 }
        }
        return 0
    }

    # 订阅的先后是乱的：照版本号从新到旧试，rc.10 在 rc.9 前面；它们都没传包（SHA256SUMS 回 404），跳过以后装上真的那一版。
    Write-Feed @($tag, 'v2099.1.0-rc.9', 'v2099.1.0-rc.10', 'v0.7.4')
    $picked = Invoke-Install 'powershell' @('-Prerelease') $served
    if ($picked.Code -ne 0 -or $picked.Out -notmatch [regex]::Escape("Installed Miyu $ver")) { Fail "跳过没传包的以后该装上 ${tag}：$($picked.Out)" }
    Invoke-Install 'powershell' @('-Uninstall') | Out-Null
    # 只有预发布：不加 -Prerelease 的提示加上。
    Write-Feed @('v2099.1.0-rc.1', 'v0.7.4')
    $bad = Invoke-Install 'powershell' @() $served
    if ($bad.Code -eq 0 -or $bad.Out -notmatch 'no stable release yet') { Fail "没有正式版时该说加 -Prerelease：$($bad.Out)" }
    # 有正式版、没传包，也没有预发布：说没找到。
    Write-Feed @('v2099.1.0')
    $bad = Invoke-Install 'powershell' @() $served
    if ($bad.Code -eq 0 -or $bad.Out -notmatch 'no release found') { Fail "一个都不成的该说没找到：$($bad.Out)" }
    # 订阅是网页（门户、代理换掉的）：说读不了。
    [IO.File]::WriteAllText($feedFile, "<html><body>sign in</body></html>`n", $utf8)
    $bad = Invoke-Install 'powershell' @('-Prerelease') $served
    if ($bad.Code -eq 0 -or $bad.Out -notmatch "isn't an Atom feed") { Fail "订阅是网页的该说读不了：$($bad.Out)" }
    if (Test-Path $Lib) { Fail '停下的不该留东西' }

    # 压缩包被改过：停在 SHA-256、什么都不装。
    [IO.File]::AppendAllText((Join-Path (Join-Path $srv $tag) $archive), 'x')
    $bad = Invoke-Install 'powershell' @('-Version', $ver) @{ MIYU_INSTALL_BASE = $here }
    Stop-Process -Id $server.Id
    if ($bad.Code -eq 0 -or $bad.Out -notmatch 'SHA-256 of') { Fail "压缩包改过的该停在 SHA-256：$($bad.Out)" }
    if (Test-Path $Lib) { Fail '停下的不该留东西' }
    Start-Sleep -Seconds 1
    # 下载处停了再读它的记录（开着的时候 Windows 上不一定读得了）。第一趟（装上真的那一版）最先请求这三个，后面几趟不改它们的
    # 先后；正文里的链接、旧版的 v0.7.4 哪一趟都不该请求。
    $ten = Get-Asked '/v2099.1.0-rc.10/SHA256SUMS'
    $nine = Get-Asked '/v2099.1.0-rc.9/SHA256SUMS'
    $real = Get-Asked "/$tag/SHA256SUMS"
    $seen = Get-Content -Raw $log
    if (-not ($ten -gt 0 -and $ten -lt $nine -and $nine -lt $real)) { Fail "该照 rc.10、rc.9、$tag 的先后试：$seen" }
    if ($seen.Contains('GET /v2099.2.0/')) { Fail "正文里的链接被认成了标签：$seen" }
    if ($seen.Contains('GET /v0.7.4/')) { Fail "旧版的 v0.7.4 不该试：$seen" }
    Remove-Item -Recurse -Force $srv, $log, "$log.out"
    Invoke-Install 'powershell' @('-Prerelease') | Out-Null
} else {
    Write-Host 'no python: skipping the local feed and the tampered download'
}

$gone = Invoke-Install 'powershell' @('-Uninstall')
if ($gone.Code -ne 0) { Fail '-Uninstall 没成' }
if (Test-Path $Lib) { Fail '安装目录还在' }
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -contains $Lib) { Fail "用户的 PATH 里还有 ${Lib}" }
if (-not (Test-Path (Join-Path $data 'keep'))) { Fail '数据根被动了' }
$again = Invoke-Install 'powershell' @('-Uninstall')
if ($again.Out -notmatch 'Not installed') { Fail "再卸该说没装：$($again.Out)" }

# 照说明的 & ([scriptblock]::Create((irm …))) 跑：irm 交回的字符串开头还带着 BOM，脚本块照样认（5.1 里跑）。
$command = "& ([scriptblock]::Create([Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes('$Script')))) -Uninstall"
$ErrorActionPreference = 'Continue'
$block = (& powershell -NoProfile -ExecutionPolicy Bypass -Command $command 2>&1 | Out-String)
$ErrorActionPreference = 'Stop'
Write-Host $block
if ($block -notmatch 'Not installed') { Fail "当脚本块跑不起来：$block" }

# 中文提示照 \uXXXX 解得出来：LANG 设成中文再跑一次，输出写进 UTF-8 文件（控制台的代码页显示不了中文）。
$said = Join-Path $env:RUNNER_TEMP 'said-zh.txt'
$command = "`$env:LANG = 'zh_CN.UTF-8'; & ([scriptblock]::Create([Text.Encoding]::UTF8.GetString([IO.File]::ReadAllBytes('$Script')))) -Uninstall *>&1 | Out-File -Encoding utf8 '$said'"
$ErrorActionPreference = 'Continue'
& powershell -NoProfile -ExecutionPolicy Bypass -Command $command 2>&1 | Out-Null
$ErrorActionPreference = 'Stop'
$zh = Get-Content -Raw -Encoding utf8 $said
Write-Host $zh
if ($zh -notmatch '未安装') { Fail "中文提示没解出来：$zh" }

# PowerShell 7 也装、卸一遍。
$seven = Invoke-Install 'pwsh' @('-Prerelease')
if ($seven.Code -ne 0 -or -not (Test-Path $miyu)) { Fail 'PowerShell 7 没装上' }
$seven = Invoke-Install 'pwsh' @('-Uninstall')
if ($seven.Code -ne 0 -or (Test-Path $Lib)) { Fail 'PowerShell 7 没卸掉' }
Write-Host 'install.ps1: all checks passed'
