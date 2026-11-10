# Compila o mod Aurora Cosmetics (itens 3D) e entrega o .jar para o launcher.
. "$PSScriptRoot\common.ps1"

$ModDir = Join-Path $Root 'mods\aurora-cosmetics'
$JdkDir = Join-Path $Root '.tools\jdk-25'
$LogFile = Join-Path $ModDir 'build.log'

function Test-Jdk25($home_) {
    if (-not $home_) { return $false }
    $javac = Join-Path $home_ 'bin\javac.exe'
    if (-not (Test-Path $javac)) { return $false }
    $v = & $javac -version 2>&1 | Out-String
    return $v -match 'javac 2[5-9]'
}

Step 'Procurando um JDK 25 (o Minecraft 26.3 exige Java 25 para compilar)'
$jdk = $null
if (Test-Jdk25 $env:JAVA_HOME) { $jdk = $env:JAVA_HOME }
elseif (Test-Jdk25 $JdkDir) { $jdk = $JdkDir }
else {
    Step 'Baixando JDK 25 portátil (Eclipse Temurin, ~190 MB)'
    $api = 'https://api.adoptium.net/v3/assets/latest/25/hotspot?architecture=x64&image_type=jdk&os=windows&vendor=eclipse'
    $pkg = (Invoke-RestMethod $api)[0].binary.package
    $zip = Join-Path $env:TEMP $pkg.name
    Invoke-WebRequest $pkg.link -OutFile $zip -UseBasicParsing
    $hash = (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
    if ($pkg.checksum -and $hash -ne $pkg.checksum.ToLower()) { Fail 'o JDK baixado está corrompido (SHA-256 não confere)' }
    $tmp = Join-Path $Root '.tools\jdk-extract'
    if (Test-Path $tmp) { Remove-Item $tmp -Recurse -Force }
    Expand-Archive $zip $tmp -Force
    $inner = Get-ChildItem $tmp -Directory | Select-Object -First 1
    if (Test-Path $JdkDir) { Remove-Item $JdkDir -Recurse -Force }
    New-Item -ItemType Directory -Force (Split-Path $JdkDir) | Out-Null
    Move-Item $inner.FullName $JdkDir
    Remove-Item $tmp -Recurse -Force
    Remove-Item $zip -Force
    if (-not (Test-Jdk25 $JdkDir)) { Fail 'o JDK 25 não ficou utilizável' }
    $jdk = $JdkDir
}
Ok "JDK: $jdk"
$env:JAVA_HOME = $jdk
$env:Path = "$jdk\bin;$env:Path"

Step 'Compilando o mod (a 1ª vez baixa o Minecraft 26.3 e o Gradle: vários minutos)'
Push-Location $ModDir
# No PowerShell 5.1, redirecionar stderr de um programa com ErrorActionPreference=Stop aborta o script
$ErrorActionPreference = 'Continue'
try {
    & .\gradlew.bat build --no-daemon --console=plain --stacktrace 2>&1 | Tee-Object -FilePath $LogFile
    $code = $LASTEXITCODE
} finally {
    Pop-Location
    $ErrorActionPreference = 'Stop'
}
if ($code -ne 0) {
    Write-Host ''
    Warn "A compilação falhou. O log completo está em mods\aurora-cosmetics\build.log"
    Warn 'Avise no chat que eu leio esse arquivo e corrijo.'
    exit 1
}

$jar = Get-ChildItem (Join-Path $ModDir 'build\libs') -Filter 'aurora-cosmetics-*.jar' |
    Where-Object { $_.Name -notmatch 'sources' } | Sort-Object LastWriteTime -Descending | Select-Object -First 1
if (-not $jar) { Fail 'o .jar do mod não foi gerado' }

$dest = Join-Path $env:APPDATA 'AuroraLauncher\bundled-mods'
New-Item -ItemType Directory -Force $dest | Out-Null
Copy-Item $jar.FullName (Join-Path $dest 'aurora-cosmetics.jar') -Force
Ok "Mod pronto: $($jar.Name)"
Ok 'O launcher instala sozinho nas instâncias Fabric 26.3 ao clicar em Jogar.'
