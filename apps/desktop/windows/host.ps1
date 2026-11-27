# 使用已安装的微软签名 WebView2 加载桌面壳。缺少运行时或 SDK 时以退出码 2 报告环境未验证。
param(
    [Parameter(Mandatory = $true)][string]$Url,
    [switch]$Probe
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if (-not [Environment]::Is64BitProcess -or [Threading.Thread]::CurrentThread.ApartmentState -ne 'STA') {
    throw 'The WebView2 host requires a 64-bit STA process.'
}
if ($Url -notmatch '^http://127\.0\.0\.1:\d+/[0-9a-f]{32}/') {
    throw 'The desktop host only accepts its own loopback address.'
}

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
    } finally { $reader.Dispose() }
}

function Wait-WebTask($Task, [string]$Operation) {
    $timer = [Diagnostics.Stopwatch]::StartNew()
    while (-not $Task.IsCompleted) {
        if ($timer.Elapsed.TotalSeconds -gt 45) { throw "WebView2 timed out during $Operation." }
        [Windows.Forms.Application]::DoEvents()
        [Threading.Thread]::Sleep(8)
    }
    return $Task.GetAwaiter().GetResult()
}

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
        try { $name = [Reflection.AssemblyName]::GetAssemblyName($core.FullName) }
        catch [BadImageFormatException] { continue }
        if ($name.ProcessorArchitecture -notin @('MSIL', 'Amd64', 'None')) { continue }
        $assembly = $core
        $loader = $loaders | Sort-Object @{ Expression = { if ($_.VersionInfo.ProductVersion -eq $core.VersionInfo.ProductVersion) { 0 } else { 1 } } }, FullName | Select-Object -First 1
        break
    }
    if ($assembly) { break }
}
if (-not $assembly) {
    [Console]::Error.WriteLine('environment_unverified')
    exit 2
}

Add-Type -AssemblyName System.Windows.Forms
[void][Reflection.Assembly]::LoadFrom($assembly.FullName)
[Microsoft.Web.WebView2.Core.CoreWebView2Environment]::SetLoaderDllFolderPath($loader.DirectoryName)
$profile = Join-Path ([IO.Path]::GetTempPath()) ("winstrata-desktop-" + [Guid]::NewGuid().ToString('N'))
$form = New-Object Windows.Forms.Form
$controller = $null
try {
    $form.ClientSize = [Drawing.Size]::new(1600, 900)
    $form.Text = 'WinStrata'
    $form.Show()
    [Windows.Forms.Application]::DoEvents()
    $environment = Wait-WebTask ([Microsoft.Web.WebView2.Core.CoreWebView2Environment]::CreateAsync($null, $profile, $null)) 'environment creation'
    $controller = Wait-WebTask ($environment.CreateCoreWebView2ControllerAsync($form.Handle)) 'controller creation'
    $controller.Bounds = $form.ClientRectangle
    $controller.IsVisible = $true
    $view = $controller.CoreWebView2
    $view.add_NewWindowRequested({ param($source, $request) $request.Handled = $true })
    $view.Navigate($Url)
    if ($Probe) {
        $deadline = [Diagnostics.Stopwatch]::StartNew()
        do {
            [Windows.Forms.Application]::DoEvents()
            if ($deadline.Elapsed.TotalSeconds -gt 45) { throw 'Timed out waiting for the desktop page.' }
            [Threading.Thread]::Sleep(30)
            $raw = Wait-WebTask ($view.ExecuteScriptAsync('window.probeReport || null')) 'reading the desktop page'
        } while (-not $raw -or $raw -eq 'null')
        $result = $raw | ConvertFrom-Json
        if ($result.ok -ne $true) { throw "Desktop page reported a failure: $raw" }
        $result | ConvertTo-Json -Depth 6 -Compress
    } else {
        $script:closing = $false
        $form.add_FormClosing({
            param($sender, $event)
            if ($script:closing) { return }
            $event.Cancel = $true
            try {
                $decision = Invoke-RestMethod -Method Post -Uri ($Url + 'api/command') -Body '{"action":"window_close"}' -ContentType 'application/json'
                if ($decision.window -eq 'exit') { $script:closing = $true; $form.Close() }
                elseif ($decision.window -eq 'hidden') { $form.Hide() }
                else { [void]$view.ExecuteScriptAsync("document.querySelector('#close-dialog').hidden=false") }
            } catch { $script:closing = $true; $form.Close() }
        })
        while (-not $script:closing) {
            [Windows.Forms.Application]::DoEvents()
            [Threading.Thread]::Sleep(200)
            try { $state = Invoke-RestMethod -Uri ($Url + 'api/state') }
            catch { break }
            if ($state.window -eq 'exit') { $script:closing = $true; break }
            elseif ($state.window -eq 'hidden') { $form.Hide() }
            elseif ($state.window -eq 'visible') { $form.Show() }
        }
    }
} finally {
    if ($controller) { $controller.Close() }
    $form.Dispose()
    Remove-Item -LiteralPath $profile -Recurse -Force -ErrorAction SilentlyContinue
}
