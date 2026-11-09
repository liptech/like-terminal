# Like Terminal

Emulador de terminal para Linux (GNOME) escrito em Rust, com GTK4 e libadwaita. Oferece
abas, painéis divididos, gerenciador de sessões, conexões SSH (senha, chave, agente e
jump host), realce de saída por regras de palavra-chave, snippets, log de sessão e
persistência de layout.

App ID: `io.github.liketerminal.LikeTerminal` · Binário: `like-terminal` · Auxiliar:
`like-terminal-relay` (filtro de PTY que colore a saída).

## Recursos

| Recurso | Descrição |
|---|---|
| Abas | Múltiplas abas por janela, com reordenação e fechamento. |
| Painéis divididos | Divisão horizontal/vertical dentro de cada aba, até 8 painéis. |
| Terminal local | Abre o shell padrão (`$SHELL`) quando o host está vazio. |
| Conexões SSH | Senha, chave pública, agente SSH, teclado interativo, jump host, compressão, X11, keepalive. |
| Perfis | Fonte, paleta (10 temas), scrollback, cursor, cópia ao selecionar, realce. |
| Realce de saída | Palavras-chave coloridas e negrito, com regras editáveis; coloração opcional de IPs. |
| Busca | Busca no histórico do terminal (regex com `PCRE2_MULTILINE`). |
| Zoom | Ajuste de escala da fonte (`0,5x` a `4x`). |
| Copiar/colar | Via atalhos e menu de contexto. |
| Broadcast | Envia o texto digitado para todos os painéis da aba. |
| Gerenciador de sessões | Pastas aninhadas, busca por nome/host/usuário, cores por sessão. |
| Conexão rápida | Abre sessão a partir de `usuario@host:porta` ou nome salvo. |
| Snippets | Comandos salvos, aplicáveis com busca; envio a um painel ou a todos. |
| Importação | Importa hosts de `~/.ssh/config`. |
| Log de sessão | Grava saída em arquivo, com timestamps e pausa/retomada. |
| Persistência | Geometria da janela, abas e layout de painéis restaurados ao reabrir. |
| Senhas | Armazenadas no Secret Service (keyring), serviço `like-terminal`. |
| Links | URLs, e-mails e caminhos clicáveis (Ctrl+clique). |

## Requisitos

- Ubuntu 24.04 (ou equivalente).
- GTK 4.14+, libadwaita 1.5+, VTE GTK4 0.76.
- OpenSSH client (`openssh-client`).
- Rust 1.99+ (toolchain estável).
- Opcional: `fish` (realce de sintaxe em terminais locais).

## Instalação

### Pacote .deb

```bash
sudo apt install ./build-deb/like-terminal_0.1.0_amd64.deb
```

Reinstalar/atualizar:

```bash
sudo apt install --reinstall ./build-deb/like-terminal_0.1.0_amd64.deb
```

### Flatpak

```bash
flatpak install --user like-terminal.flatpak
```

Dentro do Flatpak os dados ficam em
`~/.var/app/io.github.liketerminal.LikeTerminal/`.

### Compilação a partir do código

Dependências de desenvolvimento (Ubuntu 24.04):

```bash
sudo apt install build-essential pkg-config \
  libgtk-4-dev libadwaita-1-dev libvte-2.91-gtk4-dev \
  libdbus-1-dev libsecret-1-dev openssh-client
```

Compilar e executar:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
cargo build --release
cargo run
```

Testes e análise:

```bash
cargo test --workspace
cargo clippy --workspace --all-targets
cargo fmt --all
```

## Como usar

### Atalhos de teclado

| Atalho | Ação |
|---|---|
| `Ctrl+Shift+N` | Nova sessão |
| `Ctrl+L` | Conexão rápida |
| `Ctrl+Shift+T` | Nova aba |
| `Ctrl+Shift+W` | Fechar aba |
| `Ctrl+Shift+D` | Dividir à direita |
| `Ctrl+Shift+E` | Dividir abaixo |
| `Ctrl+Shift+C` / `Ctrl+Shift+V` | Copiar / colar |
| `Ctrl+Shift+F` | Buscar |
| `Ctrl+=` / `Ctrl+-` / `Ctrl+0` | Zoom (mais / menos / resetar) |
| `F9` | Mostrar/ocultar painel de sessões |
| `Ctrl+Shift+B` | Broadcast (enviar para todas as abas) |
| `Ctrl+Shift+S` | Snippets |
| `Ctrl+,` | Preferências |
| `Ctrl+?` | Lista de atalhos |
| `Ctrl+Q` | Sair |

### Conexões SSH

Crie uma sessão no gerenciador e escolha o método de autenticação:

- **Senha**: informe usuário e senha; marque "Salvar senha" para guardar no keyring.
- **Chave pública**: informe o caminho da chave privada (`IdentityFile`); passphrase opcional via keyring.
- **Agente SSH**: usa o agente do sistema (`ssh-agent`).
- **Teclado interativo**: para servidores com autenticação por desafio.
- **Jump host**: campo `Jump` aceita `usuario@host:porta` (cadeia via `-J`).
- Opções adicionais: compressão, encaminhamento de agente, X11, keepalive, argumentos extras.

Conexão rápida (`Ctrl+L`): aceita `usuario@host:porta` ou o nome de uma sessão salva.

### Realce de saída

Regras configuráveis em Preferências → Realce:

- Palavras-chave com cor (hex `#rrggbb`), negrito e sensibilidade a maiúsculas/minúsculas.
- Coloração opcional de endereços IPv4 (amarelo).
- Regras padrão: `Error`, `Failed`, `Denied` (vermelho, negrito), `Warning` (amarelo),
  `Info` (azul), `Success` (verde).
- As mudanças valem para sessões abertas depois da alteração.

O realce é aplicado pelo `like-terminal-relay`, que envolve o comando (SSH ou shell) e
filtra a saída do PTY.

### Log de sessão

- Cada sessão pode ter o log ativado (padrão: desativado).
- Arquivos em `~/.local/state/like-terminal/logs` (ou diretório configurado em
  `log_dir`), com nome `<sessao>-<timestamp>.log`.
- O log remove sequências de escape e pode incluir timestamp ISO por linha.
- Pausa/retomada via `SIGUSR1` (botão na interface); estado inicial controlado por
  `--log-paused`.

### Snippets

- Gerencie em `Ctrl+Shift+S` (criar, editar, excluir; opção "Enviar Enter ao aplicar").
- Aplique via seletor com busca; envia ao painel ativo ou a todos quando o broadcast
  está ligado.

### Importação de `~/.ssh/config`

Ação "Importar de ~/.ssh/config": lê aliases (`Host`), `HostName`, `User`, `Port` e
`IdentityFile` (converte para autenticação por chave pública). Aliases já existentes
são filtrados.

## Arquitetura do workspace

```
crates/core   lt-core   modelos, store JSON atômico, keyring, validação (sem GTK)
crates/ssh    lt-ssh    montagem do argv do ssh, quick connect, probe TCP, parser de ~/.ssh/config
crates/relay  lt-relay  relay PTY + filtro de cores (binário like-terminal-relay)
crates/app    like-terminal  UI GTK4/libadwaita, VTE, janelas, preferências
data/         CSS, ícone SVG, .desktop, metainfo, logos da marca
packaging/    build-deb.sh
```

Fluxo de conexão:

1. `lt-ssh::build` monta o argv do `ssh` (ou `$SHELL` se o host estiver vazio).
2. Se "Salvar senha" estiver ativo e houver segredo no keyring, injeta `SSH_ASKPASS`,
   `SSH_ASKPASS_REQUIRE=force` e `LT_ASKPASS_*`; o próprio binário responde a senha.
3. Se o realce estiver ativo, o comando é embrulhado:
   `like-terminal-relay --rules '...' --ips -- <ssh ...>`.
4. `vte4::Terminal::spawn_async` com ambiente completo (`TERM=xterm-256color`,
   `COLORTERM=truecolor`).
5. Host key e senha sem keyring são respondidos no próprio terminal.

## Dados do usuário

| Arquivo | Conteúdo |
|---|---|
| `~/.config/like-terminal/sessions.json` | Sessões, pastas, perfis, preferências, regras de realce, snippets (gravação atômica tmp+rename). |
| `~/.local/state/like-terminal/state.json` | Geometria da janela, sidebar, abas abertas, broadcast. |
| `~/.local/state/like-terminal/logs/` | Logs de sessão. |
| Secret Service (keyring) | Senhas e passphrases, serviço `like-terminal`. |

Dentro do Flatpak o prefixo é `~/.var/app/io.github.liketerminal.LikeTerminal/`.

## Segurança e privacidade

- Nenhum dado é enviado para servidores do projeto; sem telemetria, analytics ou conta.
- Configuração e estado ficam em arquivos locais no diretório pessoal.
- Senhas nunca são gravadas em `sessions.json`; ficam no Secret Service.
- Veja [PRIVACY.md](PRIVACY.md) e [TERMS.md](TERMS.md).
- Licença: **GPL-3.0-or-later** (veja [LICENSE](LICENSE)).

## Limitações conhecidas

- Instância única: o aplicativo usa GApplication; uma instância antiga em outro display
  pode fazer o binário apenas "ativar" a invisível e sair com código 0.
- VTE 0.76 exige `PCRE2_MULTILINE` (`0x400`) em `search_set_regex`/`match_add_regex`.
- O relay compara palavras-chave byte a byte; `nocase` afeta apenas ASCII.
- `~/.ssh/config` precisa de permissão `600`, senão o `ssh` recusa ("Bad owner or permissions").
- Reconstrução de splits exige desanexar panes antigos antes de sair do container.
- Geometria da janela: `window.width()` é área de conteúdo; soma-se 5 px por lado ao persistir.

## Roadmap resumido

Entregue na 0.2: realce configurável (regras editáveis + IPs), relay robusto em UTF-8,
log de sessão com timestamps e toggle, persistência de splits e broadcast, snippets,
importação de `~/.ssh/config`.

Próximos: port forwarding e cadeia de jump hosts, SFTP em dois painéis, CI, publicação
no Flathub, AppImage, i18n, senha mestra, sync de sessões.

## Licença

GPL-3.0-or-later. Veja [LICENSE](LICENSE).