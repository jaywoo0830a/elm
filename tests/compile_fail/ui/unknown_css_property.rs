// 사양서 6.1 — 36개 속성 밖의 이름은 컴파일 에러여야 한다.
elm_magic::css! {
    .bad { nope: 1; }
}

fn main() {}
