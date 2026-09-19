// 사양서 3.1 — `{_}`(전달값)는 콜백 prop에서만 의미가 있다. 클릭 이벤트에서 쓰면
// 값이 없으므로 컴파일 에러여야 한다 (현황 3.1 항목 11).
elm_magic::view! {
    fn Bad(selected: Option<i32> = None) {
        <Button on_click={selected = Some(_)}>"pick"</Button>
    }
}

fn main() {}
