//! 0.7.4 — 리포트 버그 13 (`css!` 자기등록이 플랫폼별로 안 돌던 문제) 회귀.
//!
//! 시작 섹션(ctor) 수정은 Windows/macOS에서만 실측할 수 있으므로, 여기서는
//! **명시적 초기화 경로**(`style::init_styles`)가 등록을 다시 채우는지 고정한다.
//!
//! 전역 레지스트리를 비우는 테스트라 **이 파일 하나만** 둔다 — 테스트마다
//! 프로세스가 분리되므로 다른 테스트 바이너리에 영향을 주지 않는다.

elm_magic::css! {
    .init_probe { padding: 7; }
    .init_probe--active { color: error; }
}

#[test]
fn init_styles_reapplies_registrations_after_reset() {
    // 시작 섹션(ELF `.init_array` — 이 호스트)이 먼저 돌아 등록돼 있다.
    assert!(
        elm_magic::style::len() >= 2,
        "css!가 자기등록됐다 (len={})",
        elm_magic::style::len()
    );

    elm_magic::style::reset_for_tests();
    assert_eq!(elm_magic::style::len(), 0, "테스트를 위해 비웠다");

    // `main()`에서 부르는 한 줄 — MSVC 등 시작 등록이 안 도는 플랫폼의 안전판.
    elm_magic::style::init_styles();
    assert!(
        elm_magic::style::len() >= 2,
        "init_styles가 등록을 다시 채운다 (len={})",
        elm_magic::style::len()
    );

    let probe = elm_magic::style::lookup_class("init_probe").expect(".init_probe");
    assert_eq!(probe.get("padding").as_deref(), Some("7"));
    assert!(
        elm_magic::style::lookup_class("init_probe--active").is_some(),
        "BEM 수정자도 함께 되살아난다"
    );

    // 멱등: 두 번 불러도 중복 등록되지 않는다.
    let before = elm_magic::style::len();
    elm_magic::style::init_styles();
    assert_eq!(elm_magic::style::len(), before, "init_styles는 멱등하다");
}
