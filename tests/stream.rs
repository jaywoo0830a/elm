// v0.3 — 스트림 (사양서 5.4)
// `expr -> slot { body }` — 스트림이 내보내는 값마다 슬롯에 반영하고 본문 실행.
// 백프레셔·펌핑은 테스트 런타임(app.pump())이 처리한다.

fn upload_progress() -> impl Iterator<Item = i32> {
    vec![25, 50, 75, 100].into_iter()
}

elm_magic::view! {
    fn Upload(pct = 0) {
        <Col>
            <Button on_click={
                upload_progress() -> pct { }
            }>"upload"</Button>
            "progress: {pct}%"
        </Col>
    }
}

#[test]
fn stream_pumps_values_into_slot() {
    let mut app = elm_magic::mount!(Upload);
    app.click("upload");
    app.expect_text("progress: 0%");
    app.pump();
    app.expect_text("progress: 25%");
    app.pump();
    app.expect_text("progress: 50%");
    app.pump();
    app.pump();
    app.expect_text("progress: 100%");
    // 소진 후: 더 이상 변화 없음
    app.pump();
    app.pump();
    app.expect_text("progress: 100%");
}

// 본문이 매 값마다 실행되는지 (슬롯 상태 변경 포함)
elm_magic::view! {
    fn UploadLog(pct = 0, log = String::new()) {
        <Col>
            <Button on_click={
                upload_progress() -> pct { log = format!("{}[{}]", log, pct) }
            }>"go"</Button>
            "{log}"
        </Col>
    }
}

#[test]
fn stream_body_runs_per_value() {
    let mut app = elm_magic::mount!(UploadLog);
    app.click("go");
    app.pump();
    app.pump();
    app.pump();
    app.pump();
    app.expect_text("[25][50][75][100]");
}
