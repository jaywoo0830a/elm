// v0.3 — egui 어댑터 (사양서 7.2)
// 같은 컴포넌트, 다른 플랫폼. egui를 헤드리스로 구동해 실제 위젯·클릭을 검증한다.

use elm_magic::Component;
use elm_magic_egui::Pass;

elm_magic::view! {
    fn Counter(n = 0) {
        <Col>
            "Count: {n}"
            <Button on_click={n += 1}>"+"</Button>
            <Button on_click={n -= 1}>"-"</Button>
        </Col>
    }
}

fn input() -> egui::RawInput {
    let mut i = egui::RawInput::default();
    i.screen_rect = Some(egui::Rect::from_min_size(
        egui::Pos2::ZERO,
        egui::vec2(800.0, 600.0),
    ));
    i
}

/// 모든 페인트 명령에서 텍스트 수집
fn painted_text(out: &egui::FullOutput) -> String {
    let mut s = String::new();
    for clipped in &out.shapes {
        if let egui::Shape::Text(t) = &clipped.shape {
            s.push_str(t.galley.text());
            s.push('\n');
        }
    }
    s
}

/// 한 프레임: 트리를 egui로 그리고 버튼 응답 수집
fn frame<C: Component>(
    ctx: &egui::Context,
    app: &mut elm_magic::testing::TestApp<C>,
    input: egui::RawInput,
) -> (egui::FullOutput, Pass) {
    let tree = app.element().clone();
    let mut pass = None;
    let out = ctx.run_ui(input, |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            pass = Some(elm_magic_egui::render(ui, &tree, &mut app.ctx.arena));
        });
    });
    // headless: 페인터가 없으므로 텍스처 델타를 소비한다 (폰트 아틀라스)
    let mut out = out;
    out.textures_delta.clear();
    (out, pass.expect("adapter did not run"))
}

fn click_events(pos: egui::Pos2) -> egui::RawInput {
    let mut i = input();
    i.events = vec![
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default(),
        },
    ];
    i
}

#[test]
fn egui_renders_counter_text() {
    let mut app = elm_magic::mount::<Counter>();
    let ctx = egui::Context::default();
    let (out, _) = frame(&ctx, &mut app, input());
    let text = painted_text(&out);
    assert!(text.contains("Count: 0"), "painted: {text}");
}

#[test]
fn egui_clicks_dispatch_to_arena() {
    let mut app = elm_magic::mount::<Counter>();
    let ctx = egui::Context::default();
    let (_, pass) = frame(&ctx, &mut app, input());
    let (_, rect) = pass
        .buttons
        .iter()
        .find(|(label, _)| label == "+")
        .expect("'+' button rendered");

    // 실제 egui 입력 이벤트로 클릭 시뮬레이션
    let _ = frame(&ctx, &mut app, click_events(rect.rect.center()));
    // 어댑터가 클릭을 핸들러로 넘겼으므로 앱 재렌더 후 텍스트가 바뀐다
    app.refresh();
    let (out, _) = frame(&ctx, &mut app, input());
    let text = painted_text(&out);
    assert!(text.contains("Count: 1"), "painted: {text}");
}

// ── <Raw> 탈출구 × egui (사양서 7.3 × 7.2) ───────────────────
elm_magic::view! {
    fn Mixed() {
        <Col>
            <Raw>|ui: &mut egui::Ui| {
                ui.hyperlink_to("docs", "https://example.com");
            }</Raw>
            "plain text"
        </Col>
    }
}

#[test]
fn raw_escape_hatch_receives_egui_ui() {
    let mut app = elm_magic::mount::<Mixed>();
    let ctx = egui::Context::default();
    // <Raw>의 클로저가 실제 &mut egui::Ui로 호출되면 통과
    let (_, pass) = frame(&ctx, &mut app, input());
    assert!(
        pass.buttons.is_empty(),
        "raw widget must not be treated as a button"
    );
}
// ── v0.6 css! → egui 스타일 적용 (사양서 6.1) ────────────────

use elm_magic::style::{Color, Edges, Palette, Token};

elm_magic::css! {
    .egui_card { gap: 8; padding: 16; bg: surface; radius: 8; }
    .egui_loud { color: error; font-size: 20; weight: bold; }
}

elm_magic::view! {
    fn Styled() {
        <Col class="egui_card">
            <Text class="egui_loud">"styled"</Text>
            <Button>"go"</Button>
        </Col>
    }
}

#[test]
fn egui_applies_container_and_text_style() {
    let mut app = elm_magic::mount::<Styled>();
    let ctx = egui::Context::default();
    let (_, pass) = frame(&ctx, &mut app, input());

    let col = pass.style_of("col").expect("Col 스타일이 적용됐다");
    assert_eq!(col.gap, Some(8.0));
    assert_eq!(col.padding, Some(Edges::splat(16.0)));
    assert_eq!(col.radius, Some(8.0));
    assert_eq!(col.bg, Some(Palette::dark().get(Token::Surface)));

    let text = pass.style_of("text").expect("Text 스타일이 적용됐다");
    assert_eq!(text.color, Some(Palette::dark().get(Token::Error)));
    assert_eq!(text.font_size, Some(20.0));
    assert_eq!(text.bold, Some(true));
}

#[test]
fn egui_unstyled_tree_records_no_styles() {
    let mut app = elm_magic::mount::<Counter>();
    let ctx = egui::Context::default();
    let (_, pass) = frame(&ctx, &mut app, input());
    assert!(pass.styles.is_empty(), "class가 없으면 스타일 기록도 없다");
}

#[test]
fn egui_theme_palette_overrides_default() {
    // 사양서 6.3 — 테마(팔레트)를 갈아끼우면 색이 따라온다
    let mut app = elm_magic::mount::<Styled>();
    let ctx = egui::Context::default();
    let theme = Palette::dark().with(Token::Surface, Color::rgb(1, 2, 3));
    let tree = app.element().clone();
    let mut pass = None;
    let mut out = ctx.run_ui(input(), |ctx| {
        egui::CentralPanel::default().show(ctx, |ui| {
            pass = Some(elm_magic_egui::render_with_palette(
                ui,
                &tree,
                &mut app.ctx.arena,
                &theme,
            ));
        });
    });
    let pass = pass.expect("adapter did not run");
    let col = pass.style_of("col").expect("col");
    assert_eq!(col.bg, Some(Color::rgb(1, 2, 3)), "지정한 팔레트가 쓰인다");
    // `FullOutput`은 텍스처 델타를 비우고 버려야 한다
    out.textures_delta.clear();
}

// ── v0.6+ 스타일로 렌더 자체를 끄기 (`display`) ───────────────

elm_magic::css! {
    ".egui_hide" { display: none; }
    ".egui_show" { display: flex; }
}

elm_magic::view! {
    fn Toggly() {
        <Col>
            <Button class="egui_hide">"never"</Button>
            <Button class="egui_show">"always"</Button>
        </Col>
    }
}

#[test]
fn egui_display_none_skips_rendering() {
    let mut app = elm_magic::mount::<Toggly>();
    let ctx = egui::Context::default();
    let (_, pass) = frame(&ctx, &mut app, input());
    assert_eq!(
        pass.buttons.len(),
        1,
        "display:none은 자리도 차지하지 않는다"
    );
    assert_eq!(pass.buttons[0].0, "always");
}
