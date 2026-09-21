# Handoff: Printcher — captura de tela, editor flutuante e configurações

Repositório de destino: `davicolzer/printcher` (branch `main`) — app Rust/GTK para Linux.

## Overview

Fluxo completo do Printcher em três partes:

1. **Captura** — atalho global (`PrtSc`) congela a tela, com flash branco de confirmação.
2. **Editor flutuante** — barra única no topo, sobre a tela congelada, com ferramentas de anotação, salvar e cancelar.
3. **Configurações** — janela aberta pelo menu/bandeja: atalhos, destino, formato, padrão de nome, tema, fonte; e uma aba de histórico.

## About the Design Files

Os arquivos deste pacote são **referências de design feitas em HTML** — protótipos que mostram aparência e comportamento pretendidos, não código de produção para copiar. A tarefa é **recriar esses designs no ambiente do próprio projeto** (Rust + GTK4/libadwaita, conforme `src/settings_window.rs` e `src/editor.rs`), usando os padrões já estabelecidos ali. O HTML serve como especificação visual e de interação.

Mapeamento sugerido para o código existente:

| Parte do design | Arquivos do repositório |
| --- | --- |
| Editor de captura (barra flutuante) | `src/editor.rs`, `src/editor/`, `src/capture.rs`, `src/capture/` |
| Configurações | `src/settings_window.rs`, `src/config.rs`, `src/global_shortcut.rs`, `src/autostart.rs` |
| Bandeja e menu rápido | `src/tray.rs`, `src/daemon.rs`, `src/launcher.rs` |
| Notificação pós-salvar | `src/notify.rs` |

## Fidelity

**Alta fidelidade.** Cores, tipografia, espaçamentos, raios e estados de interação são finais e devem ser reproduzidos com precisão. Onde o GTK impuser convenções próprias (bordas de janela, sombras, animações do compositor), seguir o GTK.

O desktop falso ao fundo (barra superior do GNOME, terminal, gerenciador de arquivos) é **cenário do protótipo** e não deve ser implementado — existe apenas para mostrar a barra flutuante e o congelamento em contexto.

---

## Design Tokens

### Cores — marca (derivadas do mascote)

| Token | Hex | Uso |
| --- | --- | --- |
| Lilás escuro | `#7E3E7C` | Acento primário no tema claro: botões, switches ligados, segmentos ativos |
| Lilás médio | `#8E4C8C` | Rótulos de seção, ferramenta ativa no tema escuro |
| Lilás claro | `#B65FB3` | Acento primário no tema escuro |
| Rosa mascote | `#FCD0EA` | Salvar (fundo), quinas do recorte, mira da lupa, monospace de destaque |
| Rosa profundo | `#5D2456` | Ícone sobre `#FCD0EA` |
| Marrom mascote | `#5D2E1D` / `#241715` | Texto no tema claro |
| Laranja mascote | `#E58635` | Cursor do terminal, swatch de anotação |
| Telha mascote | `#CE6042` | Cancelar (destrutivo) |
| Vermelho anotação | `#E51919` | Cor de anotação padrão |

### Cores — tema claro (janela de configurações)

`bg #FCFAFB` · `fg #241715` · `head #F6EAF4` · `headLine #E9D8E6` · `card #FFFFFF` · `line #E9DDE5` · `rowLine #F1E8ED` · `sub #8A7A82` · `label #8E4C8C` · `input #FCFAFB` · `inputLine #E0CFDC` · `note #F6EAF4` · `noteFg #5D4A54` · `chip rgba(36,23,21,0.06)` · `accent #7E3E7C` · `onAccent #FFFFFF` · `off #D8CBD3` (switch desligado) · `link #7E3E7C` · `hover rgba(36,23,21,0.12)` · `tabIdle #6B4C66`

### Cores — tema escuro

`bg #1E141C` · `fg #F3EAF0` · `head #2A1B27` · `headLine #3B2A38` · `card #261A24` · `line #3A2836` · `rowLine #33222F` · `sub #A99AA4` · `label #E6AFE0` · `input #1B1219` · `inputLine #4A3446` · `note #2E1F2B` · `noteFg #CFC2CB` · `chip rgba(255,255,255,0.08)` · `accent #B65FB3` · `onAccent #FFFFFF` · `off #4A3446` · `link #E6AFE0` · `hover rgba(255,255,255,0.14)` · `tabIdle #C9BAC4`

Preset de nome selecionado: claro `#F0DCEC`, escuro `#3A2437`.

O tema tem três estados: **Claro**, **Escuro**, **Sistema** (segue `prefers-color-scheme` / preferência do desktop e reage a mudanças em tempo real).

### Tipografia

- Interface: **Cantarell**, fallback `system-ui, sans-serif`.
- Modo acessível: **OpenDyslexic** (400/700), aplicada a toda a interface quando ativada. Empacotar a fonte junto do app (licença livre para uso pessoal e comercial) em vez de baixar de CDN.
- Monospace: **JetBrains Mono** — caminhos, nomes de arquivo, códigos do padrão de nome, teclas de atalho, coordenadas da lupa. Permanece monospace inclusive no modo dislexia.

Escala usada: 10.5 / 11 / 11.5 / 12 / 12.5 / 13 px. Rótulos de seção: 11px, peso 700, `letter-spacing 0.09em`, maiúsculas. Títulos de linha: 13px/700. Legendas: 11.5px na cor `sub`.

### Raios, sombras, espaçamento

- Raios: janela 14px · cards 12px · linhas internas/segmentos 7–9px · botões da barra 10px · salvar 11px · chips de código 7px.
- Sombras: janela de configurações `0 34px 80px rgba(0,0,0,0.55)`; barra flutuante `0 16px 40px rgba(0,0,0,0.38)`; notificação `0 20px 50px rgba(0,0,0,0.5)`; botão salvar `0 3px 12px` do próprio acento a 40–45%.
- Padding de linha de configuração: `11px 14px`. Corpo da janela: `18px 20px 22px`, `gap 20px` entre seções.

### Animações

- `pcFlash` 420ms ease-out — flash branco da captura (0 → 0.9 em 14% → 0).
- `pcPop` 120–160ms ease-out — janela, menu da bandeja, painel de códigos.
- `pcRise` 200ms ease-out — notificação subindo 12px.
- Switches: knob com `transition: margin-left .16s ease`.

---

## Screens / Views

### 1. Editor de captura (barra flutuante)

**Propósito:** anotar e salvar a captura recém-congelada.

**Layout:** overlay em tela cheia sobre a imagem congelada, com véu `rgba(20,12,20,0.16)`. A barra é uma **linha única** centralizada em `top: 18px`, `transform: translateX(-50%)`, `flex-wrap: nowrap`, `padding 6px`, `gap 6px`, `border-radius 14px`. Nunca deve quebrar em duas linhas.

**Duas variantes** (escolher uma; o protótipo tem alternador no canto inferior esquerdo):

- **Dock lilás** (preferida): fundo `rgba(102,46,100,0.72)`, `backdrop-filter: blur(18px) saturate(1.3)`, borda `1px rgba(252,208,234,0.22)`.
- **OSD escuro**: fundo `rgba(24,16,23,0.6)`, `blur(18px) saturate(1.2)`, borda `1px rgba(255,255,255,0.14)`.

A transparência é requisito funcional: o usuário precisa ver o conteúdo atrás da barra.

**Grupos, na ordem** (separados por divisores de `1px × 26px` em `rgba(255,255,255,0.14–0.22)`):

1. **Ferramentas** — 8 botões de 36×36px, raio 10px, `gap 3px`: recorte, seta, retângulo, elipse, caneta livre, texto, desfoque, numeração de passos. Ativa: fundo `#FCD0EA` com ícone `#5D2456` (dock) ou fundo `#8E4C8C` com ícone branco (OSD). Inativa: transparente, ícone em `rgba(255,255,255,0.8–0.85)`. Ícones em traço de 1.9px, 19×19px.
2. **Cores** — 5 círculos de 20px: `#E51919`, `#E58635`, `#7E3E7C`, `#FCFAFB`, `#241715`. Selecionada: anel `2px #FCD0EA`; demais `2px rgba(255,255,255,0.28)`. Sombra interna `0 0 0 1px rgba(0,0,0,0.18)`.
3. **Espessura** — 3 botões de 28px (raio 8px) com pontos de 5 / 8 / 12px → traços de 2 / 4 / 8px. Selecionado: fundo `rgba(255,255,255,0.14–0.2)`, ponto `#FCD0EA`.
4. **Ações** — 4 botões de 34px: desfazer, refazer, fixar na tela (fundo `rgba(252,208,234,0.28)` quando ativo), copiar.
5. **Confirmação** — dois **ícones**, sem texto:
   - **Cancelar**: 36×36px, raio 10px, fundo `rgba(206,96,66,0.3–0.34)`, ícone `#FFDCCE`; hover fundo `#CE6042` com ícone branco. Ícone: X, traço 2.3px. Tooltip "Cancelar (Esc)".
   - **Salvar**: 38×38px (maior que os demais, é o primário), raio 11px, fundo `#FCD0EA` com ícone `#5D2456` na dock / `#B65FB3` com ícone branco no OSD, sombra `0 3px 12px` do acento a 40–45%; hover `#FFFFFF` / `#C972C6`. Ícone: seta para baixo sobre uma linha (gravar arquivo), traço 2.2px, 19×19px. Tooltip "Salvar na pasta (Ctrl+S)".

**Ferramenta de recorte:**

- Moldura de seleção: `outline: 3px dashed rgba(255,255,255,0.95)`, `outline-offset: 0`, tracejado (não pontilhado).
- Fora da seleção: escurecido com `box-shadow: 0 0 0 9999px rgba(14,8,14,0.5)`.
- **Quinas:** 4 quadrados de **12×12px** em `#FCD0EA`, posicionados a `-3px` de cada quina (`left/right/top/bottom`), `z-index` acima da moldura. A borda externa do quadrado coincide exatamente com a externa do tracejado — cobre o tracejado na quina **sem ultrapassá-lo**. Sem borda escura.
- Etiqueta de dimensões acima da seleção: `26px` acima, fundo `rgba(14,8,14,0.8)`, monospace 10.5px, formato `largura × altura`.
- **Lupa** (só com a ferramenta de recorte ativa): lente circular de 96px seguindo o cursor com offset `+22px, +22px`; borda `2px rgba(255,255,255,0.9)`, sombra `0 8px 22px rgba(0,0,0,0.5)`. Dentro: pixels ampliados da captura congelada com grade de 12px, mira horizontal/vertical em `rgba(252,208,234,0.85)` e quadrado central de 12px com `box-shadow: 0 0 0 1.5px #FCD0EA, 0 0 0 3px rgba(45,20,42,0.5)` marcando o pixel exato. Etiqueta abaixo (monospace 10.5px, fundo `rgba(14,8,14,0.82)`): `x, y` antes de arrastar; `largura × altura` durante o arraste. Desaparece ao trocar de ferramenta ou sair da área.
  - No protótipo a grade é uma representação; na implementação real deve mostrar os pixels ampliados da captura.

**Nota:** o recorte **não** salva automaticamente — a seleção permanece na tela e o usuário salva pelo botão.

**Anotações:**

- Seta: linha + duas farpas com ângulo de `0.44 rad` e comprimento `11 + espessura × 2.4`.
- Retângulo: raio 3px. Elipse: `border-radius 50%`. Ambos apenas contorno, espessura = espessura selecionada.
- Caneta livre: polilinha com `stroke-linecap/linejoin: round`.
- Texto: campo inline no ponto do clique (`padding 5px 8px`, fundo `rgba(14,8,14,0.82)`, borda `1px rgba(255,255,255,0.5)`, raio 7px, placeholder "digite e Enter"); Enter confirma, Esc descarta; tamanho final `14 + espessura × 3` px, peso 700, sombra `0 1px 2px rgba(0,0,0,0.35)`.
- Desfoque: retângulo com `backdrop-filter: blur(9px)` e `background rgba(255,255,255,0.04)`, raio 2px.
- Numeração: círculo de 30px na cor selecionada, número branco 15px/700, sombra `0 2px 8px rgba(0,0,0,0.35)`; contador incrementa a cada clique e decrementa ao desfazer.

**Arraste:** só cria a forma se o movimento passar de 4px em x ou y. Ferramentas de clique único: texto e numeração.

### 2. Configurações

**Propósito:** definir atalhos, destino, aparência.

**Layout:** janela de `min(560px, 92%)` de largura, `max-height: calc(100% - 96px)`, raio 14px, coluna flex.

**Cabeçalho** (`padding 10px 12px`, fundo `head`, borda inferior `headLine`): mascote 28px circular · alternador de abas **Configurações / Histórico** (fundo `chip`, raio 9px, aba ativa com fundo `accent` e texto `onAccent`, inativa `tabIdle`) · botão fechar 28px circular à direita (fundo `chip`, hover `hover`).

**Corpo** — três seções com rótulo em maiúsculas (`label`) e um card por seção:

**Atalhos de captura** (card `line`/`card`, linhas separadas por `rowLine`):
- Tela cheia — "Captura a tela inteira e abre o editor" — `PrtSc`
- Região — "Já entra com a seleção pronta pra arrastar" — `Shift + PrtSc`
- Janela ativa — "Só a janela em foco" — `Alt + PrtSc`

Cada linha tem à direita um botão monospace 11.5px com a combinação atual. Ao clicar, entra em modo de regravação: o rótulo vira "Pressione…", fundo `accent`, texto `onAccent`, e a próxima combinação pressionada é gravada (modificadores na ordem Ctrl, Alt, Shift, Super; `PrintScreen` exibido como `PrtSc`; espaço como `Espaço`; letras em maiúscula). Clicar de novo cancela.

**Destino:**
- Pasta das capturas — caminho em monospace (`~/Imagens/printcher`) + botão "Escolher…" (abre o seletor de pastas).
- Formato — segmentado PNG / JPG / WebP.
- Padrão de nome do arquivo — campo de texto monospace (padrão `printcher_%Y-%m-%d_%H-%M-%S`), foco muda a borda para `accent`. Abaixo, "Fica assim: `<nome real>`" atualizando ao vivo.
  - À direita do título, um **link de expandir** (sem caixa nem borda — não deve parecer um select): ícone **+** que vira **−** e texto sublinhado "Ver códigos e exemplos" / "Ocultar códigos e exemplos", na cor `link`.
  - Aberto, revela painel `note` com: uma frase de ajuda; **códigos clicáveis** que inserem no campo — `%Y` ano, `%m` mês, `%d` dia, `%H` hora, `%M` minuto, `%S` segundo, `%n` contador (3 dígitos); e **3 exemplos** clicáveis mostrando padrão + nome gerado (`printcher_%Y-%m-%d_%H-%M-%S`, `print_%d-%m-%Y_%Hh%M`, `captura_%n`), com o atual destacado.
- Copiar sempre para a área de transferência — "Além de salvar o arquivo" — switch (44×26px, knob 20px, deslocamento 18px).

**Geral:**
- Iniciar com o sistema — "Sobe em segundo plano no login" — switch.
- Ícone na bandeja — "Acesso rápido a capturar e configurar" — switch.
- Tema — segmentado Claro / Escuro / Sistema.
- Fonte — segmentado **Padrão** / **Amigável para dislexia**, legenda "Letras mais fáceis de ler". Não citar o nome da fonte na interface.

**Rodapé da aba** — faixa `note` com o texto "Os atalhos funcionam em qualquer tela do computador, mesmo com esta janela fechada." e o botão **Capturar agora** (`accent`, raio 9px). Linguagem deliberadamente não técnica: o público não é técnico, então nada de portal, compositor, X11 ou Wayland na interface.

### 3. Histórico (aba)

Grade `repeat(auto-fill, minmax(180px, 1fr))`, `gap 12px`. Cada card: miniatura de 104px, nome do arquivo em monospace 10.5px truncado e linha "quando · formato" em `sub`. Ordem mais recente primeiro; a captura recém-salva entra no topo.

### 4. Menu da bandeja

Popover de 236px, raio 14px, ancorado no ícone do mascote: cabeçalho com mascote 26px + "printcher" / "em segundo plano"; itens "Capturar agora" (com o atalho à direita em monospace), "Configurações…", "Histórico de capturas"; divisor; "Encerrar" em `sub`. Hover das linhas: `#F6EAF4`. No protótipo o menu permanece claro nos dois temas — decidir se deve seguir o tema na implementação.

### 5. Notificação pós-salvar

Canto inferior direito, `bottom/right 18px`, `max-width min(360px, 80%)`: miniatura 52×38px, título ("Salva e copiada" quando a cópia automática está ligada, "Captura salva" quando não) e caminho completo em monospace truncado, mais o botão "Abrir pasta". Fundo `rgba(20,13,19,0.9)` com `blur(14px)`. Some sozinha em 4,6s.

---

## Interactions & Behavior

- `PrtSc` (ou `P` no protótipo) → flash + congelamento + editor com a ferramenta inicial ativa. Ignorado se o editor já estiver aberto.
- `Esc` → fecha, em ordem: editor → menu da bandeja → configurações.
- `Ctrl+Z` desfazer · `Ctrl+S` salvar · `Ctrl+C` copiar (só com o editor aberto).
- Teclas são ignoradas enquanto o foco está em `input`/`textarea`, exceto no modo de regravação de atalho, que captura tudo.
- Salvar: fecha o editor, adiciona ao topo do histórico (máx. 8 no protótipo) e mostra a notificação.
- Cancelar: fecha o editor e descarta as anotações.
- Desfazer/refazer: pilha de operações; desfazer decrementa o contador de numeração; qualquer nova operação limpa a pilha de refazer.

## State Management

Estado do protótipo, que se traduz direto em estado do app:

- `editorOpen`, `settingsOpen`, `tab` (`cfg`/`hist`), `trayOpen`, `tokensOpen`
- `tool`, `color`, `width`, `stepN`, `pinned`
- `ops[]` (operações confirmadas), `redoStack[]`, `drag` (arraste em curso), `textDraft`, `cur` (cursor, para a lupa)
- `flash`, `toast`, `history[]`
- `cfg`: `full`, `region`, `win`, `folder`, `format`, `pattern`, `autoCopy`, `autostart`, `tray`, `theme`, `font`
- `capturing` (qual atalho está sendo regravado), `sysDark` (preferência do sistema)

Persistência: tudo em `cfg` deve ir para o arquivo de configuração (`src/config.rs`).

## Assets

- `assets/printcher-mascot.svg` — mascote fornecido pelo usuário; usado no ícone da bandeja e no cabeçalho das configurações, sempre recortado em círculo. Paleta da marca derivada dele.
- Ícones da barra e da interface: desenhados como SVG de traço (1.9–2.8px, cantos arredondados). Substituir pelo conjunto de ícones simbólicos do GTK onde houver equivalente.
- OpenDyslexic 400/700 — empacotar com o app.

## Files

- `Printcher.dc.html` — protótipo completo e interativo (desktop simulado, editor com as duas variantes de barra, configurações, histórico, bandeja, notificação).
- `assets/printcher-mascot.svg` — mascote.
