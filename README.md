# printcher

Ferramenta de captura de tela e anotação para Linux, inspirada no [ShareX](https://getsharex.com/).

Funciona tanto em **X11** quanto em **Wayland**, e em qualquer desktop que
siga os padrões do freedesktop.org (testado em GNOME; compatível com KDE).

> Detalhes técnicos, arquitetura e status por funcionalidade estão em
> [`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md).

## Funcionalidades

- **Captura por região**: um atalho de teclado configurável abre a tela
  congelada pronta pra você arrastar a área que quiser recortar.
- **Editor de anotações**, numa barra flutuante sobre a captura: Cortar,
  Seta, Retângulo, Elipse, Caneta livre, Desfoque (borra uma área, ex. pra
  esconder dado sensível) e Numeração (marcadores numerados, tipo passo a
  passo de tutorial) — cada uma com paleta de cores e espessura de traço
  próprias.
- **Salvar é rápido**: confirmar uma seleção de corte já salva sozinho; sem
  corte, o botão Salvar (ou Enter, em qualquer lugar do editor) grava a
  tela inteira com tudo que foi desenhado.
- **Copiar pra área de transferência** com um clique ou `Ctrl+C`, e
  opcionalmente sempre que salvar também (configurável).
- **Destino configurável**: pasta de destino, formato (PNG/JPG/WebP) e
  padrão de nome do arquivo (com códigos tipo `%Y-%m-%d`, contador, etc.).
- **Histórico**: lista das últimas capturas salvas, com miniatura.
- **Notificação ao salvar**, com miniatura da captura e um botão "Abrir
  pasta".
- **Roda em segundo plano**: ícone na bandeja do sistema dá acesso rápido a
  capturar, configurar o atalho, abrir as configurações ou o histórico.
- **Inicia com o sistema** automaticamente (configurável).
- **Fonte amigável pra dislexia** (OpenDyslexic) como opção na janela de
  configurações.
- Interface nativa do GNOME/libadwaita (tema claro/escuro/sistema,
  janela de configurações redimensionável) — sem visual próprio pra
  manter, sem CSS pra brigar com o tema do usuário.
- Sem upload nem compartilhamento externo — tudo fica local, na sua
  máquina.

## Como funciona

Ao pressionar o atalho, o printcher congela a tela inteira e abre uma
barra de ferramentas flutuante sobre ela. Arraste com a ferramenta Cortar
pra selecionar a área que quer manter — ao soltar o mouse, a captura já é
salva automaticamente (na pasta e formato configurados, e copiada pra área
de transferência se essa opção estiver ligada). Pra desenhar antes de
cortar (setas, formas, desfoque, numeração), é só trocar de ferramenta na
barra; nada obrigatório acontece na ordem errada. Se não quiser cortar
nada, o botão Salvar (ou `Enter`) grava a tela inteira do jeito que foi
anotada.

## Instalação

### Flatpak (recomendado)

```bash
git clone https://github.com/davicolzer/printcher.git
cd printcher
flatpak-builder --user --install --force-clean build-dir flatpak/com.printcher.Printcher.json
```

Isso builda 100% offline (todas as dependências Rust já vêm vendorizadas
no manifesto) e instala o app isolado em sandbox, com os ícones e atalho
de teclado do sistema já registrados.

### Compilando a partir do código-fonte

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
# Flatpak
flatpak run com.printcher.Printcher --settings

# Build a partir do código-fonte
target/release/printcher --install-launcher   # registra o ícone no menu
target/release/printcher --settings           # abre as configurações
```

Comandos úteis (troque `printcher` por `flatpak run com.printcher.Printcher`
se estiver usando o Flatpak):

| Comando | O que faz |
|---|---|
| `printcher` | Captura a tela agora (sobe o app em segundo plano se ainda não estiver rodando) |
| `printcher --settings` | Abre a tela de configurações |
| `printcher --configure-shortcut` | Abre a tela do sistema pra trocar o atalho de captura |
| `printcher --quit` | Encerra o printcher |
| `printcher --install-launcher` | Adiciona o ícone ao menu de aplicativos (build a partir do código-fonte; o Flatpak já registra sozinho) |
| `printcher --install-autostart` | Liga o início automático com o sistema (já vem ligado por padrão) |
| `printcher --uninstall-all` | Remove autostart, ícone e configurações salvas |

Pra desinstalar o Flatpak por completo:

```bash
flatpak run com.printcher.Printcher --uninstall-all
flatpak uninstall --user com.printcher.Printcher
```

## Licença

Software proprietário — veja [`LICENSE`](LICENSE). Todos os direitos
reservados; uso, cópia e redistribuição não autorizados são proibidos.

## Para desenvolvedores

Arquitetura, decisões técnicas, status detalhado por funcionalidade e
processo de empacotamento estão em
[`docs/DEVELOPMENT.md`](docs/DEVELOPMENT.md). Histórico de mudanças em
[`CHANGELOG.md`](CHANGELOG.md).
