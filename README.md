# printcher

Ferramenta de captura de tela e anotação para Linux, inspirada no [ShareX](https://getsharex.com/), com suporte planejado para X11 e Wayland (foco de testes: Fedora + GNOME Wayland).

## Status

- ✅ M0 — Ambiente e base do projeto
- ✅ M1 — Captura full screen (Wayland/GNOME), validado
- ✅ M2 — Editor de captura (crop, setas, formas, texto), validado
- ✅ M3 — Copiar para a área de transferência, validado
- 🚧 M4 — Backend X11: implementado e compilando, **ainda não testado em
  sessão X11 real** (só temos GNOME Wayland disponível até agora)
- 🚧 M5 — Atalho global: implementado e validado via `gsettings`. Manifesto
  Flatpak escrito, **build ainda não testado** (falta `flatpak-builder` e o
  download do runtime/SDK)

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
6. **M5 — Atalho global e empacotamento**

A escolha de backend (`src/capture.rs`) é automática: se `WAYLAND_DISPLAY`
estiver definida, usa o portal; senão, cai para a conexão X11 direta
(`src/capture/x11.rs`, via `x11rb`, captura a janela raiz com `GetImage`).
Os dois caminhos produzem o mesmo PNG de saída, então o editor (M2) não
precisa saber qual foi usado.

## Atalho global

Sem daemon: o `printcher` é um binário comum, disparado pelo próprio GNOME
quando o atalho é pressionado (não fica nada rodando em segundo plano). Pra
registrar o atalho global (padrão `<Control><Super>p`):

```bash
cargo run --release -- --install-shortcut
# ou com um atalho customizado:
cargo run --release -- --install-shortcut '<Shift><Super>p'
```

Pra remover:

```bash
cargo run --release -- --uninstall-shortcut
```

Comando idempotente — rodar de novo só atualiza o atalho/comando em vez de
duplicar a entrada no GNOME.

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
