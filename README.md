# printcher

Ferramenta de captura de tela e anotação para Linux, inspirada no [ShareX](https://getsharex.com/), com suporte planejado para X11 e Wayland (foco de testes: Fedora + GNOME Wayland).

## Status

- ✅ M0 — Ambiente e base do projeto
- ✅ M1 — Captura full screen (Wayland/GNOME), validado
- ✅ M2 — Editor de captura (crop, setas, formas, texto), validado
- ✅ M3 — Copiar para a área de transferência, validado
- 🚧 M4 — Backend X11: implementado e compilando, **ainda não testado em
  sessão X11 real** (só temos GNOME Wayland disponível até agora)
- ✅ M6 — Processo em segundo plano (daemon de instância única + autostart),
  validado de ponta a ponta via D-Bus
- 🚧 M7 — Atalho global via portal (`GlobalShortcuts`): implementado, degrada
  graciosamente, mas **o atalho de tecla em si só funciona rodando como
  Flatpak** (o portal exige identidade de app — rejeita o binário "cru" com
  `An app id is required`). Capturar via D-Bus/tray continua funcionando
  normalmente mesmo sem isso.
- 🚧 M8 — Ícone na bandeja (StatusNotifierItem via `ksni`): implementado,
  degrada graciosamente. Testado aqui: sem a extensão "AppIndicator and
  KStatusNotifierItem Support" (não vem por padrão no Fedora), o registro
  falha com `ServiceUnknown` e o daemon segue funcionando normalmente sem
  ícone. Nativo no KDE, sem extensão nenhuma.
- 🚧 M9 — Tela de configurações (`src/settings_window.rs`, via libadwaita):
  implementada e validada de ponta a ponta via D-Bus/`pgrep` (primeira
  execução liga autostart sozinha, ícone de launcher abre/reaproveita o
  daemon corretamente, encerramento limpo). O **conteúdo visual da janela e
  o diálogo de confirmação ao fechar** ainda não foram vistos numa tela de
  verdade.
- 🚧 Empacotamento (Flatpak): manifesto escrito, **build ainda não testado**
  (falta `flatpak-builder` e o download do runtime/SDK) — agora também
  pré-requisito real pro M7 funcionar

## Escopo

- Captura de tela cheia
- Recorte local (crop) da captura
- Copiar para a área de transferência / salvar em arquivo
- Editor de anotações (setas, formas, texto, etc.)
- Sem upload/compartilhamento externo (fora de escopo por enquanto)

## Stack

- Rust (toolchain via `rustup`, sempre na última stable)
- GTK4 + libadwaita para a interface
- Captura: X11 nativo (`x11rb`) e Wayland via `xdg-desktop-portal` (`ashpd`)

## Fluxo de captura (estratégia ShareX)

Ao capturar, a tela cheia é congelada (imagem estática) e aberta em um editor
próprio, em tela cheia. Anotações (setas, formas, texto) podem ser feitas
sobre a imagem inteira, e o **crop é apenas mais uma ferramenta** da barra,
não uma etapa obrigatória logo no início — igual ao ShareX. Isso evita a
necessidade de overlay ao vivo sobre a tela (não suportado no GNOME Wayland
sem `layer-shell`): a captura via portal já entrega o bitmap parado, e toda a
edição (crop, setas, formas) acontece localmente sobre essa imagem.

## Roadmap (entregas)

1. **M0 — Ambiente e base do projeto**
2. **M1 — Captura full screen (Wayland/GNOME) + salvar em arquivo**
3. **M2 — Editor de captura: abre a imagem congelada em tela cheia, com
   ferramentas de crop, setas, formas e texto**
4. **M3 — Copiar para a área de transferência**
5. **M4 — Backend X11 (paridade com M1–M3)**
6. **M6 — Processo em segundo plano: daemon de instância única + autostart**
7. **M7 — Atalho global via portal (`GlobalShortcuts`), configurável pela UI
   nativa do sistema — funciona em qualquer desktop que implemente o portal
   (GNOME e KDE), não só GNOME**
8. **M8 — Ícone na bandeja (StatusNotifierItem)**
9. **M9 — Tela de configurações, extensível pra novas opções**
10. **Empacotamento (Flatpak)**

A escolha de backend (`src/capture.rs`) é automática: se `WAYLAND_DISPLAY`
estiver definida, usa o portal; senão, cai para a conexão X11 direta
(`src/capture/x11.rs`, via `x11rb`, captura a janela raiz com `GetImage`).
Os dois caminhos produzem o mesmo PNG de saída, então o editor (M2) não
precisa saber qual foi usado.

## Processo em segundo plano e atalho global

O `printcher` agora é um daemon de instância única, com um pequeno protocolo
de IPC próprio via D-Bus (`com.printcher.Printcher`):

- `printcher` (sem argumentos): se já tem um daemon rodando, só pede pra ele
  capturar e sai na hora; senão, vira o daemon e já dispara uma captura
  inicial.
- `printcher --daemon`: sobe o daemon sem fazer nada além disso (usado pelo
  autostart).
- `printcher --settings`: se já tem daemon rodando, só abre a janela de
  configurações nele; senão, sobe o daemon e já abre a janela (sem
  capturar). É o comando usado pelo ícone do launcher.
- `printcher --quit`: pede pro daemon rodando encerrar.
- `printcher --configure-shortcut`: abre a UI nativa do sistema pra
  remapear o atalho de captura.

Essas duas ações (`Capture` e `OpenSettings`) são tratadas de forma
unificada em `daemon::run` via um enum `InitialAction` — "sobe o daemon se
precisar, e dispara X" é a mesma lógica pros dois casos.

O atalho de teclado em si é registrado via
`org.freedesktop.portal.GlobalShortcuts` (não mais via `gsettings` — isso
funciona em qualquer desktop que implemente o portal, GNOME ou KDE). A tecla
de fato é escolhida pelo usuário na UI de configuração do sistema, não
fixada pelo app. **Importante:** esse portal exige que o processo tenha uma
identidade de app reconhecida — rodando o binário direto (`cargo run`) ele
falha com `An app id is required` e o daemon segue sem esse atalho (D-Bus e
bandeja continuam funcionando). Isso só se resolve rodando como Flatpak.

O ícone da bandeja (`src/tray.rs`, via `ksni`) sobe junto com o daemon, com
um menu (Capturar agora / Configurar atalho / Sair). Se não houver um "host"
de bandeja no D-Bus (comum no GNOME sem extensão), o registro falha e é só
logado — o resto do daemon não é afetado.

Autostart (inicia o daemon junto com a sessão, sem capturar):

```bash
cargo run --release -- --install-autostart
cargo run --release -- --uninstall-autostart
```

## Tela de configurações

`printcher --settings` (ou o ícone "Configurações" no menu da bandeja, ou o
ícone do launcher) abre uma janela (`src/settings_window.rs`, libadwaita)
com grupos de preferências independentes — pensada pra crescer: cada nova
opção futura (pasta de destino, cor padrão de anotação, etc.) entra como um
novo `PreferencesGroup`/linha, sem mexer no resto. Hoje tem:

- **Atalho de captura**: botão que abre a UI nativa do sistema pra
  remapear a tecla (mesmo mecanismo do `--configure-shortcut`).
- **Geral → Iniciar com o sistema**: liga/desliga o autostart.

As configurações ficam em `~/.config/printcher/config.toml`. Na primeira
execução (nenhum config ainda existe), `start_on_login` já entra `true` por
padrão e o autostart é registrado sozinho — não precisa de nenhum passo
manual na instalação.

Fechar a janela pergunta se você quer encerrar o printcher por completo ou
deixá-lo em segundo plano (é isso que mantém o atalho global e a bandeja
ativos) — a captura sempre continua funcionando enquanto o processo estiver
de pé, então fechar a janela sem querer não desliga nada por engano.

### Ícone no menu de aplicativos

Diferente do autostart (oculto), esse aparece no launcher/menu de
aplicativos de verdade. Clicar nele sobe o daemon se precisar e abre a
janela de configurações (não captura):

```bash
cargo run --release -- --install-launcher
cargo run --release -- --uninstall-launcher
```

## Empacotamento (Flatpak)

Manifesto em `flatpak/com.printcher.Printcher.json`, usando o runtime
`org.gnome.Platform` 50 + extensão `rust-stable`. Ainda não testado (precisa
de `flatpak-builder`, que baixa o runtime/SDK — vários GB). Pra buildar:

```bash
sudo dnf install flatpak-builder
flatpak-builder --user --install build-dir flatpak/com.printcher.Printcher.json
```

O build usa `--share=network` pra deixar o `cargo` baixar as dependências
direto (mais simples pra uso pessoal). Uma submissão ao Flathub exigiria
vendorizar as dependências via `flatpak-cargo-generator.py` pra build
offline/reprodutível — não é o objetivo agora.
