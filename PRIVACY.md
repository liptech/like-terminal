# Política de Privacidade — Like Terminal

Última atualização: 5 de novembro de 2026.

O Like Terminal é um aplicativo de terminal para Linux que roda inteiramente na sua
máquina. Esta política descreve quais dados o aplicativo trata e como eles ficam
armazenados.

## Resumo

- Nenhum dado é enviado para servidores do projeto.
- Não há telemetria, analytics, anúncios ou rastreamento.
- Não há conta, login ou cadastro no aplicativo.
- Toda a configuração fica em arquivos locais no seu diretório pessoal.

## Dados armazenados localmente

- **Sessões, pastas e preferências** — em `~/.config/like-terminal/sessions.json`, para
  lembrar as conexões e a aparência escolhida.
- **Geometria da janela, abas abertas e estado da barra lateral** — em
  `~/.local/state/like-terminal/state.json`, para restaurar a janela como estava.
- **Senhas e passphrases salvas (opcional)** — no serviço de segredos do sistema
  (libsecret/Secret Service), para autenticação automática em conexões SSH.

Nenhum desses arquivos é lido pelo projeto ou compartilhado com terceiros. Você pode
apagá-los a qualquer momento; o aplicativo recria os arquivos padrão na próxima
execução.

## Senhas e chaves

Quando você marca "Salvar senha", a senha é gravada no serviço de segredos do seu
ambiente (por exemplo, GNOME Keyring) e nunca em texto puro nos arquivos de
configuração. Chaves privadas SSH permanecem onde já estão, em `~/.ssh` ou no caminho
que você indicar; o aplicativo apenas as repassa ao `ssh` do sistema.

## Conexões de rede

O aplicativo só abre conexões de rede quando você inicia uma sessão remota, usando o
cliente `ssh` do sistema. Os dados dessa conexão vão diretamente do seu computador ao
servidor que você escolheu; o projeto não intermedeia, registra ou analisa esse
tráfego.

Ao instalar pela Flathub, o Flatpak pode consultar o repositório para verificar
atualizações. Esse comportamento é do Flatpak, não do Like Terminal.

## Serviços de terceiros

- **OpenSSH**: transporte das conexões remotas.
- **Secret Service / libsecret**: armazenamento opcional de credenciais.
- **Flathub**: distribuição do pacote, quando utilizado.

O tratamento de dados por esses componentes segue as políticas dos próprios projetos.

## Crianças

O aplicativo não coleta dados de nenhum usuário, incluindo crianças.

## Alterações

Mudanças nesta política serão publicadas neste mesmo arquivo, com a data de
atualização revisada.

## Contato

Dúvidas sobre privacidade podem ser registradas no repositório oficial do projeto.
