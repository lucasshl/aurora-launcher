# Funções compartilhadas pelos scripts do Aurora Launcher.
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root
$Utf8 = New-Object System.Text.UTF8Encoding $false   # sem BOM (o Tauri não aceita BOM no JSON)
$KeyFile = Join-Path $Root '.keys\aurora-updater.key'
$PwFile = Join-Path $Root '.keys\aurora-updater.password'
$ClientIdFile = Join-Path $Root '.keys\ms-client-id.txt'

# Embute o Client ID do Azure no executável (fica fora do Git, em .keys\)
function Set-MsClientIdEnv {
    if (Test-Path $ClientIdFile) {
        $env:AURORA_MS_CLIENT_ID = (Read-Text $ClientIdFile).Trim()
        Ok 'Client ID da Microsoft embutido no build'
    } else {
        Write-Host '  Aviso: .keys\ms-client-id.txt não encontrado; o login Microsoft ficará desativado neste build.' -ForegroundColor Yellow
    }
}
$ConfFile = Join-Path $Root 'src-tauri\tauri.conf.json'

function Read-Text($path) { [IO.File]::ReadAllText($path) }
function Write-Text($path, $text) { [IO.File]::WriteAllText($path, $text, $Utf8) }

function Step($msg) { Write-Host "`n==> $msg" -ForegroundColor Cyan }
function Ok($msg) { Write-Host "    $msg" -ForegroundColor Green }
function Warn($msg) { Write-Host "    $msg" -ForegroundColor Yellow }
function Fail($msg) { Write-Host "`nERRO: $msg" -ForegroundColor Red; exit 1 }

function Has($cmd) { [bool](Get-Command $cmd -ErrorAction SilentlyContinue) }

function Get-AppVersion {
    $m = [regex]::Match((Read-Text $ConfFile), '"version"\s*:\s*"([^"]+)"')
    if (-not $m.Success) { Fail 'versão não encontrada em tauri.conf.json' }
    $m.Groups[1].Value
}

# Retorna "usuario/repo" a partir do endpoint configurado no tauri.conf.json
function Get-UpdateRepo {
    $m = [regex]::Match((Read-Text $ConfFile), 'https://github\.com/([^/"]+/[^/"]+)/releases/latest/download/latest\.json')
    if (-not $m.Success -or $m.Groups[1].Value -eq 'SEU_USUARIO/SEU_REPO') { return $null }
    $m.Groups[1].Value
}

function Invoke-Checked($exe, [string[]]$argv) {
    & $exe @argv
    if ($LASTEXITCODE -ne 0) { Fail "$exe $($argv -join ' ') falhou (código $LASTEXITCODE)" }
}

# Busca o Visual C++ Build Tools instalado (necessário para o Rust MSVC linkar no Windows)
function Find-VcTools {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (-not (Test-Path $vswhere)) { return $null }
    & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
}

# Garante Rust com toolchain MSVC + Build Tools do Visual Studio.
# O toolchain GNU (x86_64-pc-windows-gnu) falha com "dlltool.exe: program not found".
function Ensure-MsvcToolchain {
    if ($env:OS -ne 'Windows_NT') { return }

    if (-not (Find-VcTools)) {
        Step 'Instalando Visual Studio Build Tools (C++), necessário para compilar no Windows'
        Warn 'São cerca de 2-3 GB. O Windows vai pedir permissão de administrador.'
        if (Has 'winget') {
            & winget install --id Microsoft.VisualStudio.2022.BuildTools -e `
                --accept-package-agreements --accept-source-agreements `
                --override '--passive --wait --norestart --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended'
        }
        if (-not (Find-VcTools)) {
            Warn 'Não deu para instalar automaticamente. Abrindo a página de download:'
            Warn 'marque "Desenvolvimento para desktop com C++", instale e rode este script de novo.'
            Start-Process 'https://visualstudio.microsoft.com/pt-br/visual-cpp-build-tools/'
            Fail 'Build Tools do Visual Studio não encontrados.'
        }
        Ok 'Build Tools instalados.'
    }

    $triple = ((rustc -vV) | Where-Object { $_ -like 'host:*' }) -replace '^host:\s*', ''
    if ($triple -like '*windows-gnu*') {
        Step "Rust está usando o toolchain GNU ($triple); trocando para MSVC"
        Invoke-Checked 'rustup' @('toolchain', 'install', 'stable-x86_64-pc-windows-msvc', '--profile', 'minimal')
        Invoke-Checked 'rustup' @('default', 'stable-x86_64-pc-windows-msvc')
        Ok "Rust agora: $(((rustc -vV) | Where-Object { $_ -like 'host:*' }) -replace '^host:\s*', '')"
    }
}
