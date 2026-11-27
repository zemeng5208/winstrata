# 使用 Windows 开发镜像中的 64 位 Windows PowerShell 5.1 单线程单元。
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not [Environment]::Is64BitProcess -or [Threading.Thread]::CurrentThread.ApartmentState -ne 'STA') {
    throw 'The WebView2 probe requires a 64-bit STA process.'
}
Add-Type -AssemblyName System.Windows.Forms

function Test-MicrosoftSignature($File) {
    $sig = Get-AuthenticodeSignature -LiteralPath $File.FullName
    return $sig.Status -eq 'Valid' -and $sig.SignerCertificate.Subject -match '(?:^|,\s*)O=Microsoft Corporation(?:,|$)'
}

function Test-Amd64Image($File) {
    $reader = [IO.BinaryReader]::new([IO.File]::OpenRead($File.FullName))
    try {
        $reader.BaseStream.Position = 0x3C
        $peOffset = $reader.ReadInt32()
        $reader.BaseStream.Position = $peOffset
        return $reader.ReadUInt32() -eq 0x4550 -and $reader.ReadUInt16() -eq 0x8664
    } finally {
        $reader.Dispose()
    }
}

function Wait-WebTask($Task, [string]$Operation) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while (-not $Task.IsCompleted) {
        if ($timer.Elapsed.TotalSeconds -gt 30) { throw "WebView2 timed out during $Operation." }
        [Windows.Forms.Application]::DoEvents()
        [Threading.Thread]::Sleep(8)
    }
    return $Task.GetAwaiter().GetResult()
}

# 仅从已安装的开发工具中查找微软签名的 SDK 和加载器。
$roots = @(
    "$env:ProgramFiles\Microsoft Visual Studio\2022",
    "$env:ProgramFiles\Microsoft Visual Studio\18",
    "$env:ProgramFiles\PowerShell",
    "${env:ProgramFiles(x86)}\Microsoft SDKs",
    "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Shared"
)
$assembly = $null
$loader = $null
foreach ($root in $roots) {
    if (-not (Test-Path -LiteralPath $root)) { continue }
    $coreFiles = @(Get-ChildItem -LiteralPath $root -Filter Microsoft.Web.WebView2.Core.dll -File -Recurse -ErrorAction SilentlyContinue |
        Where-Object { $_.FullName -notmatch '\\(?:ref|netcoreapp[^\\]*|net[5-9]\.[^\\]*)\\' } |
        Sort-Object @{ Expression = { if ($_.FullName -match '\\net4[^\\]*\\') { 0 } else { 1 } } }, FullName)
    if (-not $coreFiles.Count) { continue }
    $loaders = @(Get-ChildItem -LiteralPath $root -Filter WebView2Loader.dll -File -Recurse -ErrorAction SilentlyContinue |
        Where-Object { (Test-Amd64Image $_) -and (Test-MicrosoftSignature $_) })
    if (-not $loaders.Count) { continue }
    foreach ($core in $coreFiles) {
        if (-not (Test-MicrosoftSignature $core)) { continue }
        $assembly = $core
        $loader = $loaders | Sort-Object @{ Expression = { if ($_.VersionInfo.ProductVersion -eq $core.VersionInfo.ProductVersion) { 0 } else { 1 } } }, FullName | Select-Object -First 1
        break
    }
    if ($assembly) { break }
}
if (-not $assembly) { throw 'No signed Microsoft WebView2 managed SDK and AMD64 loader pair found in installed tool roots. Rendering unverified.' }

[void][Reflection.Assembly]::LoadFrom($assembly.FullName)
[Microsoft.Web.WebView2.Core.CoreWebView2Environment]::SetLoaderDllFolderPath($loader.DirectoryName)
$profile = Join-Path ([IO.Path]::GetTempPath()) ("winstrata-webview-" + [Guid]::NewGuid().ToString('N'))
$form = New-Object Windows.Forms.Form
$controller = $null
try {
    $form.ClientSize = [Drawing.Size]::new(640, 400)
    $form.Text = 'WinStrata WebView2 feasibility'
    $form.Show()
    [Windows.Forms.Application]::DoEvents()
    $startup = [Diagnostics.Stopwatch]::StartNew()
    $environment = Wait-WebTask ([Microsoft.Web.WebView2.Core.CoreWebView2Environment]::CreateAsync($null, $profile, $null)) 'environment creation'
    $controller = Wait-WebTask ($environment.CreateCoreWebView2ControllerAsync($form.Handle)) 'controller creation'
    $controller.Bounds = $form.ClientRectangle
    $controller.IsVisible = $true
    $startup.Stop()
    $view = $controller.CoreWebView2

    # 本地自编页面仅验证样式计算和帧调度，上游组件需另行验证。
    $html = @'
<!doctype html><html lang="en"><head>
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'">
<style>
body { background: repeating-linear-gradient(45deg, white, navy 20px); }
button { display: block; width: 160px; height: 44px; background: #ffffff60; backdrop-filter: blur(6px); }
</style></head><body><button>Test</button><script>
const panel = document.querySelector('button');
const deltas = [];
let last;
function frame(t) {
  if (last !== undefined) deltas.push(t - last);
  last = t;
  panel.style.transform = 'translateX(' + deltas.length + 'px)';
  if (deltas.length < 30) { requestAnimationFrame(frame); return; }
  const sorted = deltas.slice().sort((a, b) => a - b);
  window.probeReport = {
    samples: sorted.length, intervalMedianMs: sorted[14], intervalP95Ms: sorted[28],
    visibility: document.visibilityState, backdrop: getComputedStyle(panel).backdropFilter,
    transform: getComputedStyle(panel).transform, width: panel.getBoundingClientRect().width
  };
}
requestAnimationFrame(frame);
</script></body></html>
'@
    $view.NavigateToString($html)
    $render = [Diagnostics.Stopwatch]::StartNew()
    do {
        [Windows.Forms.Application]::DoEvents()
        if ($render.Elapsed.TotalSeconds -gt 30) { throw 'Timed out waiting for 30 animation-frame intervals on the local page.' }
        [Threading.Thread]::Sleep(30)
        $raw = Wait-WebTask ($view.ExecuteScriptAsync('window.probeReport || null')) 'reading the local page'
    } while (-not $raw -or $raw -eq 'null')
    $result = $raw | ConvertFrom-Json
    if ($result.visibility -ne 'visible' -or $result.samples -ne 30 -or $result.intervalMedianMs -le 0 -or
        $result.intervalP95Ms -lt $result.intervalMedianMs -or $result.width -ne 160 -or $result.backdrop -ne 'blur(6px)' -or
        $result.transform -ne 'matrix(1, 0, 0, 1, 30, 0)') {
        throw "WebView2 rendered an unexpected fixture result: $raw"
    }
    [ordered]@{
        os = [Environment]::OSVersion.VersionString
        runtime = $environment.BrowserVersionString
        sdk = $assembly.VersionInfo.ProductVersion
        loader = $loader.VersionInfo.ProductVersion
        fixture = 'offline synthetic button'
        hostStartupMs = $startup.ElapsedMilliseconds
        render = $result
    } | ConvertTo-Json -Depth 3 -Compress
} finally {
    if ($controller) { $controller.Close() }
    $form.Dispose()
    for ($i = 0; (Test-Path -LiteralPath $profile) -and $i -lt 20; $i++) {
        [Windows.Forms.Application]::DoEvents()
        Remove-Item -LiteralPath $profile -Recurse -Force -ErrorAction SilentlyContinue
        if (Test-Path -LiteralPath $profile) { [Threading.Thread]::Sleep(150) }
    }
    if (Test-Path -LiteralPath $profile) { Write-Warning 'Temporary browser profile still in use at shutdown.' }
}
