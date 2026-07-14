[CmdletBinding()]
param(
    [string]$Release = $env:PRODEX_RELEASE
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

if ([string]::IsNullOrWhiteSpace($Release)) {
    $Release = "latest"
}

$Repository = if ([string]::IsNullOrWhiteSpace($env:PRODEX_GITHUB_REPOSITORY)) {
    "christiandoxa/prodex"
} else {
    $env:PRODEX_GITHUB_REPOSITORY
}
$BaseUrlOverride = $env:PRODEX_RELEASE_BASE_URL
$RunningExe = $env:PRODEX_RUNNING_EXE
$NonInteractive = $env:PRODEX_NON_INTERACTIVE -match "^(?i:1|true|yes)$"
$Migrate = $env:PRODEX_MIGRATE -match "^(?i:1|true|yes)$"
$NoPathUpdate = $env:PRODEX_NO_PATH_UPDATE -match "^(?i:1|true|yes)$"

function Write-Step {
    param([string]$Message)
    Write-Host "==> $Message"
}

function Write-WarningStep {
    param([string]$Message)
    Write-Warning $Message
}

function Assert-ValidRelease {
    if ($Release -cne "latest" -and $Release -cnotmatch "^[0-9]+\.[0-9]+\.[0-9]+(?:[+-][0-9A-Za-z.-]+)?$") {
        throw "Invalid Prodex release: $Release. Expected latest or x.y.z."
    }
}

function Get-Target {
    $architecture = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    if ($architecture -eq [System.Runtime.InteropServices.Architecture]::X64) {
        return "x86_64-pc-windows-msvc"
    }
    if ($architecture -eq [System.Runtime.InteropServices.Architecture]::Arm64) {
        return "aarch64-pc-windows-msvc"
    }
    throw "install.ps1 supports only x64 and arm64 Windows. Detected: $architecture"
}

function Copy-Download {
    param(
        [string]$Source,
        [string]$Destination
    )

    if (Test-Path -LiteralPath $Source) {
        Copy-Item -LiteralPath $Source -Destination $Destination
        return
    }
    Invoke-WebRequest -UseBasicParsing -Uri $Source -OutFile $Destination
}

function Join-DownloadSource {
    param(
        [string]$Base,
        [string]$Leaf
    )

    if (Test-Path -LiteralPath $Base) {
        return Join-Path $Base $Leaf
    }
    return "$($Base.TrimEnd('/'))/$Leaf"
}

function Get-ExpectedDigest {
    param(
        [string]$ManifestPath,
        [string]$AssetName
    )

    $escapedAsset = [regex]::Escape($AssetName)
    foreach ($line in Get-Content -LiteralPath $ManifestPath) {
        $match = [regex]::Match($line, "^\s*([0-9a-fA-F]{64})\s+\*?$escapedAsset\s*$")
        if ($match.Success) {
            return $match.Groups[1].Value.ToLowerInvariant()
        }
    }
    throw "Could not find a valid checksum for $AssetName."
}

function Get-ProdexVersion {
    param([string]$BinaryPath)

    $output = & $BinaryPath --version 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "Downloaded asset did not run as Prodex."
    }
    $versionLine = ([string]$output).Trim()
    if ($versionLine -cnotmatch "^prodex ([0-9]+\.[0-9]+\.[0-9]+(?:[+-][0-9A-Za-z.-]+)?)$") {
        throw "Downloaded asset did not report a valid Prodex version."
    }
    return $Matches[1]
}

function Get-ExistingProdexPath {
    if (-not [string]::IsNullOrWhiteSpace($RunningExe)) {
        return $RunningExe
    }
    $command = Get-Command prodex -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($null -eq $command) {
        return ""
    }
    return [string]$command.Source
}

function Get-InstallManager {
    param([string]$ExistingPath)

    $normalized = $ExistingPath.Replace("\", "/")
    if ($env:npm_package_name -eq "@christiandoxa/prodex" -or
        $normalized -match "/node_modules/@christiandoxa/prodex(?:-|/)") {
        return "npm"
    }
    if ($normalized -match "/\.cargo/bin/prodex(?:\.exe)?$") {
        return "cargo"
    }
    return "direct"
}

function Prompt-YesNo {
    param([string]$Prompt)

    if ($NonInteractive -or [Console]::IsInputRedirected -or [Console]::IsOutputRedirected) {
        return $false
    }
    return (Read-Host "$Prompt [y/N]") -match "^(?i:y(?:es)?)$"
}

function Invoke-Native {
    param(
        [string]$Command,
        [string[]]$Arguments
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "$Command exited with code $LASTEXITCODE."
    }
}

function Migrate-InstallManager {
    param([string]$Manager)

    if ($Manager -eq "npm") {
        $npm = Get-Command npm -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($null -eq $npm) {
            throw "npm is required to remove the previous npm installation."
        }
        if ([string]::IsNullOrWhiteSpace($env:PRODEX_CODEX_BIN) -or $env:PRODEX_CODEX_BIN -eq "codex") {
            Write-Step "Preserving Codex as a standalone npm command"
            Invoke-Native -Command $npm.Source -Arguments @("install", "-g", "@openai/codex@latest")
        }
        Write-Step "Removing npm-managed Prodex"
        Invoke-Native -Command $npm.Source -Arguments @("uninstall", "-g", "@christiandoxa/prodex")
        return
    }
    if ($Manager -eq "cargo") {
        $cargo = Get-Command cargo -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($null -eq $cargo) {
            throw "cargo is required to remove the unsupported Cargo installation."
        }
        Write-Step "Removing unsupported cargo-managed Prodex"
        Invoke-Native -Command $cargo.Source -Arguments @("uninstall", "prodex")
    }
}

function Prepend-PathEntry {
    param(
        [string]$PathValue,
        [string]$Entry
    )

    $needle = $Entry.TrimEnd("\")
    $segments = @($Entry)
    if (-not [string]::IsNullOrWhiteSpace($PathValue)) {
        $segments += $PathValue.Split(";", [System.StringSplitOptions]::RemoveEmptyEntries) |
            Where-Object { $_.TrimEnd("\") -ine $needle }
    }
    return $segments -join ";"
}

function Set-ActiveRelease {
    param(
        [string]$BinDir,
        [string]$ReleaseDir
    )

    if (Test-Path -LiteralPath $BinDir) {
        $item = Get-Item -LiteralPath $BinDir -Force
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -eq 0) {
            throw "$BinDir already exists and is not a Prodex installer junction."
        }
        Remove-Item -LiteralPath $BinDir -Force
    }
    New-Item -ItemType Junction -Path $BinDir -Target $ReleaseDir | Out-Null
}

Assert-ValidRelease
$Target = Get-Target
$Asset = "prodex-$Target.exe"

if ([string]::IsNullOrWhiteSpace($env:PRODEX_INSTALL_DIR)) {
    if ([string]::IsNullOrWhiteSpace($env:LOCALAPPDATA)) {
        throw "LOCALAPPDATA is required to install Prodex."
    }
    $InstallRoot = Join-Path $env:LOCALAPPDATA "Programs\Prodex"
    $BinDir = Join-Path $InstallRoot "bin"
} else {
    $BinDir = [System.IO.Path]::GetFullPath($env:PRODEX_INSTALL_DIR)
    $InstallRoot = Join-Path (Split-Path -Parent $BinDir) ".prodex"
}
$ReleasesDir = Join-Path $InstallRoot "releases"

if (-not [string]::IsNullOrWhiteSpace($BaseUrlOverride)) {
    $BaseUrl = $BaseUrlOverride.TrimEnd("/", "\")
} elseif ($Release -eq "latest") {
    $BaseUrl = "https://github.com/$Repository/releases/latest/download"
} else {
    $BaseUrl = "https://github.com/$Repository/releases/download/$Release"
}

$ExistingPath = Get-ExistingProdexPath
$Manager = Get-InstallManager -ExistingPath $ExistingPath
$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ("prodex-install-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $TempDir | Out-Null

try {
    $ChecksumsPath = Join-Path $TempDir "SHA256SUMS"
    $DownloadPath = Join-Path $TempDir $Asset
    Write-Step "Downloading Prodex $Release for $Target"
    Copy-Download -Source (Join-DownloadSource -Base $BaseUrl -Leaf "SHA256SUMS") -Destination $ChecksumsPath
    $ExpectedDigest = Get-ExpectedDigest -ManifestPath $ChecksumsPath -AssetName $Asset
    Copy-Download -Source (Join-DownloadSource -Base $BaseUrl -Leaf $Asset) -Destination $DownloadPath
    $ActualDigest = (Get-FileHash -LiteralPath $DownloadPath -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($ActualDigest -ne $ExpectedDigest) {
        throw "Downloaded Prodex checksum did not match. Expected $ExpectedDigest but got $ActualDigest."
    }

    $InstalledVersion = Get-ProdexVersion -BinaryPath $DownloadPath
    if ($Release -ne "latest" -and $InstalledVersion -ne $Release) {
        throw "Downloaded asset version $InstalledVersion did not match requested release $Release."
    }

    New-Item -ItemType Directory -Force -Path $ReleasesDir | Out-Null
    $ReleaseDir = Join-Path $ReleasesDir "$InstalledVersion-$Target"
    $ReleaseBinary = Join-Path $ReleaseDir "prodex.exe"
    if (Test-Path -LiteralPath $ReleaseBinary) {
        $ExistingDigest = (Get-FileHash -LiteralPath $ReleaseBinary -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($ExistingDigest -ne $ExpectedDigest) {
            throw "Existing release $ReleaseDir does not match the published checksum."
        }
    } else {
        $StagingDir = Join-Path $ReleasesDir (".staging." + [guid]::NewGuid().ToString("N"))
        New-Item -ItemType Directory -Path $StagingDir | Out-Null
        Copy-Item -LiteralPath $DownloadPath -Destination (Join-Path $StagingDir "prodex.exe")
        Move-Item -LiteralPath $StagingDir -Destination $ReleaseDir
    }

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $BinDir) | Out-Null
    Set-ActiveRelease -BinDir $BinDir -ReleaseDir $ReleaseDir
    $VisibleBinary = Join-Path $BinDir "prodex.exe"
    if ((Get-ProdexVersion -BinaryPath $VisibleBinary) -ne $InstalledVersion) {
        throw "Installed Prodex command did not resolve to the selected release."
    }

    if (-not $NoPathUpdate) {
        $UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
        $NewUserPath = Prepend-PathEntry -PathValue $UserPath -Entry $BinDir
        if ($NewUserPath -cne $UserPath) {
            [Environment]::SetEnvironmentVariable("Path", $NewUserPath, "User")
            Write-Step "PATH updated for future PowerShell sessions"
        }
        $env:Path = Prepend-PathEntry -PathValue $env:Path -Entry $BinDir
    }

    if (($Manager -eq "npm" -or $Manager -eq "cargo") -and
        ($Migrate -or (Prompt-YesNo "Replace the existing $Manager-managed Prodex with the standalone binary?"))) {
        try {
            Migrate-InstallManager -Manager $Manager
        } catch {
            Write-WarningStep "Standalone Prodex is installed and first on the user PATH, but the old $Manager installation could not be removed while it is running: $($_.Exception.Message)"
        }
    } elseif ($Manager -eq "npm" -or $Manager -eq "cargo") {
        Write-WarningStep "The old $Manager installation remains present. The standalone directory was placed first on the user PATH."
    }

    Write-Step "prodex $InstalledVersion installed at $VisibleBinary"
    Write-Step "Open a new PowerShell window and run: prodex"
} finally {
    Remove-Item -LiteralPath $TempDir -Recurse -Force -ErrorAction SilentlyContinue
}
