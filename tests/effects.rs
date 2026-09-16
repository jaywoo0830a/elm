// v0.2 — 효과 (사양서 5.1, 5.2, 8.1, 8.2)

// 테스트 더블: 실제 네트워크 없이 즉시 완료되는 future
async fn load_users() -> (String, Vec<String>) {
    ("done".to_string(), vec!["alice".to_string(), "bob".to_string()])
}

// ── 1. on_mount + `<-` (사양서 5.2) ─────────────────────────
elm_magic::view! {
    fn Users(users: Vec<String> = vec![], status = String::from("idle")) {
        on_mount { status, users <- load_users() }
        <Col>
            <Button on_click={status, users <- load_users()}>"Load"</Button>
            "status: {status}"
            {users.map(|u| <Row>"{u}"</Row>)}
        </Col>
    }
}

#[test]
fn on_mount_effect_runs_on_flush() {
    let mut app = elm_magic::mount!(Users);
    // flush 전: 대기 중
    app.expect_text("status: idle");
    app.flush();
    app.expect_text("status: done");
    app.expect_text("alice");
    app.expect_text("bob");
}

#[test]
fn click_can_fire_effect() {
    let mut app = elm_magic::mount!(Users);
    app.click("Load");
    app.flush();
    app.expect_text("status: done");
    app.expect_text("alice");
}

// ── 2. 단일 타깃 `<-` (사양서 5.1) ───────────────────────────
async fn fetch_greeting() -> String {
    "hello".to_string()
}

elm_magic::view! {
    fn Greeting(msg = String::new()) {
        <Col>
            <Button on_click={msg <- fetch_greeting()}>"greet"</Button>
            "msg: {msg}"
        </Col>
    }
}

#[test]
fn single_target_effect() {
    let mut app = elm_magic::mount!(Greeting);
    app.click("greet");
    app.flush();
    app.expect_text("msg: hello");
}

// ── 3. 낙관적 업데이트: 대입 먼저, `<-` 나중 (3.rs 패턴 2) ──
elm_magic::view! {
    fn Likes(count = 0) {
        <Col>
            <Button on_click={
                count += 1;
                count <- optimistic_like(count)
            }>"like"</Button>
            "likes: {count}"
        </Col>
    }
}

async fn optimistic_like(n: i32) -> i32 {
    n * 10
}

#[test]
fn optimistic_then_effect_overwrites() {
    let mut app = elm_magic::mount!(Likes);
    app.click("like");
    // 즉시 반영 (flush 전)
    app.expect_text("likes: 1");
    app.flush();
    // 효과 결과로 덮어씀
    app.expect_text("likes: 10");
}
