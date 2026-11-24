# Install the dwg-mcp binary from the latest GitHub release (Windows x64).
#
#   irm https://raw.githubusercontent.com/aileron-tech/dwg-mcp/main/scripts/install.ps1 | iex
#
# Environment:
#   DWG_MCP_INSTALL_DIR  where to put the binary (default: %LOCALAPPDATA%\Programs\dwg-mcp)
#   DWG_MCP_TAG          release tag to install (default: the latest release)

$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'aileron-tech/dwg-mcp'
$BinName = 'dwg-mcp'
$Target = 'x86_64-pc-windows-msvc'

function Resolve-InstallDir {
    if ($env:DWG_MCP_INSTALL_DIR) { return $env:DWG_MCP_INSTALL_DIR }
    if ($env:LOCALAPPDATA) { return (Join-Path $env:LOCALAPPDATA "Programs\$BinName") }
    return (Join-Path $HOME ".local/bin")
}

function Resolve-Tag {
    if ($env:DWG_MCP_TAG) { return $env:DWG_MCP_TAG }
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -Headers @{ 'User-Agent' = 'dwg-mcp-install' }
    if (-not $release.tag_name) { throw "failed to resolve the latest release tag of $Repo" }
    return $release.tag_name
}

$installDir = Resolve-InstallDir
$tag = Resolve-Tag
$archive = "$BinName-$Target.zip"
$base = "https://github.com/$Repo/releases/download/$tag"
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("dwg-mcp-install-" + [System.Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $tmp | Out-Null

try {
    Write-Host "Installing $BinName $tag ($Target)"
    $zip = Join-Path $tmp $archive
    Invoke-WebRequest -Uri "$base/$archive" -OutFile $zip -UseBasicParsing
    Invoke-WebRequest -Uri "$base/$archive.sha256" -OutFile "$zip.sha256" -UseBasicParsing

    $expected = ((Get-Content "$zip.sha256" -Raw).Trim() -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Path $zip -Algorithm SHA256).Hash.ToLowerInvariant()
    if (-not $expected -or $expected -ne $actual) {
        throw "checksum mismatch for ${archive}: expected $expected, actual $actual"
    }

    Expand-Archive -Path $zip -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item -Path (Join-Path $tmp "$BinName.exe") -Destination (Join-Path $installDir "$BinName.exe") -Force
    Write-Host "Installed $(Join-Path $installDir "$BinName.exe")"

    # Add the folder to the user's PATH (Windows only), for new terminals.
    if ($env:OS -eq 'Windows_NT') {
        $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
        $entries = @($userPath -split ';' | Where-Object { $_ })
        if ($entries -notcontains $installDir) {
            [Environment]::SetEnvironmentVariable('Path', (($entries + $installDir) -join ';'), 'User')
            Write-Host "Added $installDir to your PATH. Open a new terminal to use $BinName."
        }
    }
    Write-Host "Connect it to Claude Code:  claude mcp add dwg -- $BinName"
}
finally {
    Remove-Item -Path $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
