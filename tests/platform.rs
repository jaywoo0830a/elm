// v0.3 — 플랫폼 추상화 (사양서 7.1)
// `elm_magic::run(Platform, Component)` — 컴포넌트는 플랫폼을 모른다.

elm_magic::view! {
    fn Counter(n = 0) {
        <Col>
            "Count: {n}"
            <Button on_click={n += 1}>"+"</Button>
        </Col>
    }
}

#[test]
fn headless_platform_runs_and_returns_session() {
    let mut app = elm_magic::run(elm_magic::Headless, Counter);
    app.click("+");
    app.expect_text("Count: 1");
}

#[test]
fn prelude_exposes_platform() {
    // 사용자 진입점 스타일: use elm_magic::prelude::*;
    let mut app = { use elm_magic::prelude::*; run(Headless, Counter) };
    app.click("+");
    app.click("+");
    app.expect_text("Count: 2");
}
