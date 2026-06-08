#!/usr/bin/env pwsh
# Windows packaging script: builds release binary and bundles into .zip

param(
    [string]$OutDir = "build/windows"
)

$ErrorActionPreference = "Stop"

Write-Host "Building Rustix Engine (Windows Release)..."
cargo build --release --workspace

$TargetDir = "target/release"
$Dest = "$OutDir/rustix-engine"
New-Item -ItemType Directory -Force -Path $Dest | Out-Null

Write-Host "Copying binaries and assets..."
Copy-Item "$TargetDir/rustix-runtime.exe" "$Dest/" -ErrorAction SilentlyContinue
Copy-Item "$TargetDir/*.dll" "$Dest/" -ErrorAction SilentlyContinue
Copy-Item "assets" "$Dest/" -Recurse -ErrorAction SilentlyContinue
Copy-Item "shaders" "$Dest/" -Recurse -ErrorAction SilentlyContinue

$ZipPath = "$OutDir/rustix-engine-windows.zip"
Write-Host "Creating $ZipPath ..."
Compress-Archive -Path "$Dest/*" -DestinationPath $ZipPath -Force

Write-Host "Windows package ready: $ZipPath"
