# Verifica pré-requisitos e instala as dependências do projeto.
. "$PSScriptRoot\common.ps1"

Step 'Verificando Node.js'
if (-not (Has 'node')) {
    Warn 'Node.js não encontrado. Abrindo a página de download (instale a versão LTS e rode de novo).'
    Start-Process 'https://nodejs.org/pt-br/download'
    exit 1
}
$nodeMajor = [int]((node --version).TrimStart('v').Split('.')[0])
if ($nodeMajor -lt 20) { Fail "Node $nodeMajor encontrado; é preciso a versão 20 ou mais nova." }
Ok "Node $(node --version)"

Step 'Verificando Rust'
if (-not (Has 'cargo')) {
    Warn 'Rust não encontrado. Abrindo rustup.rs: baixe o rustup-init.exe, aceite as opções padrão'
    Warn '(ele instala também as Build Tools do Visual Studio) e rode este script de novo.'
    Start-Process 'https://rustup.rs'
    exit 1
}
Ok (cargo --version)
Ensure-MsvcToolchain

Step 'Verificando WebView2'
$wv = Get-ItemProperty 'HKLM:\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}' -ErrorAction SilentlyContinue
if ($wv) { Ok "WebView2 $($wv.pv)" } else { Warn 'WebView2 não detectado (já vem no Windows 10/11 atualizado).' }

Step 'Instalando dependências do frontend (npm)'
if (Test-Path 'package-lock.json') { Invoke-Checked 'npm' @('ci') } else { Invoke-Checked 'npm' @('install') }

Step 'Baixando dependências do Rust (primeira vez demora alguns minutos)'
Invoke-Checked 'cargo' @('fetch')

Ok "`nTudo pronto! Use testar.bat para abrir em modo desenvolvimento."
