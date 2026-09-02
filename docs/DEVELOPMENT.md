# Desenvolvimento

Notas técnicas: arquitetura, decisões de design, status por funcionalidade
e o que falta pra empacotar. Pra visão geral do produto (o que é, como
instalar/usar), veja o [`README.md`](../README.md).

## Stack

- Rust (toolchain via `rustup`, sempre na última stable)
- GTK4 + libadwaita para a interface
- Captura: X11 nativo (`x11rb`) e Wayland via `xdg-desktop-portal` (`ashpd`)

## Testes

```bash
cargo test
```

O projeto mistura lógica pura (fácil de testar) com bastante código de
integração/"cola" que só funciona com uma sessão gráfica real (GTK, D-Bus,
portais do xdg-desktop-portal) — isso não é unitariamente testável sem
mocks artificiais que testariam o mock, não o código. A estratégia:

- **Lógica pura, com testes automatizados de verdade:**
  `src/editor/render.rs` (geometria, composição da imagem final — inclusive
  verificando pixels de verdade no PNG resultante), `src/capture/x11.rs`
  (conversão de bytes BGRX/XRGB pra RGBA), `src/config.rs`,
  `src/autostart.rs`, `src/launcher.rs` (geração de arquivos `.desktop`,
  isolados do sistema real via `XDG_CONFIG_HOME`/`XDG_DATA_HOME` —
  `src/testutil.rs`), e o mapeamento de comandos em `src/daemon.rs`
  (`InitialAction`).
- **Cola de GTK/D-Bus/portal, validada manualmente:** `src/main.rs`,
  a maior parte de `src/daemon.rs`, `src/editor.rs` (só a construção da
  janela — a lógica dela mora em `render.rs`), `src/settings_window.rs`,
  `src/tray.rs`, `src/global_shortcut.rs`, `src/notify.rs`,
  `src/capture.rs`/`src/capture/wayland.rs`. Validado do mesmo jeito que
  fizemos durante o desenvolvimento: subir o daemon em segundo plano e
  checar via D-Bus/`pgrep`/arquivos gerados (sem tela) — o que depende de
  interação visual real (editor, notificações, diálogos) fica registrado
  como pendente no `README`/`CHANGELOG` até ser validado numa tela de
  verdade.

Cobertura (medida com [`cargo-tarpaulin`](https://github.com/xd009642/tarpaulin),
excluindo os arquivos de cola acima):

```bash
cargo tarpaulin --exclude-files 'src/main.rs' --exclude-files 'src/daemon.rs' \
  --exclude-files 'src/notify.rs' --exclude-files 'src/tray.rs' \
  --exclude-files 'src/settings_window.rs' --exclude-files 'src/global_shortcut.rs' \
  --exclude-files 'src/capture.rs' --exclude-files 'src/capture/wayland.rs' \
  --exclude-files 'src/editor.rs'
```

Resultado atual: **92,6%** da lógica testável (`src/editor/render.rs` em
100%). Rodar sem os `--exclude-files` mostra a cobertura do repositório
inteiro (~26%) — número baixo esperado, já que a maior parte do código é
cola de integração por natureza, não falta de testes.

## Status por funcionalidade

- ✅ Captura full screen (Wayland/GNOME) — validado
- ✅ Editor de captura (crop, setas, formas, texto) — validado
- ✅ Copiar para a área de transferência — validado
- 🚧 Backend X11: implementado e compilando, **ainda não testado em sessão
  X11 real** (só temos GNOME Wayland disponível até agora)
- ✅ Processo em segundo plano (daemon de instância única + autostart) —
  validado de ponta a ponta via D-Bus
- 🚧 Atalho global via portal (`GlobalShortcuts`): implementado, degrada
  graciosamente, mas **o atalho de tecla em si só funciona rodando como
  Flatpak** (o portal exige identidade de app — rejeita o binário "cru" com
  `An app id is required`). Capturar via D-Bus/tray continua funcionando
  normalmente mesmo sem isso.
- 🚧 Ícone na bandeja (StatusNotifierItem via `ksni`): implementado, degrada
  graciosamente. Testado aqui: sem a extensão "AppIndicator and
  KStatusNotifierItem Support" (não vem por padrão no Fedora), o registro
  falha com `ServiceUnknown` e o daemon segue funcionando normalmente sem
  ícone. Nativo no KDE, sem extensão nenhuma.
- 🚧 Tela de configurações (`src/settings_window.rs`, via libadwaita):
  implementada e validada de ponta a ponta via D-Bus/`pgrep` (primeira
  execução liga autostart sozinha, ícone de launcher abre/reaproveita o
  daemon corretamente, encerramento limpo). O **conteúdo visual da janela e
  o diálogo de confirmação ao fechar** ainda não foram vistos numa tela de
  verdade.
- 🚧 Feedback pro usuário (`src/notify.rs`, banner de boas-vindas,
  `--uninstall-all`): implementado e testado sem tela (primeira execução,
  desinstalação completa). **Notificações e banner ainda não vistos
  aparecendo de verdade** — só sei que o código não crasha ao montá-los.
- 🚧 Empacotamento (Flatpak): manifesto escrito, **build ainda não testado**
  (falta `flatpak-builder` e o download do runtime/SDK) — agora também
  pré-requisito real pro atalho global funcionar

## Fluxo de captura (estratégia ShareX)

Ao capturar, a tela cheia é congelada (imagem estática) e aberta em um editor
próprio, em tela cheia. Anotações (setas, formas, texto) podem ser feitas
sobre a imagem inteira, e o **crop é apenas mais uma ferramenta** da barra,
não uma etapa obrigatória logo no início — igual ao ShareX. Isso evita a
necessidade de overlay ao vivo sobre a tela (não suportado no GNOME Wayland
sem `layer-shell`): a captura via portal já entrega o bitmap parado, e toda a
edição (crop, setas, formas) acontece localmente sobre essa imagem.

A escolha de backend (`src/capture.rs`) é automática: se `WAYLAND_DISPLAY`
estiver definida, usa o portal; senão, cai para a conexão X11 direta
(`src/capture/x11.rs`, via `x11rb`, captura a janela raiz com `GetImage`).
Os dois caminhos produzem o mesmo PNG de saída, então o editor não precisa
saber qual foi usado.

## Processo em segundo plano e atalho global

O `printcher` é um daemon de instância única, com um pequeno protocolo de
IPC próprio via D-Bus (`com.printcher.Printcher`):

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
`org.freedesktop.portal.GlobalShortcuts` (não via `gsettings` — isso
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
manual na instalação. Essa primeira janela também mostra um grupo extra
"Bem-vindo ao printcher!" chamando atenção pro atalho de captura logo
abaixo, já que configurar a tecla é o único passo manual que sobra.

Fechar a janela pergunta se você quer encerrar o printcher por completo ou
deixá-lo em segundo plano (é isso que mantém o atalho global e a bandeja
ativos) — a captura sempre continua funcionando enquanto o processo estiver
de pé, então fechar a janela sem querer não desliga nada por engano.

## Notificações do sistema

O printcher roda em segundo plano, sem terminal visível — então erros e
confirmações que antes só iam pro `eprintln!` (invisíveis num uso real)
também mandam uma notificação do sistema via
`org.freedesktop.portal.Notification` (`src/notify.rs`, mesmo portal do
Screenshot/GlobalShortcuts, sem permissão nova no Flatpak):

- Falha ao capturar a tela ou abrir o editor.
- Botão Salvar do editor: sucesso ("Captura salva") ou falha.
- Botão Copiar do editor: sucesso ("Copiado para a área de transferência")
  ou falha — antes não dava feedback nenhum.

## Empacotamento (Flatpak) e preparação pro Flathub

Manifesto em `flatpak/com.printcher.Printcher.json` — App ID
`com.printcher.Printcher` (domínio `printcher.com`, de propriedade do
autor), runtime `org.gnome.Platform` 50 + extensão `rust-stable`. Build
**100% offline**: as ~350 dependências do Rust já vêm vendorizadas dentro
do próprio manifesto (geradas com o `flatpak-cargo-generator.py` oficial a
partir do `Cargo.lock`), sem precisar de `--share=network`. Ainda não
testado de verdade (precisa de `flatpak-builder`, que baixa o runtime/SDK —
vários GB). Pra buildar:

```bash
sudo dnf install flatpak-builder
flatpak-builder --user --install build-dir flatpak/com.printcher.Printcher.json
```

Se alguma dependência mudar no `Cargo.toml`/`Cargo.lock`, as fontes
vendorizadas no manifesto precisam ser regeradas (`flatpak-cargo-generator.py
Cargo.lock -o cargo-sources.json`, depois mesclar no manifesto).

Também tem, na pasta `flatpak/`:
- `com.printcher.Printcher.desktop` — launcher instalado dentro do pacote
  (diferente do `--install-launcher` local, que grava no host).
- `com.printcher.Printcher.metainfo.xml` — metadados AppStream (nome,
  descrição, changelog). **Falta adicionar screenshots reais** antes de
  submeter — ainda não validamos a UI numa tela de verdade.
- `com.printcher.Printcher.svg` — ícone placeholder (substituir por um
  oficial quando tiver).

As permissões (`finish-args`) do manifesto estão comentadas explicando o
motivo de cada uma. Um ponto pendente identificado nessa revisão: o
`autostart.rs` grava o `.desktop` com o caminho absoluto do binário
(`std::env::current_exe()`), que não funciona de dentro do sandbox do
Flatpak — o `Exec=` precisaria virar `flatpak run com.printcher.Printcher
--daemon`. Fica como ajuste pendente pra quando testarmos o build de
verdade.

### Licença

GPLv3 (`LICENSE`) — qualquer trabalho derivado precisa continuar aberto,
mas permite uso comercial (exigência do Flathub). Se quisesse restringir uso
comercial, teria que abrir mão de submeter ao Flathub — foi essa a troca
feita conscientemente.

### Submissão ao Flathub

Fica pra quando tivermos uma primeira versão validada de verdade. Falta,
nessa hora: screenshots reais no metainfo, ícone oficial, corrigir o
autostart pra sandbox, e passar pelo processo de revisão deles (PR num repo
próprio + `flatpak-builder-lint` + revisão humana).
