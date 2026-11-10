# Publica uma nova versão: atualiza o número da versão, compila, assina e cria o Release no GitHub.
. "$PSScriptRoot\common.ps1"

$repo = Get-UpdateRepo
if (-not $repo) { Fail 'rode configurar-atualizacao.bat antes de publicar.' }

$current = Get-AppVersion
$parts = $current.Split('.')
$suggested = "$($parts[0]).$($parts[1]).$([int]$parts[2] + 1)"
Write-Host "Versão atual: $current" -ForegroundColor Cyan
$version = Read-Host "Nova versão [$suggested]"
if ([string]::IsNullOrWhiteSpace($version)) { $version = $suggested }
if ($version -notmatch '^\d+\.\d+\.\d+$') { Fail "versão inválida: $version (use X.Y.Z)" }
if ([version]$version -le [version]$current) { Fail "a nova versão precisa ser maior que $current" }

Write-Host 'Notas da atualização (o que mudou). Linha vazia termina:' -ForegroundColor Cyan
$lines = @()
while ($true) { $l = Read-Host; if ([string]::IsNullOrEmpty($l)) { break }; $lines += $l }
$notes = ($lines -join "`n")

Write-Host "`nComo publicar?" -ForegroundColor Cyan
Write-Host '  1) Compilar aqui e enviar com o GitHub CLI (recomendado)'
Write-Host '  2) Criar tag e deixar o GitHub Actions compilar'
$mode = Read-Host 'Escolha [1]'
if ([string]::IsNullOrWhiteSpace($mode)) { $mode = '1' }

Step "Atualizando versão $current -> $version"
$re = [regex]'"version"\s*:\s*"[^"]+"'
foreach ($f in @($ConfFile, (Join-Path $Root 'package.json'))) {
    Write-Text $f ($re.Replace((Read-Text $f), "`"version`": `"$version`"", 1))
}
$cargo = Join-Path $Root 'Cargo.toml'
Write-Text $cargo (([regex]'(?m)^version = "[^"]+"').Replace((Read-Text $cargo), "version = `"$version`"", 1))
Ok 'package.json, tauri.conf.json e Cargo.toml atualizados'

if ($mode -eq '2') {
    if (-not (Has 'git')) { Fail 'git não encontrado' }
    Invoke-Checked 'git' @('add', '-A')
    Invoke-Checked 'git' @('commit', '-m', "release: v$version")
    Invoke-Checked 'git' @('tag', '-a', "v$version", '-m', $(if ($notes) { $notes } else { "v$version" }))
    Invoke-Checked 'git' @('push', '--follow-tags')
    Ok "Tag v$version enviada. Acompanhe em https://github.com/$repo/actions"
    exit 0
}

if (-not (Has 'gh')) { Fail 'GitHub CLI (gh) não encontrado. Instale em https://cli.github.com e rode "gh auth login".' }
& "$PSScriptRoot\build.ps1" -Notes $notes
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

Step "Criando o Release v$version em $repo"
$out = Join-Path $Root 'release'
$assets = @(
    (Join-Path $out "AuroraLauncher_${version}_x64-setup.exe"),
    (Join-Path $out "AuroraLauncher_${version}_portable.exe"),
    (Join-Path $out 'latest.json')
)
$notesArg = if ($notes) { $notes } else { "Aurora Launcher $version" }
Invoke-Checked 'gh' (@('release', 'create', "v$version", '--repo', $repo, '--title', "Aurora Launcher $version", '--notes', $notesArg) + $assets)

Ok "Publicado! Quem tiver o launcher aberto vai ver o aviso de atualização na próxima inicialização."
Ok "https://github.com/$repo/releases/tag/v$version"
