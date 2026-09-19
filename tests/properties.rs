//! 모델 기반(프로퍼티) 테스트 — 사양서 3·4·5·6의 규칙을 **무작위 입력**에 대해
//! 독립적으로 계산한 모델과 비교한다.
//!
//! 단일 예제 테스트가 놓치는 조합을 찾고, 실패하면 proptest가 **최소 반례로 축약**해
//! 준다. 재현 시드는 `tests/properties.proptest-regressions`에 쌓이므로 커밋한다.
//!
//! 케이스 수는 32로 낮춰 CI 시간을 억제한다 (`PROPTEST_CASES`로 늘릴 수 있다).

use proptest::prelude::*;

// ── 1. 카운터 = 정수 산술 모델 ──────────────────────────────

#[derive(Clone, Copy, Debug)]
enum Op {
    Inc,
    Dec,
    Reset,
}

fn op_strategy() -> impl Strategy<Value = Op> {
    prop_oneof![Just(Op::Inc), Just(Op::Dec), Just(Op::Reset)]
}

elm_magic::view! {
    fn PropCounter(n = 0) {
        <Col>
            "n: {n}"
            <Button on_click={n += 1}>"inc"</Button>
            <Button on_click={n -= 1}>"dec"</Button>
            <Button on_click={n = 0}>"reset"</Button>
        </Col>
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn counter_matches_arithmetic_model(ops in prop::collection::vec(op_strategy(), 0..12)) {
        let mut app = elm_magic::mount!(PropCounter);
        let mut model: i64 = 0;
        for op in ops {
            match op {
                Op::Inc => {
                    app.click("inc");
                    model += 1;
                }
                Op::Dec => {
                    app.click("dec");
                    model -= 1;
                }
                Op::Reset => {
                    app.click("reset");
                    model = 0;
                }
            }
        }
        let text = app.text();
        prop_assert!(
            text.contains(&format!("n: {model}")),
            "model={} text={:?}",
            model,
            text
        );
    }
}

// ── 2. 리스트 = Vec 모델 (순서까지) ─────────────────────────

#[derive(Clone, Copy, Debug)]
enum ListOp {
    Reverse,
    DropFirst,
    Sort,
}

fn list_op_strategy() -> impl Strategy<Value = ListOp> {
    prop_oneof![
        Just(ListOp::Reverse),
        Just(ListOp::DropFirst),
        Just(ListOp::Sort)
    ]
}

elm_magic::view! {
    fn PropIds(ids: Vec<i32> = vec![]) {
        <Col>
            {ids.map(|i| <Row>"{i}"</Row>)}
            <Button on_click={ids.reverse()}>"reverse"</Button>
            <Button on_click={ids.remove(0)}>"drop"</Button>
            <Button on_click={ids.sort()}>"sort"</Button>
        </Col>
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn list_matches_vec_model(
        ids in prop::collection::vec(0i32..50, 0..6),
        ops in prop::collection::vec(list_op_strategy(), 0..6),
    ) {
        let mut app = elm_magic::mount_with::<PropIds>(PropIdsProps {
            ids: Some(ids.clone()),
            ..Default::default()
        });
        let mut model = ids.clone();

        for op in ops {
            match op {
                ListOp::Reverse => {
                    app.click("reverse");
                    model.reverse();
                }
                ListOp::DropFirst => {
                    // 빈 리스트에서 `remove(0)`은 패닉이므로 모델이 먼저 판단한다
                    if !model.is_empty() {
                        app.click("drop");
                        model.remove(0);
                    }
                }
                ListOp::Sort => {
                    app.click("sort");
                    model.sort();
                }
            }
        }

        let expected = model
            .iter()
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let text = app.text();
        prop_assert!(
            text.starts_with(&expected),
            "model={:?}\ntext={:?}",
            model,
            text
        );
    }
}

// ── 3. keyed 재정렬 = 키를 따라가는 상태 모델 ───────────────

elm_magic::view! {
    fn PropKeyedItem(id = 0, n = 0) {
        <Row>
            "k{id}:{n}"
            <Button on_click={n += 1}>"k{id}+"</Button>
        </Row>
    }
}

elm_magic::view! {
    fn PropKeyedList(ids: Vec<i32> = vec![]) {
        <Col>
            {ids.map(|id| <Row key={id}><PropKeyedItem id={id} /></Row>)}
            <Button on_click={ids.reverse()}>"reverse"</Button>
        </Col>
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn keyed_state_follows_the_key_across_reorders(
        ids in prop::collection::vec(1i32..6, 1..4),
        clicks in prop::collection::vec(0usize..3, 0..8),
    ) {
        let mut unique = ids.clone();
        unique.sort_unstable();
        unique.dedup();
        prop_assume!(!unique.is_empty());

        let mut app = elm_magic::mount_with::<PropKeyedList>(PropKeyedListProps {
            ids: Some(unique.clone()),
            ..Default::default()
        });
        let mut counts = std::collections::BTreeMap::new();
        for id in &unique {
            counts.insert(*id, 0i32);
        }

        for pick in clicks {
            let id = unique[pick % unique.len()];
            app.click(&format!("k{id}+"));
            *counts.get_mut(&id).unwrap() += 1;
        }

        // 순서를 뒤집어도 키별 상태는 그대로여야 한다
        app.click("reverse");
        let text = app.text();
        for (id, n) in counts {
            prop_assert!(
                text.contains(&format!("k{id}:{n}")),
                "key {id}의 상태가 재정렬 뒤에 {n}이 아니다: {text:?}"
            );
        }
    }
}

// ── 4. 입력 = 문자열 왕복 모델 ──────────────────────────────

elm_magic::view! {
    fn PropTyped(value = String::new()) {
        <Col>
            <Input value={value.clone()} on_change={value = _} />
            "v: {value}"
        </Col>
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn typing_round_trips_arbitrary_strings(
        chars in prop::collection::vec(prop::char::any(), 0..24)
    ) {
        let value: String = chars
            .into_iter()
            .map(|c| if c == '\r' { 'x' } else { c })
            .collect();
        let mut app = elm_magic::mount!(PropTyped);
        app.type_(&value);
        let text = app.text();
        prop_assert!(text.contains(&value), "value={:?}\ntext={:?}", value, text);
    }
}

// ── 5. 디바운스 = "마지막 값, 지연 후 한 번" 모델 ────────────

elm_magic::view! {
    fn PropDebounce(query = String::new(), fired = String::new(), fires = 0) {
        on_change(query) after 300ms {
            fired = query.clone();
            fires += 1;
        }
        <Col>
            <Input value={query.clone()} on_change={query = _} />
            "fired: {fired}"
            "fires: {fires}"
        </Col>
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    /// kind 0 = 새 값 입력, kind 1 = 시계 진행. 모델은 "변경 후 누적 300ms가
    /// 지나면 그 값이 정확히 한 번 발화한다"는 규칙만 안다.
    #[test]
    fn debounce_matches_last_value_model(
        steps in prop::collection::vec((0u8..2, 0u16..400), 0..6)
    ) {
        let mut app = elm_magic::mount!(PropDebounce);
        let mut typed = 0usize;
        let mut pending: Option<(String, u32)> = None;
        let mut fired: Option<String> = None;
        let mut fires = 0u32;

        for (kind, ms) in &steps {
            if *kind == 0 {
                typed += 1;
                let value = format!("v{typed}");
                app.type_(&value);
                pending = Some((value, 0));
            } else {
                app.advance(*ms as u64);
                if let Some((value, acc)) = pending.take() {
                    let acc = acc + *ms as u32;
                    if acc >= 300 {
                        fired = Some(value);
                        fires += 1;
                    } else {
                        pending = Some((value, acc));
                    }
                }
            }
        }

        let text = app.text();
        prop_assert!(
            text.contains(&format!("fires: {fires}")),
            "model fires={fires} steps={:?}\ntext={:?}",
            steps,
            text
        );
        if let Some(value) = fired {
            prop_assert!(
                text.contains(&format!("fired: {value}")),
                "model fired={value:?}\ntext={:?}",
                text
            );
        }
    }
}

// ── 6. 캐스케이드 = 명시도·선언 순서 모델 ────────────────────

elm_magic::css! {
    .prop_cascade_a { gap: 4; padding: 2; }
    .prop_cascade_b { gap: 8; }
    .prop_cascade_c { gap: 16; }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    /// 독립 모델: 같은 명시도(클래스)끼리는 **선언 순서가 뒤인 규칙**이 이긴다.
    #[test]
    fn cascade_picks_the_last_declaring_class(
        classes in prop::collection::vec(0usize..3, 0..4)
    ) {
        let names: Vec<String> = classes
            .iter()
            .map(|i| format!("prop_cascade_{}", ["a", "b", "c"][*i]))
            .collect();
        let resolved =
            elm_magic::style::resolve(&names, "", &elm_magic::style::Palette::dark());

        let expected_gap = classes
            .iter()
            .max()
            .map(|i| match i {
                0 => 4.0,
                1 => 8.0,
                _ => 16.0,
            });
        prop_assert_eq!(resolved.gap, expected_gap, "classes={:?}", classes);

        let expected_padding = if classes.contains(&0) {
            Some(elm_magic::style::Edges::splat(2.0))
        } else {
            None
        };
        prop_assert_eq!(resolved.padding, expected_padding, "classes={:?}", classes);
    }
}

// ── 7. 렌더 멱등성 = 어떤 상태에서도 두 번 그리면 같다 ──────

elm_magic::view! {
    fn PropIdem(n = 0, text = String::new()) {
        <Col>
            "n: {n}"
            "text: {text}"
            <Button on_click={n += 1}>"inc"</Button>
            <Input value={text.clone()} on_change={text = _} />
        </Col>
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 32, ..ProptestConfig::default() })]

    #[test]
    fn render_is_idempotent_for_random_states(
        clicks in 0u8..5,
        chars in prop::collection::vec(prop::char::any(), 0..8),
    ) {
        let mut app = elm_magic::mount!(PropIdem);
        for _ in 0..clicks {
            app.click("inc");
        }
        let value: String = chars
            .into_iter()
            .map(|c| if c == '\r' { 'x' } else { c })
            .collect();
        app.type_(&value);

        let first = app.render_tree();
        prop_assert_eq!(app.render_tree(), first.clone());
        prop_assert_eq!(app.render_tree(), first);
    }
}
