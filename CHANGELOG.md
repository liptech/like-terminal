# Changelog

Todas as mudanças notáveis deste projeto são documentadas neste arquivo.

O formato segue [Keep a Changelog 1.1.0](https://keepachangelog.com/pt-BR/1.1.0/) e
este projeto adere ao [Semantic Versioning](https://semver.org/lang/pt-BR/).

## [0.4.1] - 2026-11-11

### Corrigido

- Sessões VNC e RDP abrem apenas clientes dedicados (`vncviewer`/`xtigervncviewer` e `xfreerdp`/`xfreerdp3`); o Remmina deixou de ser usado.
- Clientes VNC/RDP são aguardados em segundo plano, evitando processos zumbis.
- O pacote `.deb` passou a depender de `freerdp3-x11 | freerdp2-x11` e `tigervnc-viewer`.

## [0.4.0] - 2026-11-11

### Adicionado

- Sessões **Telnet** com negociação de terminal pelo próprio relay, sem depender do binário `telnet`.
- Sessões **VNC** e **RDP** que abrem o cliente instalado (`vncviewer`, `xfreerdp` ou `remmina`), com host, porta, usuário, domínio e argumentos extras.
- Tela de bloqueio do aplicativo com senha local (Argon2) e verificação em duas etapas (TOTP) opcional, configurável em Preferências → Segurança.
- Seletor de tipo de conexão no diálogo de sessão com seis opções: SSH, Terminal local, Serial, Telnet, VNC e RDP.

### Alterado

- Novo ícone do aplicativo, com identidade visual de terminal, e logotipo Liptech na janela Sobre.
- Janela Sobre personalizada com a marca Liptech.

## [0.3.1] - 2026-11-11

### Adicionado

- O pacote `.deb` agora adiciona o usuário que instalou ao grupo `dialout`, permitindo usar portas seriais sem passos manuais (é preciso sair e entrar na sessão novamente).

## [0.3.0] - 2026-11-11

### Adicionado

- Sessões seriais com porta, velocidade, bits de dados, paridade, stop bits e controle de fluxo, usando ponte própria sobre `termios` (sem depender de `minicom`/`picocom`).
- Tipo de conexão no diálogo de sessão (SSH/terminal local ou serial), com detecção de adaptadores em `/dev/serial/by-id` e `/dev/ttyUSB*`, `/dev/ttyACM*`, `/dev/ttyS*`.

### Alterado

- Flatpak passa a declarar `--device=all` para permitir acesso às portas seriais. No `.deb`, o usuário precisa pertencer ao grupo `dialout`.

## [0.2.3] - 2026-11-09

### Adicionado

- Preferência "Restaurar abas ao abrir": permite iniciar com a janela vazia em vez de reabrir as abas e divisões da última sessão.

## [0.2.2] - 2026-11-09

### Corrigido
- O relay agora desliga o echo do próprio terminal de entrada antes de abrir o PTY filho.
  Na 0.2.1 o modo raw era aplicado só no lado do `ssh`, mas o terminal do relay ainda
  ecoava a digitação localmente, então a senha continuava visível em prompts de senha
  (`felipe.mendes@host's password:`).

## [0.2.1] - 2026-11-09

### Corrigido
- O PTY interno do relay agora inicia em modo raw, igual ao terminal do app. Antes ele
  subia em modo canônico com echo ligado, fazendo o servidor herdar `ECHO` e exibir a
  senha (e qualquer digitação) em aparelhos cujo CLI não desliga o eco.

## [0.2.0] - 2026-11-09

### Adicionado
- Realce de saída configurável: regras editáveis (palavra-chave, cor, negrito, ignorar
  maiúsculas/minúsculas) e coloração opcional de endereços IPv4, em Preferências →
  Realce.
- Relay PTY robusto em UTF-8 (acentos e `nocase` preservados em chunks arbitrários).
- Log de sessão com timestamps por linha e toggle de pausa/retomada (SIGUSR1).
- Persistência de splits (até 8 painéis) e do estado do broadcast entre sessões.
- Snippets: comandos salvos com busca e envio a um painel ou a todos.
- Importação de hosts de `~/.ssh/config` (alias, host, usuário, porta, chave).
- Encaminhamento de portas local, remoto e SOCKS (`-L`, `-R`, `-D`) com editor gráfico
  de regras no diálogo de sessão.
- Navegador SFTP de dois painéis (local/remoto) com listagem, navegação, criação de
  pasta, renomeação, exclusão e transferência de arquivos, usando russh/russh-sftp em
  Rust puro.
- Estado vazio com chamada para ação quando não há abas abertas.

## [0.1.0] - 2026-11-05

### Adicionado
- Janela GTK4/libadwaita com abas e painéis divididos.
- Terminal local e conexões SSH via OpenSSH (senha, chave, agente, teclado interativo).
- Perfis de terminal (fonte, paleta, scrollback, cursor).
- Busca no histórico, zoom e copiar/colar.
- Broadcast de texto para todos os painéis.
- Gerenciador de sessões com pastas aninhadas e busca.
- Conexão rápida por `usuario@host:porta` ou nome de sessão.
- Preferências (perfil padrão, confirmação de fechamento).
- Realce de saída por relay PTY (palavras-chave padrão e IPs).
- Ícone da marca e metadados (`.desktop`, metainfo).
- Documentos de licença, termos e privacidade.
- Empacotamento `.deb` e Flatpak.