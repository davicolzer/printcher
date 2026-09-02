# printcher

Ferramenta de captura de tela e anotação para Linux, inspirada no [ShareX](https://getsharex.com/), com suporte planejado para X11 e Wayland (foco de testes: Fedora + GNOME Wayland).

## Status

Em desenvolvimento inicial. Sem funcionalidades prontas ainda.

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

## Roadmap (entregas)

1. **M0 — Ambiente e base do projeto** (em andamento)
2. **M1 — Captura full screen (Wayland/GNOME) + salvar em arquivo**
3. **M2 — Crop local da captura**
4. **M3 — Copiar para a área de transferência**
5. **M4 — Backend X11 (paridade com M1–M3)**
6. **M5 — Editor de anotações (setas, formas, texto, etc.)**
7. **M6 — Atalho global e empacotamento**
