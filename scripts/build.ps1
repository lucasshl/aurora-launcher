# Gera o instalador .exe assinado e o latest.json em .\release\
param([string]$Notes = '')
. "$PSScriptRoot\common.ps1"

if (-not (Test-Path 'node_modules')) { Step 'Dependências não instaladas: rodando setup'; & "$PSScriptRoot\setup.ps1" }
if (-not (Test-Path $KeyFile)) { Fail 'chave de atualização não encontrada. Rode configurar-atualizacao.bat primeiro.' }

Ensure-MsvcToolchain
$version = Get-AppVersion
$repo = Get-UpdateRepo
Step "Compilando Aurora Launcher $version (modo release, pode levar vários minutos)"

$env:TAURI_SIGNING_PRIVATE_KEY = (Read-Text $KeyFile).Trim()
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = if (Test-Path $PwFile) { (Read-Text $PwFile).Trim() } else { '' }
Set-MsClientIdEnv
Invoke-Checked 'npm' @('run', 'tauri', 'build')

Step 'Organizando arquivos em .\release'
$nsisDir = Join-Path $Root 'target\release\bundle\nsis'
$installer = Get-ChildItem $nsisDir -Filter "*_${version}_*-setup.exe" | Select-Object -First 1
if (-not $installer) { Fail "instalador da versão $version não encontrado em $nsisDir" }
$sigFile = "$($installer.FullName).sig"
if (-not (Test-Path $sigFile)) { Fail 'assinatura (.sig) não foi gerada; confira a chave em .keys\' }

$out = Join-Path $Root 'release'
New-Item -ItemType Directory -Force $out | Out-Null
# Nome sem espaços: o GitHub troca espaços por pontos e quebraria a URL do latest.json
$assetName = "AuroraLauncher_${version}_x64-setup.exe"
Copy-Item $installer.FullName (Join-Path $out $assetName) -Force
Copy-Item (Join-Path $Root 'target\release\aurora-launcher.exe') (Join-Path $out "AuroraLauncher_${version}_portable.exe") -Force

if ($repo) {
    $manifest = [ordered]@{
        version   = $version
        notes     = $Notes
        pub_date  = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')
        platforms = [ordered]@{
            'windows-x86_64' = [ordered]@{
                signature = (Read-Text $sigFile).Trim()
                url       = "https://github.com/$repo/releases/download/v$version/$assetName"
            }
        }
    }
    Write-Text (Join-Path $out 'latest.json') ($manifest | ConvertTo-Json -Depth 5)
    Ok 'latest.json gerado'
} else {
    Warn 'Repositório de atualização não configurado: latest.json não foi gerado.'
}

Ok "Pronto! Arquivos em $out"
Get-ChildItem $out | Format-Table Name, @{ n = 'Tamanho (MB)'; e = { [math]::Round($_.Length / 1MB, 1) } } -AutoSize
