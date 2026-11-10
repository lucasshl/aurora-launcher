# Configura o sistema de atualização: gera a chave de assinatura e aponta para o seu repositório.
. "$PSScriptRoot\common.ps1"

# O comando 'npx tauri' precisa do CLI local em node_modules
if (-not (Test-Path 'node_modules')) { Step 'Instalando dependências primeiro'; & "$PSScriptRoot\setup.ps1"; if ($LASTEXITCODE -ne 0) { exit 1 } }

Step 'Repositório do GitHub onde as atualizações serão publicadas'
$current = Get-UpdateRepo
$default = if ($current) { $current } else { 'lucasshl/aurora-launcher' }
$repo = Read-Host "Informe usuario/repositorio [$default]"
if ([string]::IsNullOrWhiteSpace($repo)) { $repo = $default }
if ($repo -notmatch '^[\w.-]+/[\w.-]+$') { Fail "formato inválido: $repo (use usuario/repositorio)" }

Step 'Chave de assinatura das atualizações'
if (Test-Path $KeyFile) {
    Ok "Chave existente reaproveitada: $KeyFile"
} else {
    New-Item -ItemType Directory -Force (Split-Path $KeyFile) | Out-Null
    # Senha aleatória guardada ao lado da chave: evita o prompt interativo de senha no build
    $pw = -join ((48..57) + (65..90) + (97..122) | Get-Random -Count 32 | ForEach-Object { [char]$_ })
    Write-Text $PwFile $pw
    Invoke-Checked 'npx' @('tauri', 'signer', 'generate', '-w', $KeyFile, '-p', $pw, '--ci')
    Ok "Chave criada em $KeyFile"
}
$pub = (Read-Text "$KeyFile.pub").Trim()

Step 'Gravando chave pública e endpoint no tauri.conf.json'
$conf = Read-Text $ConfFile
$conf = [regex]::Replace($conf, '"pubkey"\s*:\s*"[^"]*"', "`"pubkey`": `"$pub`"")
$conf = [regex]::Replace($conf, 'https://github\.com/[^/"]+/[^/"]+/releases/latest/download/latest\.json',
    "https://github.com/$repo/releases/latest/download/latest.json")
Write-Text $ConfFile $conf
Ok "Endpoint: https://github.com/$repo/releases/latest/download/latest.json"

if (Has 'gh') {
    $answer = Read-Host "`nCadastrar a chave como secret do GitHub Actions em $repo? (s/N)"
    if ($answer -match '^[sS]') {
        Get-Content $KeyFile -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY --repo $repo
        if (Test-Path $PwFile) { Get-Content $PwFile -Raw | gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD --repo $repo }
        if ($LASTEXITCODE -eq 0) { Ok 'Secrets cadastrados.' } else { Warn 'Não foi possível cadastrar (rode gh auth login).' }
    }
} else {
    Warn 'GitHub CLI (gh) não encontrado: publicar-atualizacao.bat precisa dele para criar releases.'
    Warn 'Instale em https://cli.github.com e rode "gh auth login".'
}

Write-Host ''
Write-Host '  IMPORTANTE: faça backup da pasta .keys\ (fora do Git).' -ForegroundColor Yellow
Write-Host '  Sem essa chave, quem já instalou o launcher NÃO aceita novas atualizações.' -ForegroundColor Yellow
Write-Host '  O repositório precisa ser PÚBLICO para o launcher conseguir baixar as atualizações.' -ForegroundColor Yellow
