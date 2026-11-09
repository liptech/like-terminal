# MEMORY — Like Terminal

Memória técnica do projeto para retomada rápida (humana ou por agente).

## O que é

Clone do XShell para Linux: emulador de terminal GTK4/libadwaita com abas, painéis
divididos, gerenciador de sessões, conexões SSH (senha, chave, agent, jump host),
realce de saída e temas. Escrito em Rust.

- App ID: `io.github.liketerminal.LikeTerminal`
- Binário: `like-terminal`
- Auxiliar: `like-terminal-relay` (filtro de PTY que colore palavras-chave)
- Versão atual: 0.1.0
- Licença: GPL-3.0-or-later

## Estrutura

```
crates/core   lt-core   modelos, store JSON atômico, keyring, validação (sem GTK)
crates/ssh    lt-ssh    montagem do argv do ssh, quick connect, probe TCP
crates/relay  lt-relay  relay PTY + filtro de cores (binário like-terminal-relay)
crates/app    like-terminal  UI GTK4/libadwaita, VTE, janelas, preferências
data/         CSS, ícone SVG, .desktop, metainfo, logos da marca Liptech
packaging/    build-deb.sh
```

## Compilar e rodar

Dependências de sistema (Ubuntu 24.04): GTK 4.14, libadwaita 1.5, VTE GTK4 0.76,
libsecret, libdbus, openssh-client, Rust ≥ 1.70.

Nesta máquina o VTE e o dbus dev não estão instalados no sistema; foram extraídos em
`~/.local/vte`. Sempre exportar antes de compilar:

```bash
export PATH=$HOME/.cargo/bin:$PATH
export PKG_CONFIG_PATH=$HOME/.local/vte/usr/lib/x86_64-linux-gnu/pkgconfig
export LD_LIBRARY_PATH=$HOME/.local/vte/usr/lib/x86_64-linux-gnu

cargo build --release
cargo run
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all
```

## Empacotamento

- **.deb**: `./packaging/build-deb.sh` → `build-deb/like-terminal_<versão>_amd64.deb`
  (instala `like-terminal`, `like-terminal-relay`, desktop, ícone, metainfo e docs).
  Reinstalar: `sudo apt install --reinstall ./build-deb/like-terminal_0.1.0_amd64.deb`.
- **Flatpak canônico**: `io.github.liketerminal.LikeTerminal.json` (runtime GNOME 51,
  módulo vte 0.84 com `-Dlibdir=lib`, sdk-extension rust-stable). Feito para o Flathub.
- **Flatpak local**: `flatpak-local.json` monta o toolchain Rust do host via bind-mount,
  porque o Flatpak 1.14 do Ubuntu não resolve a extensão `rust-stable//25.08`.
  ```bash
  PATH=/tmp/opencode/as/usr/bin:/tmp/opencode/eu/usr/bin:$PATH \
  LD_LIBRARY_PATH=/tmp/opencode/as/usr/lib/x86_64-linux-gnu \
  /tmp/opencode/fb/usr/bin/flatpak-builder --force-clean --user --install \
    --disable-rofiles-fuse --state-dir=$PWD/.flatpak-builder build-dir flatpak-local.json
  ```
  (`/tmp/opencode/eu` = elfutils, `/tmp/opencode/as` = wrapper do appstreamcli compose.)
- **Bundle portátil**: `flatpak build-bundle --runtime-repo=https://dl.flathub.org/repo/flathub.flatpakrepo ~/.local/share/flatpak/repo like-terminal.flatpak io.github.liketerminal.LikeTerminal master`

## Dados do usuário

- Sessões/perfis: `~/.config/like-terminal/sessions.json` (gravação atômica tmp+rename)
- Estado da UI: `~/.local/state/like-terminal/state.json` (geometria, sidebar, abas abertas)
- Senhas: Secret Service via crate `keyring` (serviço `like-terminal`)
- Dentro do Flatpak: prefixo `~/.var/app/io.github.liketerminal.LikeTerminal/`

## Arquitetura de conexão

1. `lt-ssh::build` monta o argv do `ssh` (ou do `$SHELL`, quando o host está vazio).
2. Se "Salvar senha" está ligado e há segredo no keyring, injeta `SSH_ASKPASS`,
   `SSH_ASKPASS_REQUIRE=force`, `LT_ASKPASS_*`; o próprio binário responde a senha.
3. Se o realce de saída está ligado, o comando é embrulhado:
   `like-terminal-relay --rules '...' --ips -- <ssh ...>`.
4. `vte4::Terminal::spawn_async` com ambiente completo (`TERM=xterm-256color`,
   `COLORTERM=truecolor`) — envv vazio NÃO herda o ambiente.
5. Host key e senha sem keyring são respondidos no próprio terminal.

## Armadilhas conhecidas

- **VTE 0.76 exige `PCRE2_MULTILINE` (0x400)** em `search_set_regex`/`match_add_regex`;
  sem a flag vem `VTE-WARNING` e nenhum match.
- **Single-instance**: o app usa GApplication; uma instância antiga em outro display
  faz o binário apenas "ativar" a invisível e sair com código 0. Ver `pgrep -a like-terminal`
  e matar por PID. Nunca usar `pkill -f like-terminal` (mata o próprio shell);
  use `pkill -x like-terminal`.
- **Splits**: ao reconstruir a árvore, Paneds antigos precisam ter os filhos desanexados
  (`detach_paneds`) antes de sair do container, senão desanexam os filhos reparentados.
- **Janela/CSD**: `window.width()` é área de conteúdo; somar 5 px por lado ao persistir.
- **`adw::MessageDialog`** usa `add_response(id, label)` e `connect_response`, não ResponseType.
- **`adw::PreferencesWindow`** usa `add(&PreferencesPage)`; `adw::Window` usa `ToolbarView`,
  nunca `set_child`.
- **Relay**: comparação de palavra-chave é byte a byte; `nocase` só afeta ASCII.
  O `carry` recua até limite de caractere UTF-8 para não quebrar acentos.
- **~/.ssh/config** precisa de permissão 600, senão o ssh recusa com
  "Bad owner or permissions".

## Convenções

- UI e mensagens em pt-BR; código, identificadores e commits em inglês.
- Sem comentários no código.
- Conventional Commits; nunca commitar/pushar sem pedido explícito.
- Identidade git local: `Felipe Mendes <liptech@users.noreply.github.com>`.
- Antes de fechar fase: `fmt`, `clippy` sem warnings, testes verdes, `.deb` e Flatpak
  recompilados.

## Roadmap 0.2 (plano em `~/.plannotator/plans/like-terminal-0.2-2026-11-09.md`)

Ordem aprovada pelo usuário; SFTP incluído na 0.2; regras de realce só palavra-chave.

1. Realce configurável (regras editáveis) — concluído
2. Relay robusto UTF-8 — concluído (teste de acentos e `nocase`)
3. Log de sessão — concluído
4. Persistência de splits e broadcast — concluído
5. Snippets — concluído
6. Importar `~/.ssh/config` — concluído
7. Port forwarding e cadeia de jump hosts — concluído
8. SFTP dois painéis (crate `ssh2`)
9. CI, README, CHANGELOG, tag v0.2.0 e release — parcial (README e CHANGELOG prontos)

### Estado da fase 7

- Modelo em `lt-core`: `ForwardKind {Local, Remote, Dynamic}` e `ForwardRule {kind, bind_address, bind_port, target_host, target_port}` com `problem()` e `summary()`; `SshOptions.forwards` com `serde(default)`. Testes de round-trip, defaults legados e validação.
- `lt-ssh`: `build()` emite `-L`, `-R` e `-D` para regras válidas (antes do destino); `-J` aceita cadeia separada por vírgula. Testes cobrindo os três tipos e regra inválida ignorada.
- UI: `crates/app/src/session_dialog.rs` ganhou o campo "Encaminhamento de portas" (botão mostra "Nenhuma"/"1 regra"/"N regras"); `crates/app/src/forwards_dialog.rs` (novo, escrito pelo Beam Free) lista regras com `summary()`, permite adicionar/editar/excluir em diálogo próprio (tipo, endereço, porta, destino, porta de destino; destino desabilitado em SOCKS) e valida via `problem()`.
- Validado no Xvfb: regra `127.0.0.1:8080 → 10.0.0.5:80` salva em `sessions.json` como `{"kind":"local","bind_address":"127.0.0.1","bind_port":8080,"target_host":"10.0.0.5","target_port":80}`.
- Testes totais: 35 core + 16 relay + 32 ssh = 83.

### Estado da fase 1

Concluída: `HighlightRule`/`HighlightConfig` no `lt-core` (com defaults e migração por
serde), `parse_rules` com `bold`/`nocase` + `--rules-file` no relay, integração do
`HighlightConfig` em `terminal.rs`/`panes.rs`/`window.rs`, página "Realce" em
`crates/app/src/highlight_page.rs` (CRUD de palavras, cor, negrito, ignorar caixa,
switch de IPs), hook `set_highlight` em `preferences.rs`/`window.rs`, testes de
`HighlightRule::spec`/`problem`, `Store` legado e `highlight_config`.

### Estado da fase 5

Concluída: `lt_core::Snippet` (nome, texto, `send_enter`, validação por `problem()`),
`Store.snippets` + `Store::snippet(id)`, `crates/app/src/snippets.rs` (`show_manager`
com CRUD e `show_picker` com busca; envio por clique via `GestureClick` e Enter via
`row_activated`), atalho `Ctrl+Shift+S`, ações `app.snippets`/`app.manage-snippets` em
`window.rs` (envia ao painel ativo ou a todos os painéis quando o broadcast está ligado).

### Estado da fase 6

Concluída: `lt_ssh::config` (`ConfigEntry`, `parse`, `read_default`; Hosts múltiplos,
`=`/espaço/tab, aspas, continuação, primeiro valor vence), `crates/app/src/ssh_import.rs`
(diálogo com checkboxes e conversão para `Session`, `AuthMethod::PublicKey` quando há
`IdentityFile`) e ação `app.import-ssh-config` com filtro de aliases já existentes.
Validado no Xvfb com o `~/.ssh/config` real: 14 sessões importadas.

### Estado da fase 4

Concluída: `lt_core::PaneLayout`/`SplitAxis` (`crates/core/src/layout.rs`, limite de 8
painéis, ratios clampados, `normalize()`), `UiState.tabs`/`broadcast`, `PaneView::with_tree`
e `tree()` em `panes.rs`, `restore_tabs`/`open_layout`/`open_layouts` em `window.rs` e
gravação do estado quando a árvore de painéis muda (`on_state` chama `save_ui_state`).
Validado no Xvfb: H[leaf, V[leaf, leaf]] + "Todas as abas" sobrevivem ao fechar e reabrir.

### Estado da fase 8

Concluída: crate `lt-sftp` sobre `russh`/`russh-sftp` (worker tokio, autenticação por
senha/chave/agente, `known_hosts`, transferências em blocos de 64 KiB) e janela de dois
painéis `crates/app/src/sftp_window.rs` (navegação local/remota, enviar/baixar, nova pasta,
renomear, excluir, desconectar; threads só trocam dados `Send` com a UI via canal + timeout
local). Aberta pelo menu da sessão ("Abrir SFTP") quando a sessão não é local.
Validado no Xvfb contra `10.175.5.4` (usuário `ubuntu`, chave): home, listagem, criação de
pasta, upload, download, rename e exclusão. Corrigido `request_subsystem(true, "sftp")`
antes de `SftpSession::new` (sem isso o subsistema fica em timeout).

### Estado da fase 9

Versão 0.2.0: `Cargo.toml` do workspace e `Cargo.lock`, release no metainfo, `CHANGELOG.md`
e `README.md` atualizados, tag anotada `v0.2.0` e workflow de CI em
`.github/workflows/ci.yml` (fmt, clippy com `-D warnings`, testes em ubuntu-24.04).
Publicação (GitHub release/Flathub) ainda depende de um remote.

## Backlog pós-0.2

Telnet/serial, transparência e imagem de fundo, múltiplas janelas e tear-off de abas,
codificação de caracteres, senha mestra, sync de sessões, i18n (gettext), AppImage,
publicação no Flathub.

## Publicação

- Repositório público: https://github.com/liptech/like-terminal (remote `origin`, branch `main`).
- Tag `v0.2.0` e release com os anexos `like-terminal_0.2.0_amd64.deb` (2,2 MB) e `like-terminal.flatpak` (3,0 MB).
- Artefatos são gerados por `./packaging/build-deb.sh` e pelo `flatpak-builder` + `flatpak build-bundle`; todos ficam no `.gitignore`.
