// v0.4 — 테스트 슈가 (사양서 8.2)
//
// `.mock(fn, impl)` 체이닝, `assert_text` / `assert_visible` / `assert_hidden`,
// `type_into(selector, value)`, `toggle(label)`.

async fn search_api(_q: String) -> Vec<String> {
    panic!("real search_api called — mock missing!");
}

async fn add(_a: i32, _b: i32) -> i32 {
    panic!("real add called");
}

elm_magic::view! {
    fn Search(query = String::new(), results: Vec<String> = vec![]) {
        on_change(query) after 300ms { results <- search_api(query.clone()) }
        <Col>
            <Input value={query.clone()} on_change={query = _} />
            {results.map(|r| <Row>"{r}"</Row>)}
        </Col>
    }
}

elm_magic::view! {
    fn Calc(x = 5, sum = 0) {
        <Col>
            <Button on_click={sum <- add(x, 42)}>"calc"</Button>
            "sum: {sum}"
        </Col>
    }
}

#[test]
fn mock_method_replaces_effect_fn() {
    let app = elm_magic::mount!(Search);
    // 사양서 8.2: `app.mock(search_api, |q| vec![..])`
    let mut app = app.mock(search_api, |q: String| vec![format!("hit:{q}")]);
    app.type_("rust");
    app.advance(300);
    app.flush();
    app.assert_text("hit:rust");
}

#[test]
fn mock_method_two_args() {
    let app = elm_magic::mount!(Calc);
    let mut app = app.mock2(add, |a: i32, b: i32| a * b);
    app.click("calc");
    app.flush();
    app.assert_text("sum: 210");
}

#[test]
fn assert_helpers() {
    let app = elm_magic::mount!(Search);
    let mut app = app.mock(search_api, |q: String| vec![format!("hit:{q}")]);
    app.type_into("input", "elm");
    app.advance(300);
    app.flush();
    app.assert_text("hit:elm");
    app.assert_visible("hit:elm");
    app.assert_hidden("hit:rust");
}

#[test]
fn mock_macro_and_method_share_the_registry() {
    let app = elm_magic::mount!(Search);
    let mut app = app.mock(search_api, |q: String| vec![format!("m:{q}")]);
    // 같은 키로 덮어쓰면 나중 것이 이긴다
    elm_magic::mock!(app, search_api, |q: String| vec![format!("later:{q}")]);
    app.type_("x");
    app.advance(300);
    app.flush();
    app.assert_text("later:x");
}