param(
    [Parameter(Mandatory = $true)] [string] $BinaryPath,
    [Parameter(Mandatory = $true)] [string] $Version,
    [Parameter(Mandatory = $true)] [string] $Commit,
    [Parameter(Mandatory = $true)] [string] $OutputDirectory,
    [string] $Target = "x86_64-pc-windows-msvc"
)

$ErrorActionPreference = "Stop"
$scriptDirectory = Split-Path -Parent $MyInvocation.MyCommand.Path
$repositoryRoot = (Resolve-Path (Join-Path $scriptDirectory "../..")).Path
$outputDirectory = (New-Item -ItemType Directory -Force -Path $OutputDirectory).FullName
$stageDirectory = Join-Path ([IO.Path]::GetTempPath()) ("azusa-lens-msi-" + [guid]::NewGuid().ToString("N"))
$stageDirectory = (New-Item -ItemType Directory -Force -Path $stageDirectory).FullName

try {
    $binary = (Resolve-Path $BinaryPath).Path
    $stagedBinary = Join-Path $stageDirectory "azusa-lens.exe"
    Copy-Item -LiteralPath $binary -Destination $stagedBinary

    $magick = (Get-Command magick.exe).Source
    $iconSource = Join-Path $repositoryRoot "packaging/assets/com.azusalens.AzusaLens.svg"
    $iconPath = Join-Path $stageDirectory "azusa-lens.ico"
    & $magick $iconSource -background none -define icon:auto-resize=16,32,48,64,128,256 $iconPath

    $wixSource = Join-Path $repositoryRoot "packaging/windows/AzusaLens.wxs"
    $candle = (Get-Command candle.exe).Source
    $light = (Get-Command light.exe).Source
    $wixObject = Join-Path $stageDirectory "AzusaLens.wixobj"
    & $candle "-dProductVersion=$Version" "-dBinaryPath=$stagedBinary" "-dIconPath=$iconPath" `
        "-out" $wixObject $wixSource
    $artifact = Join-Path $outputDirectory ("azusa-lens-{0}-{1}.msi" -f $Version, $Target)
    & $light "-out" $artifact $wixObject

    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $artifact).Hash.ToLowerInvariant()
    $checksumPath = "$artifact.sha256"
    "$hash  $([IO.Path]::GetFileName($artifact))" | Set-Content -Encoding ascii -NoNewline $checksumPath
    $manifest = [ordered]@{
        application = "Azusa Lens"
        application_id = "com.azusalens.AzusaLens"
        version = $Version
        commit = $Commit
        target = $Target
        architecture = ($Target -split '-')[0]
        package = "msi"
        artifact = [IO.Path]::GetFileName($artifact)
        sha256 = $hash
        ocr_model_version = "ppocrv6-tiny-ocr-rs-v2.4.1"
        signed = $false
    } | ConvertTo-Json
    $utf8 = [Text.UTF8Encoding]::new($false)
    [IO.File]::WriteAllText((Join-Path $outputDirectory "manifest.json"), "$manifest`n", $utf8)
}
finally {
    if (Test-Path -LiteralPath $stageDirectory) {
        Remove-Item -LiteralPath $stageDirectory -Recurse -Force
    }
}
