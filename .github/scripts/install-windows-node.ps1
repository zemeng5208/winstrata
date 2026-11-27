$ErrorActionPreference = 'Stop'
$version = (Get-Content -Raw package.json | ConvertFrom-Json).engines.node
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Expected an exact Node.js version' }
$node = Get-Command node -CommandType Application -ErrorAction SilentlyContinue
if ($node -and (& $node.Source --version) -eq "v$version") { return }
$name = "node-v$version-win-x64"
$url = "https://nodejs.org/dist/v$version"
$archive = Join-Path $env:RUNNER_TEMP "$name.zip"
Invoke-WebRequest "$url/$name.zip" -OutFile $archive
$checks = (Invoke-RestMethod "$url/SHASUMS256.txt") -split '\r?\n'
$line = @($checks | Where-Object { $_.EndsWith("  $name.zip") })
if ($line.Count -ne 1 -or $line[0].Split(' ')[0] -ne (Get-FileHash $archive).Hash) { throw 'Unexpected Node archive digest' }
Expand-Archive -LiteralPath $archive -DestinationPath $env:RUNNER_TEMP -Force
$directory = Join-Path $env:RUNNER_TEMP $name
if ((& (Join-Path $directory 'node.exe') --version) -ne "v$version") { throw 'Unexpected Node.js version' }
$directory >> $env:GITHUB_PATH
