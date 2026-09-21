//! Janela do editor de captura: construção de widgets GTK e ligação dos
//! eventos (mouse, teclado, botões) com a lógica pura em [`render`]. Não
//! tem testes automatizados aqui — depende de uma sessão gráfica real pra
//! rodar, então é validado manualmente (veja `docs/DEVELOPMENT.md`).

mod icons;
mod render;

use std::cell::RefCell;
use std::fs::File;
use std::rc::Rc;

use gtk::cairo;
use gtk::gdk;
use gtk::glib;
use gtk::prelude::*;

use render::{AppState, Annotation, CropRect, Point, Tool};

/// Distância do topo da janela até a barra flutuante -- usada tanto na
/// barra quanto no "vidro" de fundo, pra ficarem alinhados.
const TOOLBAR_TOP_OFFSET: i32 = 18;

/// As 5 cores fixas da barra (design tokens) -- substituem o seletor de cor
/// livre por algo que bate com a identidade visual da marca.
const PALETTE: [render::Color; 5] = [
    (0.898, 0.098, 0.098), // #E51919
    (0.898, 0.525, 0.208), // #E58635
    (0.494, 0.243, 0.486), // #7E3E7C
    (0.988, 0.980, 0.984), // #FCFAFB
    (0.141, 0.090, 0.082), // #241715
];

/// As 3 espessuras de traço do design (Fina/Média/Grossa).
const WIDTHS: [f64; 3] = [2.0, 4.0, 8.0];

/// Arrasto precisa passar desse tanto (em pixels de imagem) em x ou y pra
/// virar anotação -- sem isso, um clique com a mão meio trêmula já criava
/// uma forma minúscula sem querer.
const MIN_DRAG: f64 = 4.0;

/// Cores dos ícones -- ao contrário de um ícone de tema, o que a gente
/// desenha é um bitmap já pronto, então cada contexto (ferramenta
/// ativa/inativa, ação sobre o fundo lilás, etc.) precisa da sua própria
/// textura já na cor certa (ver `icons::Color`/`icons::render`).
const ICON_INACTIVE: icons::Color = (1.0, 1.0, 1.0, 0.85); // ícone de ferramenta não selecionada
const ICON_ACTIVE: icons::Color = (1.0, 1.0, 1.0, 1.0); // ferramenta selecionada -- só o fundo do botão muda (branco neutro, sem cor de marca)
const ICON_ON_DOCK: icons::Color = (1.0, 1.0, 1.0, 0.88); // desfazer/refazer/copiar, sempre sobre o fundo neutro
const ICON_CANCEL: icons::Color = (1.0, 1.0, 1.0, 0.95); // sobre o fundo vermelho nativo (.destructive-action)

/// Folha de estilo da barra flutuante -- cores/raios/tamanhos exatos do
/// design (`design_handoff_printcher/README.md`). Widgets comuns do GTK não
/// têm propriedade "background color" nem "border radius" configurável por
/// código sem CSS; por isso uma folha própria em vez de tentar montar isso
/// via builders.
const TOOLBAR_CSS: &str = "
.pc-toolbar {
    background-color: transparent;
    border-radius: 14px;
    padding: 6px;
}
.pc-divider {
    background-color: rgba(255,255,255,0.18);
    min-width: 1px;
    min-height: 26px;
}
.pc-tool {
    min-width: 36px;
    min-height: 36px;
    padding: 0;
    border-radius: 10px;
    background-color: transparent;
    color: rgba(255,255,255,0.85);
}
.pc-tool:hover:not(:checked) {
    background-color: rgba(255,255,255,0.10);
}
.pc-tool:checked {
    background-color: rgba(255,255,255,0.18);
    color: #FFFFFF;
}
.pc-action {
    min-width: 34px;
    min-height: 34px;
    padding: 0;
    border-radius: 9px;
    background-color: transparent;
    color: rgba(255,255,255,0.88);
}
.pc-action:hover {
    background-color: rgba(255,255,255,0.14);
}
.pc-swatch {
    min-width: 20px;
    min-height: 20px;
    padding: 0;
    border-radius: 9999px;
    border: 2px solid rgba(255,255,255,0.28);
    box-shadow: 0 0 0 1px rgba(0,0,0,0.18) inset;
}
.pc-swatch.selected {
    border-color: #FFFFFF;
}
.pc-swatch-0 { background-color: #E51919; }
.pc-swatch-1 { background-color: #E58635; }
.pc-swatch-2 { background-color: #7E3E7C; }
.pc-swatch-3 { background-color: #FCFAFB; }
.pc-swatch-4 { background-color: #241715; }
.pc-width {
    min-width: 28px;
    min-height: 28px;
    padding: 0;
    border-radius: 8px;
    background-color: transparent;
}
.pc-width.selected {
    background-color: rgba(255,255,255,0.18);
}
.pc-dot {
    border-radius: 9999px;
    background-color: rgba(255,255,255,0.7);
}
.pc-dot.selected {
    background-color: #FFFFFF;
}
.pc-cancel {
    min-width: 36px;
    min-height: 36px;
    padding: 0;
    border-radius: 10px;
}
.pc-save {
    min-width: 38px;
    min-height: 38px;
    padding: 0;
    border-radius: 11px;
}
";

/// Abre uma janela do editor de captura (crop + anotações) sobre a imagem
/// congelada, associada à `Application` do daemon já em execução. Não
/// bloqueia: a janela fica sob o controle do loop principal do GTK que já
/// está rodando. `runtime` é usado só pra mandar notificações do sistema
/// (Salvar/Copiar rodam no thread principal do GTK, não numa task tokio).
/// `window_slot` guarda a janela do editor atualmente aberta (se houver):
/// uma nova captura antes de salvar/cancelar a anterior fecha a antiga
/// primeiro -- ter duas janelas de editor ao mesmo tempo é o que deixava a
/// mais nova sem foco nenhum (clique e Esc iam pra janela escondida atrás).
pub fn open_editor_window(
    app: &gtk::Application,
    image_path: std::path::PathBuf,
    runtime: tokio::runtime::Handle,
    window_slot: &Rc<RefCell<Option<gtk::ApplicationWindow>>>,
) -> anyhow::Result<()> {
    // Mesmo cuidado que em daemon.rs::handle_capture: não dá pra fazer isso
    // num `if let` só, porque o `RefMut` fica vivo até o fim do bloco e
    // `old.destroy()` dispara "destroy" na hora, que tenta pegar esse mesmo
    // RefCell emprestado de novo (ver `connect_destroy` no fim desta
    // função) -- panic de "RefCell already borrowed".
    let old = window_slot.borrow_mut().take();
    if let Some(old) = old {
        old.destroy();
    }

    let mut file = File::open(&image_path)?;
    let image = cairo::ImageSurface::create_from_png(&mut file)
        .map_err(|e| anyhow::anyhow!("falha ao carregar captura: {e:?}"))?;
    let temp_path = image_path.clone();

    // Ferramenta inicial é Cortar (não Selecionar): reproduz o fluxo do
    // GNOME Screenshot/Lightshot -- a tela já congela pronta pra você
    // arrastar uma seleção, sem precisar clicar em nada primeiro.
    let state = Rc::new(RefCell::new(AppState {
        image,
        tool: Tool::Crop,
        color: PALETTE[0],
        stroke_width: WIDTHS[1],
        annotations: Vec::new(),
        redo_stack: Vec::new(),
        drag_start: None,
        drag_current: None,
        drag_points: Vec::new(),
        step_n: 1,
        cursor: None,
    }));

    let window = gtk::ApplicationWindow::builder()
        .application(app)
        .title("printcher — editor de captura")
        .build();

    // A imagem ocupa a janela inteira; a barra de ferramentas flutua por
    // cima (como um dock), em vez de dividir o espaço com ela -- por isso
    // `Overlay` no lugar de uma Box vertical simples.
    let overlay = gtk::Overlay::new();
    window.set_child(Some(&overlay));

    // Sem ScrolledWindow: a imagem (em resolução física, que em telas HiDPI
    // é maior do que a área lógica disponível) é sempre escalada pra caber
    // inteira na janela -- ver `scale_factor()`/o draw_func abaixo. Isso
    // evita o corte que acontecia antes (a área de desenho pedia o tamanho
    // físico da imagem, maior que o espaço lógico da tela, e ficava sem
    // rolagem visível pro resto).
    let area = gtk::DrawingArea::new();
    area.set_hexpand(true);
    area.set_vexpand(true);
    overlay.set_child(Some(&area));

    // Folha de estilo da barra -- ver `TOOLBAR_CSS`, carregada uma vez por
    // display (idempotente: recarregar não duplica nada, só substitui).
    let css_provider = gtk::CssProvider::new();
    css_provider.load_from_string(TOOLBAR_CSS);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(&display, &css_provider, gtk::STYLE_PROVIDER_PRIORITY_APPLICATION);
    }

    // Dock flutuante no topo, centralizado -- fundo translúcido arredondado
    // que não precisa de nenhum espaço próprio no layout, então a imagem
    // sempre usa a janela inteira. O "vidro fosco" atrás da barra é uma
    // segunda camada (`toolbar_backdrop`, ver mais abaixo) desenhada por
    // trás dela, na mesma posição e tamanho -- o GTK/Wayland não tem
    // backdrop-filter de verdade pra janelas comuns.
    // Só margem de posicionamento (18px do topo da janela) -- a respiração
    // ao redor dos botões já vem inteira do `padding: 6px` do CSS acima. O
    // GTK soma margem ao tamanho medido do widget (documentado: "margin
    // will be added in addition to the size from set_size_request"), então
    // qualquer margem aqui também precisa ser descontada de `toolbar_h`
    // mais abaixo, ou o "vidro" de fundo (medido a partir deste widget)
    // fica bem mais alto que a barra de botões de verdade.
    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    toolbar.add_css_class("pc-toolbar");
    toolbar.set_halign(gtk::Align::Center);
    toolbar.set_valign(gtk::Align::Start);
    toolbar.set_margin_top(TOOLBAR_TOP_OFFSET);

    // Calcula o fator de escala pra caber a imagem inteira na área
    // disponível (nunca corta, encolhe ou aumenta conforme o necessário).
    let scale_factor = {
        let state = state.clone();
        let area = area.clone();
        move || -> f64 {
            let (img_w, img_h) = {
                let state = state.borrow();
                (state.image.width() as f64, state.image.height() as f64)
            };
            let (aw, ah) = (area.width() as f64, area.height() as f64);
            if img_w <= 0.0 || img_h <= 0.0 || aw <= 0.0 || ah <= 0.0 {
                1.0
            } else {
                (aw / img_w).min(ah / img_h)
            }
        }
    };

    fn divider() -> gtk::Box {
        let d = gtk::Box::new(gtk::Orientation::Vertical, 0);
        d.add_css_class("pc-divider");
        d
    }

    // --- Ferramentas ---
    // Cada ferramenta precisa de duas texturas (inativa/ativa) porque o
    // ícone é um bitmap já pronto, não um ícone de tema que o GTK recolore
    // sozinho -- ver `icons.rs`.
    type ToolIcons = fn(icons::Color) -> gdk::Texture;
    let tools: [(&str, Tool, ToolIcons); 7] = [
        ("Cortar", Tool::Crop, icons::crop),
        ("Seta", Tool::Arrow, icons::arrow),
        ("Retângulo", Tool::Rect, icons::rect),
        ("Elipse", Tool::Ellipse, icons::ellipse),
        ("Caneta livre", Tool::Pen, icons::pen),
        ("Desfoque", Tool::Blur, icons::blur),
        ("Numeração", Tool::Step, icons::step),
    ];
    let mut leader: Option<gtk::ToggleButton> = None;
    let mut crop_btn: Option<gtk::ToggleButton> = None;
    for (label, tool, icon_fn) in tools {
        let inactive_texture = icon_fn(ICON_INACTIVE);
        let active_texture = icon_fn(ICON_ACTIVE);
        let image = gtk::Image::from_paintable(Some(&inactive_texture));
        let btn = gtk::ToggleButton::builder().child(&image).tooltip_text(label).build();
        btn.add_css_class("pc-tool");
        btn.add_css_class("flat");
        btn.set_halign(gtk::Align::Center);
        btn.set_valign(gtk::Align::Center);
        if let Some(ref l) = leader {
            btn.set_group(Some(l));
        } else {
            leader = Some(btn.clone());
        }
        if tool == Tool::Crop {
            crop_btn = Some(btn.clone());
        }
        let state = state.clone();
        let area_clone = area.clone();
        btn.connect_toggled(move |b| {
            image.set_paintable(Some(if b.is_active() { &active_texture } else { &inactive_texture }));
            if b.is_active() {
                state.borrow_mut().tool = tool;
                area_clone.queue_draw();
            }
        });
        toolbar.append(&btn);
    }
    // Ativa Cortar por padrão (em vez do primeiro botão criado) -- reflete
    // o `tool: Tool::Crop` já definido no AppState inicial acima.
    if let Some(btn) = crop_btn {
        btn.set_active(true);
    }

    toolbar.append(&divider());

    // --- Cores fixas (5 círculos da marca, em vez de um seletor livre) ---
    let swatch_btns: Vec<gtk::Button> = PALETTE
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let btn = gtk::Button::builder().build();
            btn.add_css_class("pc-swatch");
        btn.add_css_class("flat");
            btn.add_css_class(&format!("pc-swatch-{i}"));
            // Sem isso, a Box horizontal estica o botão pra cobrir a altura
            // dos vizinhos mais altos (as ferramentas, 36px), deixando o
            // círculo oval em vez de redondo.
            btn.set_halign(gtk::Align::Center);
            btn.set_valign(gtk::Align::Center);
            toolbar.append(&btn);
            btn
        })
        .collect();
    for (i, btn) in swatch_btns.iter().enumerate() {
        let state = state.clone();
        let area_clone = area.clone();
        let swatch_btns = swatch_btns.clone();
        btn.connect_clicked(move |_| {
            state.borrow_mut().color = PALETTE[i];
            for (j, other) in swatch_btns.iter().enumerate() {
                other.set_css_classes(&["pc-swatch", &format!("pc-swatch-{j}")]);
            }
            swatch_btns[i].add_css_class("selected");
            area_clone.queue_draw();
        });
    }
    swatch_btns[0].add_css_class("selected");

    toolbar.append(&divider());

    // --- Espessura do traço (Fina/Média/Grossa -> 2/4/8px) ---
    let width_labels = ["Fina", "Média", "Grossa"];
    let width_dots = [5.0, 8.0, 12.0];
    let width_btns: Vec<gtk::Button> = WIDTHS
        .iter()
        .enumerate()
        .map(|(i, _)| {
            let btn = gtk::Button::builder().tooltip_text(width_labels[i]).build();
            btn.add_css_class("pc-width");
        btn.add_css_class("flat");
            btn.set_halign(gtk::Align::Center);
            btn.set_valign(gtk::Align::Center);
            let dot = gtk::Box::new(gtk::Orientation::Horizontal, 0);
            dot.add_css_class("pc-dot");
            dot.set_size_request(width_dots[i] as i32, width_dots[i] as i32);
            dot.set_halign(gtk::Align::Center);
            dot.set_valign(gtk::Align::Center);
            btn.set_child(Some(&dot));
            toolbar.append(&btn);
            btn
        })
        .collect();
    for (i, btn) in width_btns.iter().enumerate() {
        let state = state.clone();
        let width_btns = width_btns.clone();
        btn.connect_clicked(move |_| {
            state.borrow_mut().stroke_width = WIDTHS[i];
            for other in &width_btns {
                other.remove_css_class("selected");
                if let Some(dot) = other.child() {
                    dot.remove_css_class("selected");
                }
            }
            width_btns[i].add_css_class("selected");
            if let Some(dot) = width_btns[i].child() {
                dot.add_css_class("selected");
            }
        });
    }
    width_btns[1].add_css_class("selected");
    if let Some(dot) = width_btns[1].child() {
        dot.add_css_class("selected");
    }

    toolbar.append(&divider());

    // --- Ações (desfazer, refazer, copiar) ---
    let undo_btn = gtk::Button::builder()
        .child(&gtk::Image::from_paintable(Some(&icons::undo(ICON_ON_DOCK))))
        .tooltip_text("Desfazer")
        .build();
    undo_btn.add_css_class("pc-action");
        undo_btn.add_css_class("flat");
    undo_btn.set_halign(gtk::Align::Center);
    undo_btn.set_valign(gtk::Align::Center);
    {
        let state = state.clone();
        let area_clone = area.clone();
        undo_btn.connect_clicked(move |_| {
            render::undo(&mut state.borrow_mut());
            area_clone.queue_draw();
        });
    }
    toolbar.append(&undo_btn);

    let copy_btn = gtk::Button::builder()
        .child(&gtk::Image::from_paintable(Some(&icons::copy(ICON_ON_DOCK))))
        .tooltip_text("Copiar")
        .build();
    copy_btn.add_css_class("pc-action");
        copy_btn.add_css_class("flat");
    copy_btn.set_halign(gtk::Align::Center);
    copy_btn.set_valign(gtk::Align::Center);
    {
        let state = state.clone();
        let runtime = runtime.clone();
        copy_btn.connect_clicked(move |btn| {
            let result = render::compose_final(&state.borrow()).and_then(|surface| {
                let bytes = render::encode_png_bytes(&surface)?;
                let texture = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes))
                    .map_err(|e| anyhow::anyhow!("falha ao criar textura: {e}"))?;
                btn.clipboard().set_texture(&texture);
                Ok(())
            });
            match result {
                Ok(()) => notify(&runtime, "editor-copy", "Copiado", "A captura foi copiada para a área de transferência.".to_string()),
                Err(e) => {
                    eprintln!("Erro ao copiar: {e}");
                    notify(&runtime, "editor-copy", "Falha ao copiar", e.to_string());
                }
            }
        });
    }
    toolbar.append(&copy_btn);

    toolbar.append(&divider());

    // --- Confirmação (cancelar, salvar) ---
    let cancel_btn = gtk::Button::builder()
        .child(&gtk::Image::from_paintable(Some(&icons::cancel(ICON_CANCEL))))
        .tooltip_text("Cancelar (Esc)")
        .build();
    // `.destructive-action` é o vermelho nativo do GNOME (mesmo usado em
    // diálogos de confirmação do sistema) -- sem `.flat`, vem preenchido.
    cancel_btn.add_css_class("pc-cancel");
    cancel_btn.add_css_class("destructive-action");
    cancel_btn.set_halign(gtk::Align::Center);
    cancel_btn.set_valign(gtk::Align::Center);
    {
        let window = window.clone();
        cancel_btn.connect_clicked(move |_| window.close());
    }
    toolbar.append(&cancel_btn);

    let save_btn = gtk::Button::builder()
        .child(&gtk::Image::from_paintable(Some(&icons::save(ICON_ACTIVE))))
        .tooltip_text("Salvar na pasta (Ctrl+S)")
        .build();
    // `.suggested-action` usa a cor de destaque configurada pelo próprio
    // usuário no GNOME (Configurações > Cores), em vez de uma cor de marca
    // fixa -- sem `.flat`, vem preenchido.
    save_btn.add_css_class("pc-save");
    save_btn.add_css_class("suggested-action");
    save_btn.set_halign(gtk::Align::Center);
    save_btn.set_valign(gtk::Align::Center);
    {
        let state = state.clone();
        let window = window.clone();
        let runtime = runtime.clone();
        save_btn.connect_clicked(move |btn| {
            // Salva num arquivo novo na pasta de destino configurada (não
            // sobrescreve o arquivo temporário da captura crua) -- só o
            // resultado final (já cortado/anotado) deve acabar lá. Se
            // "copiar sempre" estiver ligado, também copia pro clipboard,
            // igual o botão Copiar já faz.
            let cfg = crate::config::load();
            let result = crate::capture::dest_path(&cfg).and_then(|dest| {
                let surface = render::compose_final(&state.borrow())?;
                render::save_surface_as(&surface, &dest, cfg.format)?;
                if cfg.auto_copy {
                    let bytes = render::encode_png_bytes(&surface)?;
                    let texture = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes))
                        .map_err(|e| anyhow::anyhow!("falha ao criar textura: {e}"))?;
                    btn.clipboard().set_texture(&texture);
                }
                let thumbnail = thumbnail_png_bytes(&surface);
                Ok((dest, thumbnail))
            });
            match result {
                Ok((dest, thumbnail)) => {
                    let title = if cfg.auto_copy { "Salva e copiada" } else { "Captura salva" };
                    let body = dest.display().to_string();
                    let folder = dest.parent().map(|p| p.to_path_buf()).unwrap_or(dest);
                    notify_saved(&runtime, title, body, thumbnail, folder);
                    window.close();
                }
                Err(e) => {
                    eprintln!("Erro ao salvar: {e}");
                    notify(&runtime, "editor-save", "Falha ao salvar", e.to_string());
                }
            }
        });
    }
    toolbar.append(&save_btn);

    // Mede o tamanho natural da barra (todos os botões já foram anexados
    // acima) pra dar o mesmo tamanho ao fundo borrado por trás dela -- as
    // duas camadas ficam empilhadas na mesma posição/tamanho na overlay
    // externa (fundo primeiro = atrás, barra depois = na frente).
    // A barra ainda não está anexada a nenhuma janela nesse ponto, então a
    // folha de estilo pode não ter sido totalmente resolvida ainda pra
    // medir com 100% de precisão -- soma uma margem de segurança na largura
    // (a barra centraliza horizontalmente, então sobra igual dos dois
    // lados -- não desalinha nada). Na altura NÃO dá pra fazer o mesmo: as
    // duas camadas começam no mesmo y (mesma margem/alinhamento), então
    // qualquer folga extra ali só cresce por baixo, empurrando os botões
    // pra cima dentro do "vidro" -- por isso a altura usa o valor exato.
    // `measure()` inclui a margem de posicionamento no tamanho retornado
    // (documentado pelo próprio GTK: a margem "é somada em cima" do
    // tamanho do widget) -- desconta aqui, senão o fundo fica bem mais
    // alto que a barra de botões de verdade.
    let (_, toolbar_w, _, _) = toolbar.measure(gtk::Orientation::Horizontal, -1);
    let (_, toolbar_h, _, _) = toolbar.measure(gtk::Orientation::Vertical, -1);
    let toolbar_h = toolbar_h - TOOLBAR_TOP_OFFSET;

    let toolbar_backdrop = gtk::DrawingArea::new();
    toolbar_backdrop.set_content_width(toolbar_w + 8);
    toolbar_backdrop.set_content_height(toolbar_h);
    toolbar_backdrop.set_halign(gtk::Align::Center);
    toolbar_backdrop.set_valign(gtk::Align::Start);
    toolbar_backdrop.set_margin_top(TOOLBAR_TOP_OFFSET);
    {
        let state = state.clone();
        let scale_factor = scale_factor.clone();
        let area = area.clone();
        toolbar_backdrop.set_draw_func(move |_widget, cr, w, h| {
            draw_toolbar_backdrop(cr, w, h, &state, &scale_factor, &area);
        });
    }
    overlay.add_overlay(&toolbar_backdrop);
    overlay.add_overlay(&toolbar);

    // --- Desenho ---
    {
        let state = state.clone();
        let scale_factor = scale_factor.clone();
        area.set_draw_func(move |_area, cr, _w, _h| {
            let state = state.borrow();
            let scale = scale_factor();

            let _ = cr.save();
            cr.scale(scale, scale);

            let _ = cr.set_source_surface(&state.image, 0.0, 0.0);
            let _ = cr.paint();

            render::draw_all_annotations(cr, &state);

            if let (Some(start), Some(current)) = (state.drag_start, state.drag_current) {
                match state.tool {
                    Tool::Arrow | Tool::Rect | Tool::Ellipse => {
                        let preview = render::make_annotation(state.tool, start, current, state.color, state.stroke_width);
                        if let Some(ann) = preview {
                            let _ = render::draw_annotation(cr, &ann);
                        }
                    }
                    Tool::Blur => {
                        let _ = render::draw_blur_region(cr, &state.image, &CropRect { p0: start, p1: current }, render::BLUR_RADIUS);
                    }
                    Tool::Pen => {
                        let preview = Annotation::Pen {
                            points: state.drag_points.clone(),
                            color: state.color,
                            width: state.stroke_width,
                        };
                        let _ = render::draw_annotation(cr, &preview);
                    }
                    _ => {}
                }
            }

            // Enquanto uma nova seleção de corte está sendo arrastada, ela
            // tem prioridade sobre o corte já confirmado -- sem isso, refazer
            // o corte (depois de já ter um) não mostrava o pontilhado se
            // mexendo durante o arrasto, só depois de soltar o botão (o
            // corte antigo ficava sempre por cima).
            let (img_w, img_h) = (state.image.width() as f64, state.image.height() as f64);
            let confirmed = render::confirmed_crop(&state);
            let live_crop = if state.tool == Tool::Crop {
                if let (Some(start), Some(current)) = (state.drag_start, state.drag_current) {
                    Some(CropRect { p0: start, p1: current })
                } else {
                    confirmed
                }
            } else {
                confirmed
            };
            if let Some(r) = &live_crop {
                render::draw_crop_overlay(cr, r, img_w, img_h);
                render::draw_crop_label(cr, r, img_w, img_h);
            }

            let _ = cr.restore();

            // A lupa fica em espaço de widget (tamanho fixo na tela, não
            // escala com a imagem) -- por isso vem depois do `cr.restore()`
            // que desfaz o `cr.scale(scale, scale)` da imagem, ao contrário
            // da régua/overlay do corte acima.
            if state.tool == Tool::Crop {
                if let Some(image_cursor) = state.cursor {
                    let widget_cursor = (image_cursor.0 * scale, image_cursor.1 * scale);
                    let dragging_size = state.drag_start.zip(state.drag_current).map(|(s, c)| ((c.0 - s.0).abs(), (c.1 - s.1).abs()));
                    draw_magnifier(cr, widget_cursor, image_cursor, &state.image, dragging_size);
                }
            }
        });
    }

    // --- Gesto de arrastar (linha, seta, retângulo, elipse, corte) ---
    // Coordenadas do GTK chegam em espaço do widget (pixels lógicos da
    // tela); dividimos pelo fator de escala pra guardar tudo em espaço da
    // imagem (resolução física), que é o que `render::` e o arquivo final
    // esperam -- ver comentário em `scale_factor` acima.
    let drag = gtk::GestureDrag::new();
    {
        let state = state.clone();
        let scale_factor = scale_factor.clone();
        drag.connect_drag_begin(move |_, x, y| {
            let scale = scale_factor();
            let mut state = state.borrow_mut();
            if state.tool == Tool::Step {
                return;
            }
            let pos = (x / scale, y / scale);
            state.drag_start = Some(pos);
            state.drag_current = Some(pos);
            if state.tool == Tool::Pen {
                state.drag_points = vec![pos];
            }
        });
    }
    {
        let state = state.clone();
        let area_clone = area.clone();
        let scale_factor = scale_factor.clone();
        drag.connect_drag_update(move |_, dx, dy| {
            let scale = scale_factor();
            let mut state = state.borrow_mut();
            if let Some(start) = state.drag_start {
                let pos = (start.0 + dx / scale, start.1 + dy / scale);
                state.drag_current = Some(pos);
                if state.tool == Tool::Pen {
                    state.drag_points.push(pos);
                }
                drop(state);
                area_clone.queue_draw();
            }
        });
    }
    {
        let state = state.clone();
        let area_clone = area.clone();
        let scale_factor = scale_factor.clone();
        let save_btn = save_btn.clone();
        drag.connect_drag_end(move |_, dx, dy| {
            let scale = scale_factor();
            let mut state = state.borrow_mut();
            // Marca se a soltura foi uma seleção de corte de verdade --
            // decide se salva automaticamente, mas só depois de soltar o
            // empréstimo do RefCell abaixo (o clique do Salvar também
            // precisa dele pra compor a imagem final).
            let mut just_cropped = false;
            if let Some(start) = state.drag_start {
                let end = (start.0 + dx / scale, start.1 + dy / scale);
                let moved = (end.0 - start.0).abs() > MIN_DRAG || (end.1 - start.1).abs() > MIN_DRAG;
                if moved {
                    match state.tool {
                        Tool::Crop => {
                            render::push_annotation(&mut state, Annotation::Crop(CropRect { p0: start, p1: end }));
                            just_cropped = true;
                        }
                        Tool::Blur => render::push_annotation(&mut state, Annotation::Blur(CropRect { p0: start, p1: end })),
                        Tool::Arrow | Tool::Rect | Tool::Ellipse => {
                            if let Some(ann) = render::make_annotation(state.tool, start, end, state.color, state.stroke_width) {
                                render::push_annotation(&mut state, ann);
                            }
                        }
                        Tool::Pen => {
                            let points = std::mem::take(&mut state.drag_points);
                            let (color, width) = (state.color, state.stroke_width);
                            render::push_annotation(&mut state, Annotation::Pen { points, color, width });
                        }
                        Tool::Step => {}
                    }
                }
            }
            state.drag_start = None;
            state.drag_current = None;
            state.drag_points.clear();
            drop(state);
            area_clone.queue_draw();
            // Escolher uma área de corte já é a intenção de terminar a
            // edição -- salva na hora, sem precisar clicar em Salvar depois.
            if just_cropped {
                save_btn.emit_clicked();
            }
        });
    }
    area.add_controller(drag);

    // --- Movimento do mouse (posição do cursor, pra lupa) ---
    let motion = gtk::EventControllerMotion::new();
    {
        let state = state.clone();
        let area_clone = area.clone();
        let scale_factor = scale_factor.clone();
        motion.connect_motion(move |_, x, y| {
            let scale = scale_factor();
            state.borrow_mut().cursor = Some((x / scale, y / scale));
            area_clone.queue_draw();
        });
    }
    {
        let state = state.clone();
        let area_clone = area.clone();
        motion.connect_leave(move |_| {
            state.borrow_mut().cursor = None;
            area_clone.queue_draw();
        });
    }
    area.add_controller(motion);

    // --- Clique (texto, numeração) ---
    let click = gtk::GestureClick::new();
    {
        let state = state.clone();
        let area_clone = area.clone();
        let scale_factor = scale_factor.clone();
        click.connect_released(move |_, n_press, x, y| {
            if n_press != 1 {
                return;
            }
            let scale = scale_factor();
            let tool = state.borrow().tool;
            if tool == Tool::Step {
                let mut state = state.borrow_mut();
                let color = state.color;
                render::push_step(&mut state, (x / scale, y / scale), color);
                drop(state);
                area_clone.queue_draw();
            }
        });
    }
    area.add_controller(click);

    // --- Atalhos de teclado ---
    // Fase de captura (em vez do padrão, borbulhar): sem isso, um botão da
    // barra com foco (ex: a ferramenta Cortar, ativa por padrão) responde
    // sozinho ao Enter como "ativar este botão" antes do evento chegar até
    // aqui -- capturar garante que a gente decide primeiro.
    let key_controller = gtk::EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    {
        let state = state.clone();
        let area_clone = area.clone();
        let window = window.clone();
        let save_btn = save_btn.clone();
        let copy_btn = copy_btn.clone();
        key_controller.connect_key_pressed(move |_, key, _, modifiers| {
            if key == gdk::Key::Escape {
                window.close();
                return glib::Propagation::Stop;
            }
            if key == gdk::Key::Return || key == gdk::Key::KP_Enter {
                save_btn.emit_clicked();
                return glib::Propagation::Stop;
            }
            let ctrl = modifiers.contains(gdk::ModifierType::CONTROL_MASK);
            let shift = modifiers.contains(gdk::ModifierType::SHIFT_MASK);
            let is_z = key == gdk::Key::z || key == gdk::Key::Z;
            if ctrl {
                // Ctrl+Shift+Z refaz (padrão GTK/GNOME no Linux); testa antes
                // do Ctrl+Z simples, senão essa combinação também cairia ali.
                if shift && is_z {
                    render::redo(&mut state.borrow_mut());
                    area_clone.queue_draw();
                    return glib::Propagation::Stop;
                }
                if is_z {
                    render::undo(&mut state.borrow_mut());
                    area_clone.queue_draw();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::s {
                    save_btn.emit_clicked();
                    return glib::Propagation::Stop;
                }
                if key == gdk::Key::c {
                    copy_btn.emit_clicked();
                    return glib::Propagation::Stop;
                }
            }
            glib::Propagation::Proceed
        });
    }
    window.add_controller(key_controller);

    // Independente de como a janela fecha (Cancelar, Salvar, Esc, ou o X da
    // janela), o arquivo temporário da captura crua não serve mais pra nada
    // -- ver `capture::temp_capture_path`. Ignora erro se já tiver sido
    // removido (ex: chamado duas vezes).
    window.connect_close_request(move |_| {
        let _ = std::fs::remove_file(&temp_path);
        glib::Propagation::Proceed
    });

    // Registra esta janela como "a" janela do editor atual, e limpa o slot
    // quando ela for destruída -- é o que permite `open_editor_window`
    // fechar automaticamente uma janela de edição anterior ainda aberta.
    {
        let window_slot_for_destroy = window_slot.clone();
        window.connect_destroy(move |_| {
            window_slot_for_destroy.borrow_mut().take();
        });
        *window_slot.borrow_mut() = Some(window.clone());
    }

    window.fullscreen();
    window.present();
    Ok(())
}

/// Caminho de um retângulo com cantos arredondados -- cairo não tem essa
/// primitiva pronta, só arcos/linhas.
fn rounded_rect_path(cr: &cairo::Context, x: f64, y: f64, w: f64, h: f64, r: f64) {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    cr.new_sub_path();
    cr.arc(x + w - r, y + r, r, -std::f64::consts::FRAC_PI_2, 0.0);
    cr.arc(x + w - r, y + h - r, r, 0.0, std::f64::consts::FRAC_PI_2);
    cr.arc(x + r, y + h - r, r, std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    cr.arc(x + r, y + r, r, std::f64::consts::PI, std::f64::consts::PI * 1.5);
    cr.close_path();
}

/// Desenha o "vidro fosco" atrás da barra flutuante: recorta da própria
/// captura congelada a região que fica por trás da barra (mesma posição e
/// tamanho dela), borra essa região (`render::box_blur`) e pinta por cima
/// a tinta lilás translúcida da marca. Existe porque o Wayland não tem
/// `backdrop-filter` de verdade pra janelas comuns -- só dá pra fingir
/// borrando o conteúdo que a gente já sabe que está atrás (a imagem
/// congelada), não o desktop ao vivo.
fn draw_toolbar_backdrop(
    cr: &cairo::Context,
    w: i32,
    h: i32,
    state: &Rc<RefCell<AppState>>,
    scale_factor: &impl Fn() -> f64,
    area: &gtk::DrawingArea,
) {
    if w <= 0 || h <= 0 {
        return;
    }
    let scale = scale_factor();
    let win_w = area.width() as f64;
    let x_widget = ((win_w - w as f64) / 2.0).max(0.0);
    let y_widget = 18.0;
    let (sx0, sy0) = (x_widget / scale, y_widget / scale);

    let Ok(mut cropped) = cairo::ImageSurface::create(cairo::Format::ARgb32, w, h) else {
        return;
    };
    {
        let state_ref = state.borrow();
        let Ok(crop_cr) = cairo::Context::new(&cropped) else { return };
        crop_cr.scale(scale, scale);
        crop_cr.translate(-sx0, -sy0);
        let _ = crop_cr.set_source_surface(&state_ref.image, 0.0, 0.0);
        let _ = crop_cr.paint();
    }

    let blurred = render::box_blur(&mut cropped, 6).unwrap_or(cropped);

    let _ = cr.save();
    rounded_rect_path(cr, 0.0, 0.0, w as f64, h as f64, 14.0);
    cr.clip();
    let _ = cr.set_source_surface(&blurred, 0.0, 0.0);
    let _ = cr.paint();
    // Tint neutro (cinza-escuro translúcido), mesmo espírito da classe
    // `.osd` do próprio GTK/libadwaita usada em barras flutuantes sobre
    // imagem/vídeo -- sem cor de marca.
    cr.set_source_rgba(0.0, 0.0, 0.0, 0.55);
    let _ = cr.paint();
    let _ = cr.restore();

    // Borda desenhada aqui (não via CSS na barra) de propósito: garante que
    // ela acompanha exatamente o mesmo raio/tamanho do fundo, sem risco de
    // as duas camadas ficarem alguns pixels desalinhadas.
    let _ = cr.save();
    rounded_rect_path(cr, 0.75, 0.75, w as f64 - 1.5, h as f64 - 1.5, 13.5);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.14);
    cr.set_line_width(1.5);
    let _ = cr.stroke();
    let _ = cr.restore();
}

/// Lente circular de aumento, seguindo o cursor -- só aparece com a
/// ferramenta de Cortar ativa. `widget_cursor`/`image_cursor` são a mesma
/// posição em dois espaços diferentes: a lente em si é desenhada em
/// tamanho fixo na tela (não escala com a imagem), mas o conteúdo dentro
/// dela vem de pixels reais da captura. `dragging_size`, quando presente
/// (largura, altura em espaço de imagem), troca a etiqueta de "x, y" pra
/// "largura × altura" -- reflete o que está sendo arrastado no momento.
fn draw_magnifier(cr: &cairo::Context, widget_cursor: Point, image_cursor: Point, image: &cairo::ImageSurface, dragging_size: Option<Point>) {
    const LENS: f64 = 96.0;
    const ZOOM: f64 = 8.0;
    let sample = LENS / ZOOM;
    let cx = widget_cursor.0 + 22.0 + LENS / 2.0;
    let cy = widget_cursor.1 + 22.0 + LENS / 2.0;

    let _ = cr.save();
    cr.arc(cx, cy, LENS / 2.0, 0.0, std::f64::consts::TAU);
    cr.clip_preserve();

    cr.set_source_rgb(0.18, 0.11, 0.17);
    let _ = cr.fill_preserve();

    // Pixels ampliados da captura ao redor do cursor.
    let _ = cr.save();
    cr.translate(cx - LENS / 2.0, cy - LENS / 2.0);
    cr.scale(ZOOM, ZOOM);
    cr.translate(-(image_cursor.0 - sample / 2.0), -(image_cursor.1 - sample / 2.0));
    let _ = cr.set_source_surface(image, 0.0, 0.0);
    let _ = cr.paint();
    let _ = cr.restore();

    // Grade acompanhando o zoom -- cada célula é um pixel real da imagem.
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.14);
    cr.set_line_width(1.0);
    let offset = (image_cursor.0 - sample / 2.0).fract() * ZOOM;
    let mut gx = cx - LENS / 2.0 - offset;
    while gx <= cx + LENS / 2.0 {
        cr.move_to(gx, cy - LENS / 2.0);
        cr.line_to(gx, cy + LENS / 2.0);
        let _ = cr.stroke();
        gx += ZOOM;
    }
    let offset_y = (image_cursor.1 - sample / 2.0).fract() * ZOOM;
    let mut gy = cy - LENS / 2.0 - offset_y;
    while gy <= cy + LENS / 2.0 {
        cr.move_to(cx - LENS / 2.0, gy);
        cr.line_to(cx + LENS / 2.0, gy);
        let _ = cr.stroke();
        gy += ZOOM;
    }

    // Mira horizontal/vertical e quadrado marcando o pixel exato no centro.
    cr.set_source_rgba(252.0 / 255.0, 208.0 / 255.0, 234.0 / 255.0, 0.85);
    cr.set_line_width(1.0);
    cr.move_to(cx - LENS / 2.0, cy);
    cr.line_to(cx + LENS / 2.0, cy);
    let _ = cr.stroke();
    cr.move_to(cx, cy - LENS / 2.0);
    cr.line_to(cx, cy + LENS / 2.0);
    let _ = cr.stroke();

    cr.set_source_rgba(45.0 / 255.0, 20.0 / 255.0, 42.0 / 255.0, 0.5);
    cr.set_line_width(3.0);
    cr.rectangle(cx - ZOOM / 2.0, cy - ZOOM / 2.0, ZOOM, ZOOM);
    let _ = cr.stroke();
    cr.set_source_rgba(252.0 / 255.0, 208.0 / 255.0, 234.0 / 255.0, 1.0);
    cr.set_line_width(1.5);
    cr.rectangle(cx - ZOOM / 2.0, cy - ZOOM / 2.0, ZOOM, ZOOM);
    let _ = cr.stroke();

    let _ = cr.restore();

    // Borda do círculo.
    cr.arc(cx, cy, LENS / 2.0, 0.0, std::f64::consts::TAU);
    cr.set_source_rgba(1.0, 1.0, 1.0, 0.9);
    cr.set_line_width(2.0);
    let _ = cr.stroke();

    // Etiqueta abaixo: coordenadas paradas, tamanho durante o arrasto.
    let label = match dragging_size {
        Some((w, h)) => format!("{} × {}", w.round() as i64, h.round() as i64),
        None => format!("{}, {}", image_cursor.0.round() as i64, image_cursor.1.round() as i64),
    };
    cr.select_font_face("monospace", cairo::FontSlant::Normal, cairo::FontWeight::Normal);
    cr.set_font_size(10.5);
    if let Ok(extents) = cr.text_extents(&label) {
        let pad_x = 7.0;
        let pad_y = 4.0;
        let box_w = extents.width() + pad_x * 2.0;
        let box_h = extents.height() + pad_y * 2.0;
        let box_x = cx - box_w / 2.0;
        let box_y = cy + LENS / 2.0 + 5.0;
        cr.set_source_rgba(14.0 / 255.0, 8.0 / 255.0, 14.0 / 255.0, 0.82);
        cr.rectangle(box_x, box_y, box_w, box_h);
        let _ = cr.fill();
        cr.set_source_rgb(1.0, 1.0, 1.0);
        cr.move_to(box_x + pad_x - extents.x_bearing(), box_y + pad_y - extents.y_bearing());
        let _ = cr.show_text(&label);
    }
}

/// Manda uma notificação do sistema em segundo plano (dispara e esquece),
/// sem bloquear o thread do GTK.
fn notify(runtime: &tokio::runtime::Handle, id: &'static str, title: &'static str, body: String) {
    runtime.spawn(async move {
        let _ = crate::notify::send(id, title, &body).await;
    });
}

/// Reduz a superfície final pra uma miniatura pequena (bytes PNG), pra
/// usar como ícone da notificação de "Captura salva" -- sem isso, mandar a
/// imagem inteira (podendo ser vários megapixels) pro daemon de
/// notificação seria bem mais pesado do que precisa.
fn thumbnail_png_bytes(surface: &cairo::ImageSurface) -> Option<Vec<u8>> {
    let full = render::encode_png_bytes(surface).ok()?;
    let thumb = image::load_from_memory(&full).ok()?.thumbnail(96, 96);
    let mut buf = Vec::new();
    thumb.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).ok()?;
    Some(buf)
}

/// Como `notify`, mas manda a notificação de "Captura salva" (com
/// miniatura e o botão "Abrir pasta") em vez da simples -- cada chamada
/// usa um `id` próprio (baseado no horário) pra `notify::send_saved` saber
/// distinguir a ação do botão desta notificação das de outras.
fn notify_saved(runtime: &tokio::runtime::Handle, title: &'static str, body: String, thumbnail: Option<Vec<u8>>, folder: std::path::PathBuf) {
    let id = format!(
        "editor-save-{}",
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
    );
    runtime.spawn(async move {
        let _ = crate::notify::send_saved(&id, title, &body, thumbnail, folder).await;
    });
}

