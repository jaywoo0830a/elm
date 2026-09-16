// ============================================================
// 클라이언트가 보는 코드 — JS처럼 짧고, Rust처럼 빠르게
// 구현은 상상. 문법과 계약만 본다.
// ============================================================


// ============================================================
// 1. 가장 작은 컴포넌트 — Counter
// ============================================================

#[component]
fn Counter(start = 0) {
    let count = state(start);

    ui! {
        <Col gap=8>
            <Text class="title">"Count: {count}"</Text>
            <Row gap=4>
                <Button class="primary" on_click={|| count += 1}>"+"</Button>
                <Button class="ghost"   on_click={|| count -= 1}>"-"</Button>
            </Row>
        </Col>
    }
}

// JS 비교:
//   function Counter({ start = 0 }) {
//     const [count, setCount] = useState(start);
//     return (
//       <Col gap={8}>
//         <Text className="title">Count: {count}</Text>
//         <Row gap={4}>
//           <Button className="primary" onClick={() => setCount(c => c+1)}>+</Button>
//           <Button className="ghost"   onClick={() => setCount(c => c-1)}>-</Button>
//         </Row>
//       </Col>
//     );
//   }
// 거의 같다. 다른 건 없다. 컴파일은 Rust가 한다.


// ============================================================
// 2. 리스트 + 자식 컴포넌트 — TodoItem, TodoList
// ============================================================

#[component]
fn TodoItem(todo: Todo, on_toggle: fn(u32), on_remove: fn(u32)) {
    ui! {
        <Row class={if todo.done { "item done" } else { "item" }} gap=6>
            <Check checked={todo.done} on_change={move || on_toggle(todo.id)} />
            <Text>{todo.text}</Text>
            <Button class="icon" on_click={move || on_remove(todo.id)}>"x"</Button>
        </Row>
    }
}

#[component]
fn TodoList() {
    let todos   = state::<Vec<Todo>>(vec![]);
    let input   = state(String::new());
    let next_id = state(1u32);

    let add = move || {
        if input.trim().is_empty() { return; }
        todos.push(Todo { id: next_id, text: input.clone(), done: false });
        next_id += 1;
        input.clear();
    };

    ui! {
        <Col gap=8>
            <Row gap=4>
                <Input value={input} placeholder="What to do?" on_enter={add} />
                <Button class="primary" on_click={add}>"Add"</Button>
            </Row>

            <Col gap=2>
                {todos.iter().map(|todo| ui! {
                    <TodoItem
                        key={todo.id}
                        todo={todo.clone()}
                        on_toggle={|id| todos.toggle(id)}
                        on_remove={|id| todos.remove(id)}
                    />
                })}
            </Col>

            <Text class="muted">{todos.len()} items</Text>
        </Col>
    }
}


// ============================================================
// 3. 비동기 + 로딩 + 에러 — ServersPanel
// ============================================================

#[component]
fn ServersPanel() {
    let servers = state::<Vec<Server>>(vec![]);
    let loading = state(false);
    let error   = state::<Option<String>>(None);

    let refresh = move || async move {
        loading.set(true);
        error.set(None);
        match fetch_servers().await {
            Ok(v)  => servers.set(v),
            Err(e) => error.set(Some(e.to_string())),
        }
        loading.set(false);
    };

    ui! {
        <Col gap=8>
            <Row gap=4>
                <Text class="title">"Servers"</Text>
                <Button class="primary" disabled={loading} on_click={refresh}>
                    {if loading { "Loading..." } else { "Refresh" }}
                </Button>
            </Row>

            {if let Some(e) = &error {
                ui! { <Banner kind="error">{e}</Banner> }
            } else if loading {
                ui! { <Spinner /> }
            } else {
                ui! {
                    <Col gap=2>
                        {servers.iter().map(|s| ui! { <ServerRow server={s} /> })}
                    </Col>
                }
            }}
        </Col>
    }
}


// ============================================================
// 4. 대시보드 조립 — Dashboard
// ============================================================

#[component]
fn Dashboard() {
    let tab = state(Tab::Servers);

    ui! {
        <Dock>
            <Tab id="servers" title="Servers" active={tab == Tab::Servers}
                 on_select={|| tab.set(Tab::Servers)}>
                <ServersPanel />
            </Tab>
            <Tab id="metrics" title="Metrics" active={tab == Tab::Metrics}
                 on_select={|| tab.set(Tab::Metrics)}>
                <MetricsPanel />
            </Tab>
            <Tab id="logs" title="Logs" active={tab == Tab::Logs}
                 on_select={|| tab.set(Tab::Logs)}>
                <LogsPanel />
            </Tab>
        </Dock>
    }
}


// ============================================================
// 5. 최상위 앱 — main
// ============================================================

fn main() {
    run(|| ui! {
        <Theme preset="dark">
            <Dashboard />
        </Theme>
    });
}


// ============================================================
// 6. 스타일 / 테마 — Tailwind-like
// ============================================================

theme! {
    "title"     => text(24).bold().color(text),
    "muted"     => text(12).color(text_dim),
    "primary"   => bg(primary).fg(on_primary).pad(8, 16).radius(6),
    "ghost"     => bg(transparent).border(1).radius(6),
    "icon"      => size(24).center(),
    "item"      => pad(4, 8).radius(4),
    "item.done" => strike().color(text_dim),
    "error"     => bg(red_dim).fg(red).border(1),
}


// ============================================================
// 7. 테스트 — UI 없이, egui 없이
// ============================================================

#[test]
fn counter_inc() {
    let app = mount!(Counter);
    app.click("+");
    app.expect_text("Count: 1");
}

#[test]
fn todo_add() {
    let app = mount!(TodoList);
    app.type_into("input", "buy milk");
    app.click("Add");
    app.expect_text("buy milk");
    app.expect_text("1 items");
}

#[test]
fn todo_toggle_removes_strike() {
    let app = mount!(TodoList, initial = [Todo { id: 1, text: "a", done: false }]);
    app.click("Check");
    app.expect_class("item", "done");
}

#[test]
fn servers_loading_then_loaded() {
    let app = mount!(ServersPanel, api = FakeApi::with_servers(3));
    app.click("Refresh");
    app.expect_text("Loading...");
    app.flush();
    app.expect_count("ServerRow", 3);
}

#[test]
fn servers_error_banner() {
    let app = mount!(ServersPanel, api = FakeApi::failing("timeout"));
    app.click("Refresh");
    app.flush();
    app.expect_text("timeout");
    app.expect_class("Banner", "error");
}

#[test]
fn dashboard_tab_switch() {
    let app = mount!(Dashboard);
    app.click("Metrics");
    app.expect_visible("MetricsPanel");
    app.expect_hidden("ServersPanel");
}


// ============================================================
// 8. 같은 Counter를 순수 egui로 쓰면 — 비교
// ============================================================

// 순수 egui:
//   struct Counter { count: i32 }
//   impl Counter {
//       fn ui(&mut self, ui: &mut egui::Ui) {
//           ui.vertical(|ui| {
//               ui.spacing_mut().item_spacing.y = 8.0;
//               ui.label(egui::RichText::new(format!("Count: {}", self.count))
//                   .size(24.0).strong());
//               ui.horizontal(|ui| {
//                   ui.spacing_mut().item_spacing.x = 4.0;
//                   if ui.add(egui::Button::new("+")
//                       .fill(egui::Color32::from_rgb(60,120,220)))
//                       .clicked() { self.count += 1; }
//                   if ui.add(egui::Button::new("-")).clicked() { self.count -= 1; }
//               });
//           });
//       }
//   }
//
// 우리 라이브러리:
//   #[component]
//   fn Counter(start = 0) {
//       let count = state(start);
//       ui! {
//           <Col gap=8>
//               <Text class="title">"Count: {count}"</Text>
//               <Row gap=4>
//                   <Button class="primary" on_click={|| count += 1}>"+"</Button>
//                   <Button class="ghost"   on_click={|| count -= 1}>"-"</Button>
//               </Row>
//           </Col>
//       }
//   }


// ============================================================
// 9. 어떤 모던 러스트 문법을 썼나
// ------------------------------------------------------------
// - proc macro `#[component]`  → impl App 자동 생성
// - proc macro `ui!`           → JSX-like DSL → Element 트리
// - proc macro `theme!`        → 클래스 → StyleId 인터닝
// - proc macro `mount!`        → 테스트용 인스턴스 트리
// - 제네릭/라이프타임/트레잇 바운드는 매크로 뒤로 숨김
// - enum variant를 그대로 이벤트로 (클로저 트레잇 노출 없음)
// - async 블록을 `on_click`에 그대로 (Pin/Box/Send 노출 없음)
// - `if let` / `else if` / `match`가 ui! 안에서 표현식으로
// - 이터레이터 `.map()`이 ui! 블록을 반환 → 자식 리스트
// - `state()`가 Deref/DerefMut + `.set()` + `+=` 지원
// - 문자열 리터럴 `"{count}"`는 매크로가 format!으로 전개
// ============================================================


// ============================================================
// 10. 고성능은 어디서 오나
// ------------------------------------------------------------
// - Element는 enum. 힙에 Box<dyn> 없음 → 정적 디스패치
// - Msg도 enum → match가 점프 테이블로 컴파일
// - Element 트리는 bump arena에 매 프레임 할당, 끝나면 리셋
// - 클래스 문자열 → StyleId(u32) 인터닝, 렌더 시 정수 비교
// - state()는 슬롯 인덱스 접근. Rc<RefCell> 아님
// - async future는 상태 머신으로 단형화, 힙 할당은 Box::pin 1회
// - 자식 컴포넌트는 keyed 트리, 안 쓰는 키는 unmount
// - 모든 것이 모노모피제이션 → 런타임 타입 소거 없음
// ============================================================
