# printcher

Ferramenta de captura de tela e anotação para Linux, inspirada no [ShareX](https://getsharex.com/).

Funciona tanto em **X11** quanto em **Wayland**, e em qualquer desktop que
siga os padrões do freedesktop.org (testado em GNOME; compatível com KDE).

> **Status:** em desenvolvimento ativo. As funcionalidades principais já
> estão implementadas; ainda não existe um pacote pronto pra instalar (veja
> [Instalação](#instalação)). Detalhes técnicos e o que falta validar estão
> em [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

## Funcionalidades

- **Captura de tela cheia** com um atalho de teclado configurável.
- **Editor de anotações**: setas, retângulos, elipses, linhas, texto e
  recorte (crop) — tudo isso sobre a captura "congelada", sem pressa.
- **Copiar pra área de transferência** ou **salvar em arquivo**, com apenas
  um clique (ou atalho de teclado).
- **Roda em segundo plano**: um ícone na bandeja do sistema dá acesso rápido
  a capturar, configurar o atalho, ou abrir as configurações.
- **Inicia com o sistema** automaticamente (configurável).
- Sem upload nem compartilhamento externo — tudo fica local, na sua
  máquina.

## Como funciona

Ao pressionar o atalho, o printcher captura a tela inteira e abre um editor
próprio, em tela cheia — a mesma estratégia do ShareX. Você pode desenhar
setas, formas e texto sobre a imagem inteira, e recortar (crop) só quando
quiser: cortar é mais uma ferramenta na barra, não uma etapa obrigatória
logo no início.

## Instalação

Ainda não existe um instalador ou pacote pronto — por enquanto, a única
forma de usar o printcher é **compilando a partir do código-fonte**.
(Um pacote Flatpak já está preparado, mas o build ainda não foi feito —
acompanhe o progresso em [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).)

Pré-requisitos (Fedora):

```bash
sudo dnf install rustup gtk4-devel libadwaita-devel dbus-devel \
    libxcb-devel libX11-devel libXrandr-devel libXfixes-devel
rustup-init -y --default-toolchain stable --profile default
```

Em outras distros, o nome dos pacotes muda (no `apt`: `libgtk-4-dev`,
`libadwaita-1-dev`, `libdbus-1-dev`, `libxcb-dev`, `libx11-dev`,
`libxrandr-dev`, `libxfixes-dev`), mas a ideia é a mesma.

```bash
git clone https://github.com/davicolzer/printcher.git
cd printcher
cargo build --release
```

O binário fica em `target/release/printcher`.

## Como usar

Na primeira vez, rode o printcher (ou instale o ícone dele no menu de
aplicativos — veja abaixo) e configure o atalho de captura na tela de
configurações. Depois disso, é só apertar a tecla escolhida pra capturar.

```bash
# Registra o ícone no menu de aplicativos do sistema
target/release/printcher --install-launcher

# Abre a tela de configurações (também acessível pelo ícone na bandeja)
target/release/printcher --settings
```

Comandos úteis:

| Comando | O que faz |
|---|---|
| `printcher` | Captura a tela agora (sobe o app em segundo plano se ainda não estiver rodando) |
| `printcher --settings` | Abre a tela de configurações |
| `printcher --configure-shortcut` | Abre a tela do sistema pra trocar o atalho de captura |
| `printcher --quit` | Encerra o printcher |
| `printcher --install-launcher` | Adiciona o ícone ao menu de aplicativos |
| `printcher --install-autostart` | Liga o início automático com o sistema (já vem ligado por padrão) |
| `printcher --uninstall-all` | Remove tudo (autostart, ícone, configurações) |

Na tela de configurações também dá pra ligar/desligar o início automático
com o sistema.

## Licença

[GPLv3](LICENSE) — o código é livre pra usar e modificar, mas qualquer
versão derivada precisa continuar aberta sob a mesma licença.

## Para desenvolvedores

Arquitetura, decisões técnicas, status detalhado por funcionalidade e
processo de empacotamento estão em
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).
