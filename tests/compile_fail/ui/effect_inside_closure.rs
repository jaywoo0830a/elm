// 사양서 5.1 — 효과(`<-`)는 문장 위치에서만 쓸 수 있다. 클로저 안은 컴파일 에러여야 한다.
async fn api() -> String {
    String::new()
}

elm_magic::view! {
    fn Bad(msg = String::new()) {
        let start = || {
            msg <- api()
        };
        let _ = start;
        <Col>"x"</Col>
    }
}

fn main() {}
