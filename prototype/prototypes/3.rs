```rust
// ============================================================
// 고급 패턴 20 — 복잡한 앱의 응용 사례
// ============================================================

// ── 1. Undo / Redo ─────────────────────────────────────────
#[view]
fn Editor(text = "", history: Vec<String> = vec![], cursor = 0) {
    on_key("Ctrl+Z") { cursor -= 1; text = history[cursor] }
    on_key("Ctrl+Y") { cursor += 1; text = history[cursor] }

    on_change(text) after 500ms {
        history.truncate(cursor + 1);
        history.push(text.clone());
        cursor = history.len() - 1;
    }

    <TextArea value={text} on_change={text = _} />
}

// ── 2. 낙관적 업데이트 (optimistic) ─────────────────────────
#[view]
fn Likes(post: Post, liked = false) {
    <Button
        active={liked}
        on_click={
            liked = !liked;                          // 즉시 반영
            revert <- api.toggle_like(post.id)       // 실패 시 되돌림
        }>
        "{post.likes + liked as i32}"
    </Button>
}

// ── 3. 서버 캐시 (stale-while-revalidate) ──────────────────
#[view]
fn Profile(id: UserId, user: Option<User> = cache.get(id), fresh = false) {
    on_mount {
        if user.is_none() { user <- api.user(id) }
        fresh <- api.user(id) after 5min    // 백그라운드 갱신
    }

    {match user {
        None    => <Skeleton />,
        Some(u) => <UserCard user={u} stale={!fresh} />,
    }}
}

// ── 4. 실시간 구독 (WebSocket) ─────────────────────────────
#[view]
fn Chat(room: RoomId, msgs: Vec<Msg> = vec![], connected = false) {
    on_mount {
        connected <- ws.connect(format!("/chat/{room}"));
    }
    on_message(ws, m) {
        msgs.push(m);
    }

    <Col>
        <Status dot={if connected { "green" } else { "red" }} />
        <Scroll stick_to_bottom>
            {msgs.map(|m| <ChatBubble msg={m} />)}
        </Scroll>
        <Input on_enter={ws.send(text)} />
    </Col>
}

// ── 5. 가상 스크롤 (동적 높이) ──────────────────────────────
#[view]
fn Feed(posts: Vec<Post> = vec![]) {
    <VirtualList
        items={posts}
        estimate={80}
        measure={|p| p.body.len() / 40 * 20 + 60}
        render={|p| <PostCard post={p} />}
        overscan={5}
    />
}

// ── 6. 드래그 앤 드롭 칸반 ─────────────────────────────────
#[view]
fn Kanban(cols: Vec<Column> = vec![], dragging: Option<Card> = None) {
    <Row>
        {cols.map(|c|
            <DropZone
                accept={Card}
                on_drop={|card| cols.move(card, c, index: _)}
                highlight={dragging.is_some()}>
                {c.cards.map(|card|
                    <Card
                        draggable
                        on_drag_start={dragging = Some(card)}
                        on_drag_end={dragging = None}
                    />
                )}
            </DropZone>
        )}
    </Row>
}

// ── 7. 커맨드 팔레트 ───────────────────────────────────────
#[view]
fn App(palette_open = false, query = "") {
    on_key("Ctrl+K") { palette_open = !palette_open }

    {if palette_open {
        <Modal>
            <Input value={query} auto_focus on_change={query = _} />
            <List>
                {commands.search(query).map(|cmd|
                    <Row on_click={palette_open = false; cmd.run()}>
                        "{cmd.label}" <Kbd>"{cmd.shortcut}"</Kbd>
                    </Row>
                )}
            </List>
        </Modal>
    }}
}

// ── 8. 폼 검증 (필드별 + 전체) ─────────────────────────────
#[view]
fn Signup(email = "", pw = "", touched = Set::new()) {
    let email_ok = email.contains("@");
    let pw_ok    = pw.len() >= 8;
    let valid    = email_ok && pw_ok;

    <Col>
        <Input value={email} on_blur={touched.insert("email")}
               error={touched.has("email") && !email_ok} />
        {if touched.has("email") && !email_ok { <Error>"Invalid email"</Error> }}
        <Input type="password" value={pw} on_blur={touched.insert("pw")} />
        {if pw_ok { <Progress value={pw_strength(pw)} /> }}
        <Button disabled={!valid} on_click={submit(email, pw)}>"Sign up"</Button>
    </Col>
}

// ── 9. 마법사 (wizard) ─────────────────────────────────────
#[view]
fn Wizard(step = 0, data = FormData::default()) {
    <Col>
        <Steps current={step} total={3} />
        {match step {
            0 => <Step1 data={data} on_next={step = 1} />,
            1 => <Step2 data={data} on_next={step = 2} on_back={step = 0} />,
            2 => <Step3 data={data} on_submit={submit(data)} />,
        }}
    </Col>
}

// ── 10. 라우팅 ─────────────────────────────────────────────
#[view]
fn Router(route = Route::Home) {
    on_navigate(|r| route = r);
    on_key("Alt+Left") { route = route.back() }

    {match route {
        Home            => <Home />,
        User(id)        => <Profile id={id} />,
        Post(id, slug)  => <PostPage id={id} slug={slug} />,
        NotFound        => <NotFound />,
    }}
}

// ── 11. 다국어 (i18n) ─────────────────────────────────────
#[view]
fn Welcome() {
    let t = i18n::use_lang();       // 반응형. 언어 바뀌면 자동 재렌더
    <Col>
        <h1>{t("welcome.title")}</h1>
        <p>{t("welcome.body", name: user.name)}</p>
        <LangPicker value={i18n::current()} on_change={i18n::set(_)} />
    </Col>
}

// ── 12. 접근성 (focus / aria) ─────────────────────────────
#[view]
fn Dialog(open = true) {
    <FocusTrap active={open} on_escape={open = false}>
        <Modal role="dialog" aria_label="Confirm">
            <Button auto_focus on_click={confirm()}>"OK"</Button>
            <Button on_click={open = false}>"Cancel"</Button>
        </Modal>
    </FocusTrap>
}

// ── 13. 키보드 네비게이션 ──────────────────────────────────
#[view]
fn List(rows: Vec<Row> = vec![], cursor = 0) {
    on_key("ArrowDown") { cursor = (cursor + 1).min(rows.len() - 1) }
    on_key("ArrowUp")   { cursor = cursor.saturating_sub(1) }
    on_key("Enter")     { rows[cursor].open() }

    <Col>
        {rows.enumerate().map(|(i, r)|
            <Row focused={i == cursor} on_click={cursor = i}>
                "{r.name}"
            </Row>
        )}
    </Col>
}

// ── 14. 크로스 컴포넌트 통신 (event bus) ───────────────────
#[view]
fn Toolbar() {
    <Button on_click={bus.emit(RefreshRequested)}>"Refresh"</Button>
}

#[view]
fn DataTable(items: Vec<Item> = vec![]) {
    on_event(RefreshRequested) { items <- api.items() }
    <Table>{items.map(|i| <Row>{i.name}</Row>)}</Table>
}

// ── 15. 파일 업로드 (진행률) ──────────────────────────────
#[view]
fn Uploader(files: Vec<Upload> = vec![]) {
    <Col>
        <DropZone on_files={|fs| fs.for_each(|f|
            files.push(Upload { name: f.name, progress: 0 });
            upload(f) progress -> u { files[u].progress = pct }
        )} />
        {files.map(|u|
            <Row>
                "{u.name}"
                <Progress value={u.progress} />
                {if u.progress == 100 { <Check /> } else { <Spinner /> }}
            </Row>
        )}
    </Col>
}

// ── 16. 오프라인 지원 ─────────────────────────────────────
#[view]
fn Notes(items: Vec<Note> = cache.load(), online = net.is_online()) {
    on_net_change { online = net.is_online() }

    <Col>
        {if !online { <Banner>"Offline — changes saved locally"</Banner> }}
        <Button on_click={note = Note::new(); items.push(note); cache.save(items)}>
            "New note"
        </Button>
        {items.map(|n| <NoteCard note={n} />)}
    </Col>

    on_event(BackOnline) { sync <- api.push(items) }
}

// ── 17. 백그라운드 작업 큐 ─────────────────────────────────
#[view]
fn Exporter(jobs: Vec<Job> = vec![]) {
    let start = || {
        let job = Job::new();
        jobs.push(job);
        run <- spawn_worker(job)       // 워커 스레드
    };

    <Col>
        <Button on_click={start}>"Export"</Button>
        {jobs.map(|j|
            <Row>
                "{j.name}"
                <Progress value={j.progress} />
                <Button on_click={j.cancel()}>"Cancel"</Button>
            </Row>
        )}
    </Col>
}

// ── 18. 낙관적 리스트 조작 ─────────────────────────────────
#[view]
fn TodoApp(items: Vec<Todo> = vec![]) {
    <Col>
        {items.map(|t|
            <Row>
                <Check checked={t.done} on_change={
                    t.done = !t.done;                        // 즉시
                    rollback <- api.toggle(t.id)             // 실패 시 자동 복구
                } />
                <Text strike={t.done}>{t.text}</Text>
            </Row>
        )}
    </Col>
}

// ── 19. 리치 텍스트 에디터 ────────────────────────────────
#[view]
fn Markdown(doc = "", preview = true) {
    <Row>
        <SplitPane>
            <TextArea value={doc} on_change={doc = _} />
            <Raw>|ui: &mut egui::Ui| {
                ui.add(egui::Label::new(egui::RichText::new(md::render(&doc))));
            }</Raw>
        </SplitPane>
    </Row>
}

// ── 20. 스냅샷 + 시간여행 디버깅 ───────────────────────────
#[view]
fn App(state: AppState = default(), timeline: Vec<Snapshot> = vec![], cursor = 0) {
    on_change(state) {
        timeline.truncate(cursor + 1);
        timeline.push(state.snapshot());
        cursor = timeline.len() - 1;
    }

    <Col>
        <DevToolbar>
            <Button on_click={cursor -= 1}>"◀"</Button>
            "{cursor} / {timeline.len() - 1}"
            <Button on_click={cursor += 1}>"▶"</Button>
            <Button on_click={state = timeline[cursor]}>"Restore"</Button>
        </DevToolbar>

        <Center>{state.render()}</Center>
    </Col>
}
```

---

## 관통하는 규칙

| 패턴 | 이 라이브러리에서 표현되는 방식 |
|---|---|
| Undo/Redo | `history` + `cursor` (평범한 값) |
| Optimistic | 대입 먼저, `revert <-` 나중 |
| Cache | `cache.get(id)`가 기본값, `<- ... after 5min` |
| 실시간 | `on_mount { <- ws.connect }` + `on_message` |
| Virtual scroll | `<VirtualList>` + `measure` 콜백 |
| Drag & Drop | `draggable` / `on_drop` / `accept` |
| Command palette | 그냥 `Modal` + `Input` + `filter` |
| Form 검증 | 파생 값 (`email_ok`) + `touched` |
| Wizard | `step` 정수 + `match` |
| Routing | `route` enum + `match` + `on_navigate` |
| i18n | `i18n::use_lang()`가 반응형 |
| A11y | `<FocusTrap>` / `role` / `auto_focus` |
| Keyboard nav | `on_key` + `cursor` 인덱스 |
| Event bus | `bus.emit` / `on_event` |
| Upload | `progress -> u { ... }` 스트림 |
| Offline | `net.is_online()` + `cache.save` |
| Backgroud | `spawn_worker` + `run <-` |
| Optimistic list | `t.done = !t.done` + `rollback <-` |
| Markdown | `<Raw>` 탈출구로 egui 위젯 |
| Time travel | `on_change(state)` + `timeline` |

---

## 복잡한 앱의 핵심

> **“복잡함은 상태가 아니라 조합에서 온다.”**

- Undo, Cache, i18n, Net, Bus — **전부 슬롯이다.** 특별한 API가 아니다.
- 비동기, 스트림, 백그라운드 — **전부 `<-`다.** `Cmd`, `Sub`, `spawn`은 어휘에 없다.
- 가상화, 드래그, 시간여행 — **전부 `<태그>` 또는 `on_*`다.** 라이브러리 이름은 어디에도 없다.

이 20개는 새로운 문법이 아니라, **1번에서 배운 다섯 가지 문법(변수, 대입, 태그, `<-`, `mock`)** 을 복잡한 상황에 반복 적용한 것뿐입니다. 그게 “인지부하 최소화 + 절대 코드 수 감소 + 고성능”이 동시에 가능한 이유입니다.
