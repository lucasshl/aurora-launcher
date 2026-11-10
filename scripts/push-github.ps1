# Sobe o projeto para o GitHub (cria o repositório Git local na primeira vez)
param([string]$Message = '')
. "$PSScriptRoot\common.ps1"
# git escreve progresso no stderr; com 'Stop' o PowerShell 5.1 trataria isso como erro
$ErrorActionPreference = 'Continue'

$repo = Get-UpdateRepo
if (-not $repo) { $repo = 'lucasshl/aurora-launcher' }
$url = "https://github.com/$repo.git"

if (-not (Has 'git')) {
    Step 'Git não encontrado: instalando'
    if (Has 'winget') { & winget install --id Git.Git -e --accept-package-agreements --accept-source-agreements }
    $env:Path = [Environment]::GetEnvironmentVariable('Path', 'Machine') + ';' + [Environment]::GetEnvironmentVariable('Path', 'User')
    if (-not (Has 'git')) { Start-Process 'https://git-scm.com/download/win'; Fail 'instale o Git e rode este script de novo.' }
    Ok 'Git instalado.'
}

# O workflow de release precisa ficar em .github\workflows para o GitHub Actions rodar
if (Test-Path 'ci\release.yml') {
    New-Item -ItemType Directory -Force '.github\workflows' | Out-Null
    Copy-Item 'ci\release.yml' '.github\workflows\release.yml' -Force
}

if (-not (Test-Path '.git')) {
    Step 'Criando o repositório Git local'
    Invoke-Checked 'git' @('init', '-b', 'main')
}
# Identidade só deste repositório, com o e-mail "noreply" do GitHub (não expõe o seu e-mail)
if (-not (& git config user.name)) { & git config user.name ($repo.Split('/')[0]) }
if (-not (& git config user.email)) { & git config user.email "$($repo.Split('/')[0])@users.noreply.github.com" }

$current = & git remote get-url origin 2>$null
if (-not $current) { & git remote add origin $url }
elseif ($current -ne $url) { & git remote set-url origin $url }
Ok "Destino: $url"

Step 'Preparando os arquivos'
Invoke-Checked 'git' @('add', '-A')

# Trava de segurança: chaves e o Client ID nunca podem ir para o GitHub
$secrets = & git diff --cached --name-only | Where-Object { $_ -match '^\.keys/|\.key$|\.password$|ms-client-id' }
$tracked = & git ls-files -- '.keys' | Where-Object { $_ }
if ($secrets -or $tracked) {
    & git reset -q -- '.keys' 2>$null
    Fail "arquivos secretos estavam indo para o Git: $(@($secrets) + @($tracked) -join ', '). Nada foi enviado."
}

& git diff --cached --quiet
if ($LASTEXITCODE -ne 0) {
    if (-not $Message) { $Message = "Atualização $(Get-Date -Format 'yyyy-MM-dd HH:mm')" }
    Invoke-Checked 'git' @('commit', '-q', '-m', $Message)
    Ok "Commit criado: $Message"
} else {
    Ok 'Nenhuma mudança nova nos arquivos.'
}

Step 'Juntando com o que já existe no GitHub'
& git fetch -q origin
if ($LASTEXITCODE -eq 0) {
    & git rev-parse --verify -q origin/main *> $null
    if ($LASTEXITCODE -eq 0) {
        & git merge-base --is-ancestor origin/main HEAD
        if ($LASTEXITCODE -ne 0) {
            Invoke-Checked 'git' @('merge', 'origin/main', '--allow-unrelated-histories', '--no-edit', '-q')
        }
        # Arquivo de teste "git" criado direto no site: não faz parte do launcher
        if ((Test-Path 'git' -PathType Leaf) -and ((Get-Item 'git').Length -lt 64)) {
            & git rm -q 'git'
            Invoke-Checked 'git' @('commit', '-q', '-m', 'Remove arquivo de teste')
        }
    }
}

Step 'Enviando para o GitHub (na primeira vez abre o navegador para você entrar na conta)'
Invoke-Checked 'git' @('push', '-u', 'origin', 'main')
Ok "Pronto: https://github.com/$repo"
