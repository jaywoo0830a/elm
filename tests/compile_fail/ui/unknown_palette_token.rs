// 사양서 6.1 · 6.3 — 팔레트 토큰 14종 밖의 값은 컴파일 에러여야 한다.
elm_magic::css! {
    .bad { bg: nope; }
}

fn main() {}
