# Aurora Launcher

Launcher de Minecraft: Java Edition em **Tauri 2 (Rust) + React 19 + TypeScript + Tailwind 4**.
Suporta contas **Microsoft (originais)** e **offline**, instâncias isoladas, Java automático
(Adoptium/Temurin), Fabric e skins offline via **CustomSkinLoader**.

## Estrutura

```
laucher/
├── Cargo.toml                    # workspace Rust
├── package.json / vite.config.ts # frontend
├── crates/launcher-core/         # núcleo independente de UI (testável sem Tauri)
│   ├── src/
│   │   ├── lib.rs                # struct Launcher (contexto compartilhado)
│   │   ├── error.rs              # LauncherError (serializa como texto p/ a UI)
│   │   ├── http.rs               # cliente HTTP, retry com backoff exponencial
│   │   ├── paths.rs              # layout em disco + escrita atômica
│   │   ├── settings.rs           # settings.json
│   │   ├── auth/
│   │   │   ├── microsoft.rs      # Device Code → XBL → XSTS → Minecraft Services
│   │   │   ├── offline.rs        # UUID v3 idêntico a UUID.nameUUIDFromBytes
│   │   │   ├── secure_store.rs   # refresh_token no cofre do SO (keyring)
│   │   │   └── accounts.rs       # AccountStore: lista, troca, renovação de tokens
│   │   ├── version/
│   │   │   ├── model.rs          # tipos do manifest e do JSON de versão
│   │   │   ├── rules.rs          # avaliação de rules (os/arch/features)
│   │   │   └── mod.rs            # manifest, download do JSON, merge inheritsFrom
│   │   ├── download/
│   │   │   ├── mod.rs            # Downloader: pool concorrente + SHA-1/SHA-256
│   │   │   ├── hash.rs           # hashing em streaming e verificação local
│   │   │   ├── libraries.rs      # classpath, natives (formato antigo e LWJGL 3)
│   │   │   ├── assets.rs         # asset index + objects (+ assets legados)
│   │   │   └── natives.rs        # extração para natives/<sessão>/
│   │   ├── java/mod.rs           # regra 8/17/21, detecção, download Adoptium
│   │   ├── instance.rs           # instance.json + pastas isoladas
│   │   ├── loaders/fabric.rs     # perfil Fabric via meta.fabricmc.net
│   │   ├── skins/csl.rs          # CustomSkinLoader via Modrinth + config em cascata
│   │   ├── modrinth.rs           # API Modrinth: busca, versões, hashes, updates
│   │   ├── content.rs            # gerenciador de mods/resource packs/shaders
│   │   ├── configs.rs            # editor de config/ e options.txt (com validação)
│   │   ├── optimize.rs           # pacotes de desempenho/PvP, presets, AppCDS
│   │   ├── cosmetics.rs          # guarda-roupa: skins, capas, Mojang, servidor
│   │   └── launch/
│   │       ├── args.rs           # Process Builder: JVM, Aikar's Flags, ${variáveis}
│   │       ├── process.rs        # spawn, pipe de logs, kill, detecção de crash
│   │       └── mod.rs            # orquestração completa do "Jogar"
│   └── tests/pipeline.rs         # testes de integração (offline)
├── crates/cosmetics-server/      # servidor Rust (axum) de skins/capas compartilhadas
├── src-tauri/                    # casca Tauri: comandos + eventos
│   ├── src/commands.rs
│   ├── src/state.rs              # AppState + ponte de eventos (logs em lote)
│   └── tauri.conf.json
└── src/                          # UI React
    ├── App.tsx
    ├── hooks/useLaunchEvents.ts  # progresso, velocidade, logs, saída
    ├── lib/{api,types}.ts        # invoke() tipado
    └── components/               # AccountSwitcher, AddAccountModal, InstanceSidebar,
                                  # InstancePanel (Play + RAM), DownloadProgress, Console…
```

Dados do launcher em `%APPDATA%\AuroraLauncher` (Windows):

```
libraries/  assets/  versions/  runtimes/java-N/  natives/<sessão>/
instances/<id>/instance.json
instances/<id>/.minecraft/{mods,saves,config,resourcepacks,…}
accounts.json   settings.json
```

## Scripts (duplo clique no Windows)

| Arquivo | O que faz |
|---|---|
| `instalar.bat` | confere Node/Rust/WebView2 e instala as dependências |
| `testar.bat` | abre o launcher em modo desenvolvimento (hot reload) |
| `configurar-atualizacao.bat` | gera a chave de assinatura e aponta o updater para o seu repositório |
| `gerar-exe.bat` | gera o instalador e o `.exe` portátil assinados em `release\` |
| `publicar-atualizacao.bat` | sobe a versão, compila, assina e cria o Release no GitHub |

## Atualização automática pelo GitHub

1. Crie um repositório **público** no GitHub (ex.: `lucasshl/aurora-launcher`).
2. Instale o [GitHub CLI](https://cli.github.com) e rode `gh auth login`.
3. Rode `configurar-atualizacao.bat` uma vez. Ele cria a chave em `.keys\` e grava
   a chave pública e o endereço do `latest.json` no `tauri.conf.json`.
4. Gere e distribua a primeira versão com `gerar-exe.bat`. As instalações feitas com ela já
   sabem onde procurar atualizações.
5. Para lançar uma atualização: `publicar-atualizacao.bat`. Ele pergunta a nova versão
   e as notas, compila, assina e publica o Release com o `latest.json`.

Ao abrir, o launcher consulta
`https://github.com/<usuario>/<repo>/releases/latest/download/latest.json`. Se a versão
for maior, mostra as notas e o botão **Atualizar agora**, baixa o instalador, confere a
assinatura, instala e reinicia. Também dá para verificar em Configurações.

Opção 2 do `publicar-atualizacao.bat`: cria só a tag `vX.Y.Z` e o workflow
`ci/release.yml` compila no GitHub Actions (mova-o para `.github/workflows/release.yml` antes do primeiro push) (precisa dos secrets
`TAURI_SIGNING_PRIVATE_KEY` e `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, que o
`configurar-atualizacao.bat` cadastra via `gh`).

> Faça backup de `.keys\`. Se perder a chave, as instalações existentes deixam de
> aceitar atualizações e todo mundo precisa reinstalar manualmente.

## Rodando

Pré-requisitos (Windows): [Rust](https://rustup.rs), Node 20+, e o WebView2 (já vem no Windows 10/11).

```bash
npm install
npm run tauri dev       # desenvolvimento com hot reload
npm run tauri build     # instalador .msi/.exe em target/release/bundle
cargo test -p launcher-core
```

## Login Microsoft: o que você precisa configurar

1. Em **portal.azure.com → App registrations → New registration**, escolha
   *Personal Microsoft accounts only*.
2. Em **Authentication**, ative **Allow public client flows** (necessário para Device Code).
3. Copie o **Application (client) ID** e cole em *Configurações* no launcher
   (ou compile com `AURORA_MS_CLIENT_ID=<id> npm run tauri build`).
4. **Importante:** apps Azure novos recebem **403** em `login_with_xbox` até a Mojang
   aprovar o acesso à API do Minecraft. Peça a liberação pelo formulário oficial da
   Mojang/Microsoft para launchers de terceiros. Antes disso, o fluxo vai até o XSTS e
   para com uma mensagem explicando o 403.

O `refresh_token` fica no Gerenciador de Credenciais do Windows (Keychain no macOS);
o token do jogo só existe em memória e é renovado automaticamente.

## Contas offline e skins

- UUID = `UUID.nameUUIDFromBytes("OfflinePlayer:" + nick)`, idêntico ao que servidores
  `online-mode=false` usam.
- Em instâncias **Fabric/Forge/NeoForge** com conta offline, o launcher baixa o
  CustomSkinLoader do Modrinth (com dependências obrigatórias) e grava
  `CustomSkinLoader/CustomSkinLoader.json` com a cascata
  **LocalSkin → API customizada → Ely.by → Mojang**.
- O botão de imagem no painel da instância copia um PNG para
  `LocalSkin/skins/<nick>.png`.

## Modloaders

| Loader   | Status |
|----------|--------|
| Vanilla  | completo |
| Fabric   | completo (instala o perfil automaticamente) |
| Forge / NeoForge | executa perfis já instalados. A instalação automática exige rodar os *processors* do instalador oficial; rode o instalador apontando para a pasta de dados e informe o `versionId` gerado no `instance.json`. |

## Todas as versões

O seletor de versões lê o manifest oficial inteiro: releases, snapshots, beta e alpha
(desde a rd-132211), com filtro por texto. Versões antigas usam os argumentos legados,
assets "virtuais" e Java 8 automaticamente.

## Desempenho (meta: 50+ FPS em PC fraco)

Na aba **Otimização** de cada instância (ou marcando "Otimizar automaticamente" ao criar):

| Mod | Ganho |
|---|---|
| Sodium (Embeddium no Forge antigo) | renderização, normalmente o maior ganho de FPS |
| Lithium, BadOptimizations, Starlight* | lógica de ticks, IA, luz |
| FerriteCore, ModernFix | RAM e tempo de abertura |
| C2ME, Noisium | geração e carregamento de chunks em paralelo |
| Entity Culling, More Culling, ImmediatelyFast | menos coisa desenhada por frame |
| Krypton, Dynamic FPS, LazyDFU* | rede, segundo plano, abertura |
| Chunky (opcional) | pré-gera o mapa: depois os chunks carregam na hora |

\* só existem para versões antigas; o launcher detecta e pula sozinho.

Além disso:
- **Preset gráfico** (PC fraco / equilibrado / qualidade) gravado no `options.txt`
  no formato certo para cada versão, e nas opções do Sodium.
- **AppCDS** (Java 19+): a JVM salva as classes carregadas em
  `instances/<id>/.cache/aurora-cds.jsa` e reaproveita nas próximas aberturas.
- G1GC com as Aikar's Flags e alerta de RAM em PCs com pouca memória.

O tempo de abertura depende do disco, da CPU e da quantidade de mods. Com ModernFix e
AppCDS em SSD, a abertura a partir da 2ª vez costuma ficar na casa de poucos segundos,
mas não existe garantia de 5 s em qualquer PC.

**Por que não "converter o jogo para Rust":** o Minecraft roda inteiro na JVM. Não dá
para trocar partes do `.jar` por código nativo e o resto continuar funcionando, e
descompilar/redistribuir o código da Mojang viola o EULA. Os mods acima são o jeito
real de reescrever as partes lentas (renderização, chunks, luz) por dentro do jogo.

## Pacote PvP

Freelook (câmera livre, segure Alt), Zoomify (zoom), Toggle Sprint, Armor HUD,
Status Effect Timer, AppleSkin, BetterF3 e Iris (shaders). São mods open-source do
Modrinth com funções parecidas às dos clients PvP, instaladas com dependências.

## Gerenciador de mods

- **Mods**: lista os instalados com ícone/nome/versão lidos de dentro do JAR
  (Fabric, Quilt, Forge, NeoForge), liga/desliga (`.disabled`), remove, verifica e
  aplica atualizações pelo SHA-1 no Modrinth. Também gerencia resource packs e shaders.
- **Explorar**: busca no Modrinth filtrada pela versão e pelo loader da instância e
  instala com todas as dependências obrigatórias.
- **Configs**: editor de `config/` e `options.txt`. Valida JSON/TOML antes de salvar
  e guarda um `.bak`.

## Cosméticos (guarda-roupa)

- Biblioteca de skins (64x64, 64x32 e HD) e capas com **preview 3D** (skinview3d),
  andando ou parado, capa ou élitra.
- **Conta original**: envia a skin para a Mojang e troca entre as capas oficiais.
- **Conta offline**: aplica skin/capa/élitra via CustomSkinLoader ao clicar em Jogar.
- **Servidor de cosméticos** (`crates/cosmetics-server`): para todos os jogadores do
  launcher se verem com as skins/capas uns dos outros, como num client PvP. Rode na
  sua VPS:

  ```bash
  COSMETICS_ADDR=0.0.0.0:8787 COSMETICS_DATA=/var/lib/aurora-cosmetics \
    cargo run --release -p cosmetics-server
  ```

  Coloque atrás de HTTPS (Caddy/Nginx/Cloudflare) e informe a URL em Configurações.
  O primeiro a publicar um nick vira o dono dele (o token fica no cofre do SO). Em
  instâncias com mods o CustomSkinLoader busca nesse servidor antes de Ely.by e Mojang.
  Cosméticos 3D (chapéus, asas customizadas) exigiriam um mod client próprio e não
  estão incluídos.

## Robustez

- Downloads gravam em `.part`, calculam o hash durante o streaming e só renomeiam se
  conferir; falhas de rede, HTTP 5xx/429 e hash inválido têm retry com backoff.
- Arquivos locais corrompidos são detectados por SHA-1 e baixados de novo.
- JSONs corrompidos (`accounts.json`, versões) são isolados e regenerados.
- Sem internet, o manifest em cache permite abrir versões já baixadas.
- No Windows, classpaths enormes usam `@argfile` (Java 9+) para não estourar o limite
  de 32 767 caracteres da linha de comando.
