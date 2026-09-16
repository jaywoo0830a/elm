// ============================================================
// 20 패턴 — 클라이언트가 실제로 쓰는 모습
// ============================================================

// ── 1. 기본 상태 ─────────────────────────────────────────────
#[view]
fn Counter(n = 0) {
    <Row>
        <Button on_click={n -= 1}>"-"</Button>
        "{n}"
        <Button on_click={n += 1}>"+"</Button>
    </Row>
}

// ── 2. 입력 + 리스트 ─────────────────────────────────────────
#[view]
fn Todos(items: Vec<Todo> = vec![], text = "") {
    <Col>
        <Input value={text} on_enter={items.push(Todo { text })} />
        {items.map(|t| <Row>"{t.text}" <Button on_click={items.remove(t)}>"x"</Button></Row>)}
    </Col>
}

// ── 3. 폼 (여러 필드) ────────────────────────────────────────
#[view]
fn Signup(name = "", email = "", agreed = false) {
    <Col>
        <Input value={name}  placeholder="Name"  on_change={name = _} />
        <Input value={email} placeholder="Email" on_change={email = _} />
        <Check checked={agreed} on_change={agreed = !agreed}>"I agree"</Check>
        <Button disabled={!agreed || name.is_empty()} on_click={submit(name, email)}>
            "Sign up"
        </Button>
    </Col>
}

// ── 4. 파생 값 (computed) ────────────────────────────────────
#[view]
fn Cart(items: Vec<Item> = vec![]) {
    let total = items.iter().map(|i| i.price * i.qty).sum();
    let tax   = total * 0.1;
    <Col>
        {items.map(|i| <Row>"{i.name}" " — " "{i.price}"</Row>)}
        <Divider />
        "Subtotal: {total}"
        "Tax: {tax}"
        <Strong>"Total: {total + tax}"</Strong>
    </Col>
}

// ── 5. 조건부 렌더 ──────────────────────────────────────────
#[view]
fn Status(state = Idle) {
    {match state {
        Idle      => <Text>"Ready"</Text>,
        Loading   => <Spinner />,
        Ok        => <Text class="ok">"Done"</Text>,
        Failed(e) => <Banner>{e}</Banner>,
    }}
}

// ── 6. 비동기 + 로딩 ────────────────────────────────────────
#[view]
fn Users(users: Vec<User> = vec![], status = Idle) {
    <Col>
        <Button on_click={status, users <- fetch_users()}>"Load"</Button>
        {if status.loading { <Spinner /> } else { users.map(|u| <Row>"{u.name}"</Row>) }}
    </Col>
}

// ── 7. 자식 컴포넌트 + 콜백 ─────────────────────────────────
#[view]
fn ItemRow(item: Item, on_select: fn(Id)) {
    <Row on_click={on_select(item.id)}>
        "{item.name}"
    </Row>
}

#[view]
fn List(items: Vec<Item> = vec![], selected = None) {
    {items.map(|i| <ItemRow item={i} on_select={selected = Some(_)} />)}
}

// ── 8. 전역 상태 (context) ──────────────────────────────────
#[store]
struct App { user: Option<User>, theme: Theme }

#[view]
fn Header() {
    <Row>
        "Hello, "{app.user.name}
        <Button on_click={app.theme = app.theme.toggle()}>"theme"</Button>
    </Row>
}

// ── 9. 효과 (mount / change) ────────────────────────────────
#[view]
fn Clock(now = 0) {
    on_mount { now <- tick_every(1000ms) }
    <Text>"{format_time(now)}"</Text>
}

// ── 10. 디바운스 검색 ───────────────────────────────────────
#[view]
fn Search(query = "", results: Vec<Hit> = vec![]) {
    <Col>
        <Input value={query} on_change={query = _} />
        on_change(query) after 300ms {
            results <- search_api(query)
        }
        {results.map(|h| <Row>"{h.title}"</Row>)}
    </Col>
}

// ── 11. 페이지네이션 ────────────────────────────────────────
#[view]
fn Pager(page = 0, size = 20, total = 100) {
    let pages = (total + size - 1) / size;
    <Row>
        <Button disabled={page == 0} on_click={page -= 1}>"‹"</Button>
        "{page + 1} / {pages}"
        <Button disabled={page >= pages - 1} on_click={page += 1}>"›"</Button>
    </Row>
}

// ── 12. 테이블 + 정렬 ──────────────────────────────────────
#[view]
fn Table(rows: Vec<Row> = vec![], sort = None, dir = Asc) {
    let sorted = rows.sorted_by(sort, dir);
    <Col>
        <Row>
            <Th on_click={sort = Name, dir = dir.toggle()}>"Name"</Th>
            <Th on_click={sort = Age,  dir = dir.toggle()}>"Age"</Th>
        </Row>
        {sorted.map(|r| <Row><Td>"{r.name}"</Td><Td>"{r.age}"</Td></Row>)}
    </Col>
}

// ── 13. 탭 ──────────────────────────────────────────────────
#[view]
fn Tabs(tab = Home) {
    <Col>
        <Row>
            <Tab active={tab == Home}  on_click={tab = Home}>"Home"</Tab>
            <Tab active={tab == Stats} on_click={tab = Stats}>"Stats"</Tab>
            <Tab active={tab == Logs}  on_click={tab = Logs}>"Logs"</Tab>
        </Row>
        {match tab {
            Home  => <Home />,
            Stats => <Stats />,
            Logs  => <Logs />,
        }}
    </Col>
}

// ── 14. 모달 ────────────────────────────────────────────────
#[view]
fn App(open = false) {
    <Col>
        <Button on_click={open = true}>"Open"</Button>
        {if open {
            <Modal on_close={open = false}>
                <Text>"Are you sure?"</Text>
                <Row>
                    <Button on_click={confirm()}>"Yes"</Button>
                    <Button on_click={open = false}>"No"</Button>
                </Row>
            </Modal>
        }}
    </Col>
}

// ── 15. 키보드 단축키 ──────────────────────────────────────
#[view]
fn Editor(text = "") {
    on_key("Ctrl+S") { save(text) }
    on_key("Ctrl+Z") { text = undo() }
    <TextArea value={text} on_change={text = _} />
}

// ── 16. 드래그 앤 드롭 ─────────────────────────────────────
#[view]
fn Kanban(cols: Vec<Col> = vec![]) {
    <Row>
        {cols.map(|c|
            <DropZone on_drop={cols.move_to(_, c)}>
                {c.cards.map(|card| <Card draggable={card} />)}
            </DropZone>
        )}
    </Row>
}

// ── 17. 애니메이션 ─────────────────────────────────────────
#[view]
fn Fade(visible = true) {
    <Fade in={visible} duration={200ms}>
        <Text>"Hello"</Text>
    </Fade>
}

// ── 18. 에러 경계 ──────────────────────────────────────────
#[view]
fn Safe() {
    <ErrorBoundary fallback={|e| <Banner>{e}</Banner>}>
        <RiskyWidget />
    </ErrorBoundary>
}

// ── 19. 테마 + 플랫폼 ─────────────────────────────────────
#[view]
fn Root() {
    <Theme preset={app.theme}>
        <Layout>
            <Sidebar />
            <Main />
        </Layout>
    </Theme>
}

fn main() {
    rui::run(Desktop, Root);
    // rui::run(Web, Root);
    // rui::run(Terminal, Root);
    // rui::run(Headless, Root);   // CI / 스냅샷
}

// ── 20. 테스트 ─────────────────────────────────────────────
#[test]
fn cart_total() {
    let app = Cart(items: vec![
        Item { name: "A", price: 10, qty: 2 },
        Item { name: "B", price: 5,  qty: 1 },
    ]);
    app.assert_text("Subtotal: 25");
    app.assert_text("Total: 27.5");
}

#[test]
fn search_debounce() {
    let app = Search().mock(search_api, |q| vec![Hit { title: format!("hit:{q}") }]);
    app.type_("input", "rust");
    app.advance(300ms);
    app.assert_text("hit:rust");
}

#[test]
fn tabs_switch() {
    let app = Tabs();
    app.click("Stats");
    app.assert_visible("Stats");
    app.assert_hidden("Home");
}
