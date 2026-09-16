// v0.3 — mock (사양서 8.2)
// `mock!(app, fn_name, |args| ...)` — 효과 함수를 런타임에 교체.
// 실제 구현은 호출되지 않아야 한다(네트워크 없음).

async fn search_api(_q: String) -> Vec<String> {
    // 실제 네트워크 흉내 — 목으로 대체되지 않으면 flush 시 panic
    panic!("real search_api called — mock missing!");
}

elm_magic::view! {
    fn Search(query = String::new(), results: Vec<String> = vec![]) {
        on_change(query) after 300 { results <- search_api(query.clone()) }
        <Col>
            <Input value={query.clone()} on_change={query = _} />
            {results.map(|r| <Row>"{r}"</Row>)}
        </Col>
    }
}

#[test]
fn mock_replaces_effect_fn() {
    let mut app = elm_magic::mount!(Search);
    elm_magic::mock!(app, search_api, |q: String| vec![format!("hit:{}", q)]);
    app.type_("rust");
    app.advance(300);
    app.flush();
    app.expect_text("hit:rust");
}

#[test]
fn mock_receives_call_site_args() {
    let mut app = elm_magic::mount!(Search);
    elm_magic::mock!(app, search_api, |q: String| vec![
        format!("hit:{}", q),
        "second".to_string()
    ]);
    app.type_("elm");
    app.advance(300);
    app.flush();
    app.expect_text("hit:elm");
    app.expect_text("second");
}

// 2인자 효과 함수도 목 가능
async fn add(_a: i32, _b: i32) -> i32 {
    panic!("real add called");
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
fn mock_two_arg_effect() {
    let mut app = elm_magic::mount!(Calc);
    elm_magic::mock!(app, add, |a: i32, b: i32| a * b);
    app.click("calc");
    app.flush();
    // 실제 구현은 a+b=47, 목은 a*b=210
    app.expect_text("sum: 210");
}
