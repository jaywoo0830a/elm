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
    egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(800.0, 600.0),
        )),
        ..Default::default()
    }
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

// ── 0.7.4 — Button/Tab의 CSS padding·height (리포트 버그 12) ──
//
// `design-audit` 실측: Button은 padding을 바꿔도 rect의 w/h가 변하지 않았고,
// Tab은 padding도 height도 무시했다(h 19 고정). 실제 rect로 고정한다.

elm_magic::css! {
    .egui_sized { padding: 12 20; height: 30; }
}

elm_magic::view! {
    fn Sized() {
        <Col>
            <Button class="egui_sized">"pad"</Button>
            <Button>"pad"</Button>
            <Tab class="egui_sized">"tab"</Tab>
            <Tab>"tab"</Tab>
        </Col>
    }
}

#[test]
fn egui_button_and_tab_respect_padding_and_height() {
    let mut app = elm_magic::mount::<Sized>();
    let ctx = egui::Context::default();
    let (_, pass) = frame(&ctx, &mut app, input());

    // 같은 라벨이 둘씩이므로 등장 순서로 (스타일 적용본, 대조군)을 고른다.
    let rect = |label: &str, nth: usize| {
        pass.buttons
            .iter()
            .filter(|(l, _)| l == label)
            .nth(nth)
            .unwrap_or_else(|| panic!("{label} #{nth} not rendered"))
            .1
            .rect
    };
    let padded_btn = rect("pad", 0);
    let plain_btn = rect("pad", 1);
    let padded_tab = rect("tab", 0);
    let plain_tab = rect("tab", 1);

    assert!(
        padded_btn.height() >= 30.0,
        "Button이 height: 30을 따라야 한다: {padded_btn:?}"
    );
    assert!(
        padded_btn.width() > plain_btn.width(),
        "Button의 padding이 rect 폭을 키워야 한다: {} vs {}",
        padded_btn.width(),
        plain_btn.width()
    );
    assert!(
        padded_tab.height() >= 30.0,
        "Tab이 height: 30을 따라야 한다 (예전에는 19 고정): {padded_tab:?}"
    );
    assert!(
        padded_tab.width() > plain_tab.width(),
        "Tab의 padding이 rect 폭을 키워야 한다: {} vs {}",
        padded_tab.width(),
        plain_tab.width()
    );
}

// ── P2 패리티 — 같은 컴포넌트가 두 경로(헤드리스 / egui)에서 같은 계약을 따르는가 ──
//
// 어댑터는 코어 위에 얹히므로, 여기서 확인하는 것은 **어댑터가 코어의 결정과
// 어긋나지 않는지**다. 의도된 발산(disabled 클릭, display:none, Raw)은 명시적으로
// 고정한다 — 조용한 발산이 회귀의 온상이기 때문이다.

use elm_magic::prelude::Role;

elm_magic::view! {
    fn ParityButtons(n = 0) {
        <Col>
            "Count: {n}"
            <Button on_click={n += 1}>"plus"</Button>
            <Button on_click={n -= 1}>"minus"</Button>
            <Row>
                <Tab active={n == 0} on_click={n = 0}>"Home"</Tab>
                <Tab active={n == 1} on_click={n = 1}>"Stats"</Tab>
            </Row>
        </Col>
    }
}

#[test]
fn egui_buttons_cover_every_headless_button() {
    let mut app = elm_magic::mount::<ParityButtons>();
    let ctx = egui::Context::default();
    let (_, pass) = frame(&ctx, &mut app, input());

    let egui_labels: Vec<&str> = pass.buttons.iter().map(|(l, _)| l.as_str()).collect();
    let headless_buttons: Vec<String> = app
        .roles()
        .into_iter()
        .filter(|(role, _)| *role == Role::Button)
        .filter_map(|(_, label)| label)
        .collect();

    assert_eq!(
        headless_buttons.len(),
        2,
        "헤드리스 계약: 버튼 2개 — {headless_buttons:?}"
    );
    for label in &headless_buttons {
        assert!(
            egui_labels.contains(&label.as_str()),
            "헤드리스 버튼 {label:?}가 egui에 그려지지 않았다: {egui_labels:?}"
        );
    }
    // egui가 헤드리스에 없는 위젯을 만들어내지 않는다 (Tab도 버튼 계열로 그려진다)
    for label in &egui_labels {
        assert!(
            headless_buttons.iter().any(|b| b == label) || *label == "Home" || *label == "Stats",
            "egui에만 있는 위젯이 생겼다: {label:?} ({egui_labels:?})"
        );
    }
}

#[test]
fn egui_styles_match_headless_resolution() {
    let mut app = elm_magic::mount::<Styled>();
    let ctx = egui::Context::default();
    let (_, pass) = frame(&ctx, &mut app, input());

    let headless = app.element().resolved_style(&Palette::dark());
    let egui_col = pass.style_of("col").expect("Col 스타일");

    assert_eq!(egui_col.gap, headless.gap, "gap이 코어 해석과 다르다");
    assert_eq!(egui_col.padding, headless.padding, "padding이 다르다");
    assert_eq!(egui_col.bg, headless.bg, "bg가 다르다");
    assert_eq!(egui_col.radius, headless.radius, "radius가 다르다");

    let headless_text = app
        .element()
        .children()
        .and_then(|children| children.iter().find(|c| c.tag() == "text"))
        .expect("Text 자식")
        .resolved_style(&Palette::dark());
    let egui_text = pass.style_of("text").expect("Text 스타일");
    assert_eq!(egui_text.color, headless_text.color, "color가 다르다");
    assert_eq!(egui_text.font_size, headless_text.font_size);
    assert_eq!(egui_text.bold, headless_text.bold);
}

#[test]
fn display_none_is_painted_nowhere_but_present_in_the_headless_tree() {
    let mut app = elm_magic::mount::<Toggly>();
    let ctx = egui::Context::default();
    let (_, pass) = frame(&ctx, &mut app, input());

    // egui: display:none은 자리도 차지하지 않는다
    let egui_labels: Vec<&str> = pass.buttons.iter().map(|(l, _)| l.as_str()).collect();
    assert_eq!(egui_labels, ["always"], "숨긴 버튼은 그려지지 않는다");

    // 헤드리스: 트리와 접근성에는 그대로 남는다 (의도된 발산)
    let tree = app.render_tree();
    assert!(
        tree.contains("\"never\""),
        "헤드리스 트리에는 남는다:\n{tree}"
    );
    app.assert_text("never");
}

elm_magic::view! {
    fn ParityDisabled(n = 0) {
        <Col>
            "Count: {n}"
            <Button disabled={true} on_click={n += 1}>"nope"</Button>
        </Col>
    }
}

#[test]
fn disabled_buttons_do_not_dispatch_in_egui_but_do_headless() {
    let mut app = elm_magic::mount::<ParityDisabled>();
    let ctx = egui::Context::default();
    let (_, pass) = frame(&ctx, &mut app, input());

    if let Some((_, response)) = pass.buttons.iter().find(|(label, _)| label == "nope") {
        let _ = frame(&ctx, &mut app, click_events(response.rect.center()));
        app.refresh();
    }
    let (out, _) = frame(&ctx, &mut app, input());
    assert!(
        painted_text(&out).contains("Count: 0"),
        "egui는 disabled 버튼 클릭을 무시한다 (의도된 발산)"
    );

    // 헤드리스는 현재 의미론상 핸들러를 그대로 부른다 (tests/callbacks.rs)
    app.click("nope");
    assert!(
        app.render_tree().contains("Count: 1"),
        "헤드리스 클릭은 디스패치한다:\n{}",
        app.render_tree()
    );
}
