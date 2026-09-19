//! 사양서 8.3 — `insta::assert_snapshot!` 스냅샷 계약.
//!
//! `tests/snapshot.rs`(serde JSON: 구조·텍스트·클래스만 남기고 핸들러를 버리는 계약)와
//! 역할이 다르다. 이쪽은 **표현 회귀**를 사람이 리뷰하는 스냅샷으로 잡는다 —
//! 트리 덤프나 접근성 트리가 조금이라도 바뀌면 diff가 리뷰 대상이 된다.
//!
//! 갱신: `INSTA_UPDATE=always cargo test --test snapshots` (또는 `cargo insta review`).
//! CI에서는 pending 스냅샷이 있으면 실패한다(기본 동작).

elm_magic::view! {
    fn SnapCounter(n = 0) {
        <Col>
            "Count: {n}"
            <Button on_click={n -= 1}>"-"</Button>
            <Button on_click={n += 1}>"+"</Button>
        </Col>
    }
}

elm_magic::css! {
    .snap_gallery { gap: 8; padding: 16; bg: surface; }
    .snap_dim { color: text_dim; }
}

elm_magic::view! {
    fn SnapGallery(checked = false, tab = 0, items: Vec<String> = vec![]) {
        <Col class="snap_gallery">
            <Row>"row"</Row>
            <Text class="snap_dim">"text"</Text>
            <Strong>"strong"</Strong>
            <Button>"button"</Button>
            <Input value={String::new()} />
            <TextArea value={String::new()} />
            <Check checked={checked} on_change={checked = _}>"check"</Check>
            <Tab active={tab == 0} on_click={tab = 0}>"tab"</Tab>
            <Th on_click={tab = 1}>"th"</Th>
            <Td>"cell"</Td>
            <Banner kind="error">"banner"</Banner>
            <Spinner />
            <Divider />
            <Progress value={0.25} />
            <Modal on_close={checked = false}><Text>"modal"</Text></Modal>
            {items.map(|i| <Row>"item:{i}"</Row>)}
        </Col>
    }
}

fn gallery() -> elm_magic::testing::TestApp<SnapGallery> {
    elm_magic::mount_with::<SnapGallery>(SnapGalleryProps {
        items: Some(vec!["a".to_string(), "b".to_string()]),
        ..Default::default()
    })
}

#[test]
fn counter_render_tree_snapshot() {
    let app = elm_magic::mount!(SnapCounter);
    insta::assert_snapshot!("counter_render_tree", app.render_tree());
}

#[test]
fn counter_a11y_tree_snapshot() {
    let app = elm_magic::mount!(SnapCounter);
    insta::assert_snapshot!("counter_a11y_tree", app.a11y_tree());
}

#[test]
fn gallery_render_tree_snapshot() {
    let app = gallery();
    insta::assert_snapshot!("gallery_render_tree", app.render_tree());
}

#[test]
fn gallery_a11y_tree_snapshot() {
    let app = gallery();
    insta::assert_snapshot!("gallery_a11y_tree", app.a11y_tree());
}

#[test]
fn checked_gallery_render_tree_snapshot() {
    // 상태가 트리 표현에 어떻게 반영되는지도 스냅샷으로 남긴다
    let mut app = gallery();
    app.toggle("check");
    insta::assert_snapshot!("gallery_checked_render_tree", app.render_tree());
}

#[cfg(feature = "serde")]
#[test]
fn gallery_json_snapshot() {
    let app = gallery();
    insta::assert_json_snapshot!("gallery_json", app.element());
}
