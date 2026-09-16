```rust
// ============================================================
// egui 통합 — 클라이언트가 보는 20 패턴
// 원칙: egui 타입은 어디에도 없다. 필요하면 탈출구만 있다.
// ============================================================

// ── 1. 기본 창 ──────────────────────────────────────────────
fn main() {
    rui::egui("My App", || ui! {
        <Col>
            <h1>"Hello, egui"</h1>
            <Counter />
        </Col>
    });
}

// ── 2. 사이드바 + 중앙 ─────────────────────────────────────
#[view]
fn Layout() {
    <Sidebar side="left" width={240} resizable>
        <NavMenu />
    </Sidebar>
    <Center>
        <Content />
    </Center>
}

// ── 3. 상단바 + 하단바 ────────────────────────────────────
#[view]
fn Chrome() {
    <TopBar><MenuBar /></TopBar>
    <BottomBar><StatusBar /></BottomBar>
    <Center><Editor /></Center>
}

// ── 4. 리사이즈 가능한 패널 ───────────────────────────────
#[view]
fn Panels() {
    <Sidebar side="right" width={320} min={200} max={600}>
        <Inspector />
    </Sidebar>
    <Center><Canvas /></Center>
}

// ── 5. egui_dock 스타일 탭 도킹 ───────────────────────────
#[view]
fn Dashboard(tabs = vec![Servers, Metrics, Logs]) {
    <Dock>
        {tabs.map(|t| <Tab id={t} title={t.label()}>
            {match t {
                Servers => <ServersPanel />,
                Metrics => <MetricsPanel />,
                Logs    => <LogsPanel />,
            }}
        </Tab>)}
    </Dock>
}

// ── 6. 스크롤 영역 ────────────────────────────────────────
#[view]
fn Feed(posts: Vec<Post> = vec![]) {
    <Scroll vertical>
        {posts.map(|p| <PostCard post={p} />)}
    </Scroll>
}

// ── 7. 접히는 섹션 ────────────────────────────────────────
#[view]
fn Settings(open = true) {
    <Collapsing title="Advanced" open={open} on_toggle={open = !open}>
        <Row>"Option A" <Switch /></Row>
        <Row>"Option B" <Slider value={0..100} /></Row>
    </Collapsing>
}

// ── 8. egui_plot 통합 ─────────────────────────────────────
#[view]
fn Chart(data: Vec<f32> = vec![]) {
    <Plot height={240} auto_bounds>
        <Line data={data} color="primary" />
        <Points data={data} size={4} />
        <HLine y={0.0} dashed />
    </Plot>
}

// ── 9. 실시간 스트리밍 차트 ────────────────────────────────
#[view]
fn LiveChart(samples: Vec<f32> = vec![]) {
    on_tick(16ms) { samples.push(read_sensor()); if samples.len() > 1000 { samples.remove(0) } }
    <Plot height={200}>
        <Line data={samples} />
    </Plot>
}

// ── 10. 캔들스틱 ───────────────────────────────────────────
#[view]
fn Candles(bars: Vec<Bar> = vec![]) {
    <Plot height={300}>
        <Candlestick data={bars} bull="green" bear="red" />
        <Volume data={bars} height_ratio={0.2} />
    </Plot>
}

// ── 11. 대용량 가상화 테이블 ──────────────────────────────
#[view]
fn Logs(rows: Vec<LogRow> = vec![]) {
    <Table virtualized height={400} row_height={20}>
        <Column field="time"   width={160} />
        <Column field="level"  width={80}  />
        <Column field="msg"    flex />
        <Column field="source" width={120} />
        {rows.map(|r| <Row data={r} />)}
    </Table>
}

// ── 12. 이미지 표시 ───────────────────────────────────────
#[view]
fn Viewer(tex: Texture = default()) {
    <Image source={tex} fit="contain" zoom={1.0..4.0} />
}

// ── 13. 드래그 가능한 값 ──────────────────────────────────
#[view]
fn Vec3(x = 0.0, y = 0.0, z = 0.0) {
    <Row>
        <DragValue value={x} speed={0.1} range={-10..10} prefix="x " />
        <DragValue value={y} speed={0.1} range={-10..10} prefix="y " />
        <DragValue value={z} speed={0.1} range={-10..10} prefix="z " />
    </Row>
}

// ── 14. 컬러 피커 ─────────────────────────────────────────
#[view]
fn Theme(color = rgb(60, 120, 220)) {
    <Row>
        <ColorPicker value={color} alpha />
        <Text>"Preview"</Text> .bg({color})
    </Row>
}

// ── 15. 컨텍스트 메뉴 ─────────────────────────────────────
#[view]
fn FileTree(nodes: Vec<Node> = vec![]) {
    <Col>
        {nodes.map(|n|
            <Row on_context_menu={menu! {
                "Rename" => rename(n),
                "Delete" => nodes.remove(n),
                sep,
                "Properties" => inspect(n),
            }}>
                "{n.name}"
            </Row>
        )}
    </Col>
}

// ── 16. 툴팁 + 호버 ───────────────────────────────────────
#[view]
fn InfoButton() {
    <Button on_hover={tooltip!("Shows detailed info")}>
        "?"
    </Button>
}

// ── 17. 파일 다이얼로그 ───────────────────────────────────
#[view]
fn Loader(path: Option<PathBuf> = None) {
    <Col>
        <Button on_click={path <- rfd::open_file("*.txt;*.md")}>
            "Open file"
        </Button>
        {if let Some(p) = &path { <Text>"{p.display()}"</Text> }}
    </Col>
}

// ── 18. 네이티브 메뉴바 ──────────────────────────────────
#[view]
fn Menus() {
    <MenuBar>
        <Menu label="File">
            <Item on_click={new()}>"New"  shortcut="Ctrl+N"</Item>
            <Item on_click={open()}>"Open" shortcut="Ctrl+O"</Item>
            <Sep />
            <Item on_click={quit()}>"Quit" shortcut="Ctrl+Q"</Item>
        </Menu>
        <Menu label="Edit">
            <Item on_click={undo()}>"Undo"</Item>
            <Item on_click={redo()}>"Redo"</Item>
        </Menu>
    </MenuBar>
}

// ── 19. egui 탈출구 (escape hatch) ───────────────────────
#[view]
fn Custom() {
    <Col>
        <Text>"Normal widgets above"</Text>
        <Raw>|ui: &mut egui::Ui| {
            ui.hyperlink_to("egui docs", "https://docs.rs/egui");
            ui.add(egui::Slider::new(&mut 42.0, 0.0..100.0).text("raw slider"));
        }</Raw>
        <Text>"Normal widgets below"</Text>
    </Col>
}

// ── 20. 창 분리 (multi-window) ──────────────────────────
#[view]
fn Root(windows: Vec<Window> = vec![]) {
    <Col>
        <Button on_click={windows.push(Window::new("Inspector", || ui! { <Inspector /> }))}>
            "Open inspector"
        </Button>
        <Center><Main /></Center>
    </Col>

    {windows.map(|w|
        <Window id={w.id} title={w.title} open on_close={windows.remove(w)}>
            {w.content()}
        </Window>
    )}
}
```

---

## egui가 어디에 숨어 있는가

| 클라이언트가 쓰는 것 | egui에서 오는 것 | 숨는 방식 |
|---|---|---|
| `<Sidebar>` | `egui::SidePanel::left` | 어댑터가 변환 |
| `<TopBar>` | `egui::TopBottomPanel::top` | 어댑터가 변환 |
| `<Scroll>` | `egui::ScrollArea::vertical` | 어댑터가 변환 |
| `<Collapsing>` | `egui::CollapsingHeader` | 어댑터가 변환 |
| `<Plot>` | `egui_plot::Plot` | 별도 어댑터 |
| `<Candlestick>` | `egui-charts` | 별도 어댑터 |
| `<Table virtualized>` | `oxiui-table` | 별도 어댑터 |
| `<Dock>` | `egui_dock::DockState` | 별도 어댑터 |
| `<Window>` | `egui::Window` | 어댑터가 변환 |
| `<MenuBar>` | `egui::menu::bar` | 어댑터가 변환 |
| `<Raw>` | 그대로 egui | 탈출구 |

---

## 관통하는 규칙

> **egui는 “플랫폼” 중 하나일 뿐이다.**
> `rui::egui(...)`로 시작하면 egui 어댑터가 붙고,
> `rui::web(...)`로 시작하면 웹 어댑터가 붙는다.
> 컴포넌트 코드는 어느 쪽인지 모른다.

그래서:

- **테스트는 `rui::headless`로** — egui Context 없이 돈다.
- **`<Raw>`는 유일한 탈출구** — egui 특수 위젯이 필요할 때만.
- **`egui_dock`, `egui_plot`, `oxiui-table`은 어휘로 승격** — 클라이언트는 라이브러리 이름조차 모른다.

이 20개 패턴의 절반은 egui 고유 기능이고, 나머지 절반은 이식 가능한 어휘입니다. 클라이언트는 둘을 구분 없이 쓰지만, 헤드리스 테스트에서는 이식 가능한 절반만 검증하고, egui 고유 절반은 스냅샷 테스트로 커버합니다.
