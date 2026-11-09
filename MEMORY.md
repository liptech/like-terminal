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

1. Realce configurável (regras editáveis) — **em andamento**
2. Relay robusto UTF-8 — concluído (teste de acentos e `nocase`)
3. Log de sessão
4. Persistência de splits e broadcast
5. Snippets
6. Importar `~/.ssh/config`
7. Port forwarding e cadeia de jump hosts
8. SFTP dois painéis (crate `ssh2`)
9. CI, README, CHANGELOG, tag v0.2.0 e release

### Estado da fase 1

Concluída: `HighlightRule`/`HighlightConfig` no `lt-core` (com defaults e migração por
serde), `parse_rules` com `bold`/`nocase` + `--rules-file` no relay, integração do
`HighlightConfig` em `terminal.rs`/`panes.rs`/`window.rs`, página "Realce" em
`crates/app/src/highlight_page.rs` (CRUD de palavras, cor, negrito, ignorar caixa,
switch de IPs), hook `set_highlight` em `preferences.rs`/`window.rs`, testes de
`HighlightRule::spec`/`problem`, `Store` legado e `highlight_config`.

## Backlog pós-0.2

Telnet/serial, transparência e imagem de fundo, múltiplas janelas e tear-off de abas,
codificação de caracteres, senha mestra, sync de sessões, i18n (gettext), AppImage,
publicação no Flathub.
