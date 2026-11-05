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
# miyu upgrade 匿名问 GitHub：共用出口的 CI 机器上撞了限流的，记一句没验成，别的照查。
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

# 压缩包被改过：本机起一个下载处，停在 SHA-256、什么都不装。
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
    [IO.File]::AppendAllText((Join-Path (Join-Path $srv $tag) $archive), 'x')
    $server = Start-Process -PassThru -WindowStyle Hidden -FilePath $python.Source -ArgumentList @('-m', 'http.server', '18731', '--bind', '127.0.0.1', '--directory', $srv)
    Start-Sleep -Seconds 2
    Invoke-Install 'powershell' @('-Uninstall') | Out-Null
    $bad = Invoke-Install 'powershell' @('-Version', $ver) @{ MIYU_INSTALL_BASE = 'http://127.0.0.1:18731' }
    Stop-Process -Id $server.Id
    if ($bad.Code -eq 0 -or $bad.Out -notmatch 'SHA-256 of') { Fail "压缩包改过的该停在 SHA-256：$($bad.Out)" }
    if (Test-Path $Lib) { Fail '停下的不该留东西' }
    Remove-Item -Recurse -Force $srv
    Invoke-Install 'powershell' @('-Prerelease') | Out-Null
} else {
    Write-Host 'no python: skipping the tampered download'
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
if ($zh -notmatch '没有装着') { Fail "中文提示没解出来：$zh" }

# PowerShell 7 也装、卸一遍。
$seven = Invoke-Install 'pwsh' @('-Prerelease')
if ($seven.Code -ne 0 -or -not (Test-Path $miyu)) { Fail 'PowerShell 7 没装上' }
$seven = Invoke-Install 'pwsh' @('-Uninstall')
if ($seven.Code -ne 0 -or (Test-Path $Lib)) { Fail 'PowerShell 7 没卸掉' }
Write-Host 'install.ps1: all checks passed'
