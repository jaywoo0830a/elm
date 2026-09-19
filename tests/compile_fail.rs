//! 컴파일 실패 계약 — 사양서가 "컴파일 에러"라고 약속한 것들을 `.stderr`로 고정한다.
//!
//! 사양서 13은 "에러 메시지가 뭉개진다"를 트레이드오프로 인정하므로, 메시지 자체를
//! 회귀 테스트로 잡는다. 즉 이 테스트는 **문서화된 컴파일 계약**이다.
//!
//! `.stderr`는 rustc 버전에 민감하다. 툴체인을 올리면
//! `TRYBUILD=overwrite cargo test --test compile_fail`로 재생성하고 diff를 리뷰한다.

#[test]
fn compile_fail_contract() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile_fail/ui/*.rs");
}
