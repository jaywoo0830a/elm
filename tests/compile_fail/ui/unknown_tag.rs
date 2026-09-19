// 사양서 3.2 — 태그 어휘 밖의 이름은 컴파일 에러여야 한다.
elm_magic::view! {
    fn Bad() {
        <h1>"nope"</h1>
    }
}

fn main() {}
