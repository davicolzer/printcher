# Changelog

Formato baseado em [Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/),
versionamento seguindo [SemVer](https://semver.org/lang/pt-BR/).

## [Unreleased]

Ainda não validado numa tela de verdade — veja
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md) pro status detalhado por
funcionalidade. Essa seção vira `[0.1.0] - AAAA-MM-DD` quando a primeira
versão for validada e marcada com tag.

### Adicionado

- Captura de tela cheia — via `xdg-desktop-portal` no Wayland, conexão X11
  direta (`x11rb`) no X11, com escolha automática de backend.
- Editor de anotações em tela cheia: setas, retângulos, elipses, linhas,
  texto e recorte (crop como mais uma ferramenta, não uma etapa
  obrigatória).
- Copiar a captura para a área de transferência ou salvar em arquivo.
- Execução em segundo plano (daemon de instância única) com atalho de
  teclado global, configurável pela tela de atalhos nativa do sistema
  (GNOME/KDE) via `org.freedesktop.portal.GlobalShortcuts`.
- Ícone na bandeja do sistema, com menu rápido (capturar, configurar
  atalho, configurações, sair).
- Tela de configurações (atalho de captura, iniciar com o sistema),
  extensível pra novas opções futuras.
- Início automático com o sistema, ligado por padrão na primeira execução.
- Ícone no menu de aplicativos (`--install-launcher`).
- Notificações do sistema para erros e confirmações (captura, salvar,
  copiar).
- Comando para desinstalar tudo de uma vez (`--uninstall-all`).
- Preparação para empacotamento via Flatpak: manifesto com build 100%
  offline (dependências vendorizadas), metadados AppStream, ícone
  placeholder, permissões revisadas.
- Licença GPLv3.

### Limitações conhecidas

- Backend de captura X11 ainda não testado numa sessão X11 real.
- Atalho de teclado global e registro completo do ícone de bandeja exigem
  rodar como Flatpak (o portal do atalho global exige identidade de app) —
  fora disso, degradam graciosamente (captura via clique/comando continua
  funcionando).
- Ícone de bandeja no GNOME depende da extensão "AppIndicator and
  KStatusNotifierItem Support" (não vem instalada por padrão); nativo no
  KDE.
- Build do Flatpak ainda não foi executado de verdade.
