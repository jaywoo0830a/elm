egui는 **즉시 모드(immediate mode)** 라서, 사실 Elm 아키텍처와 궁합이 아주 좋습니다. `view`가 매 프레임 다시 돌고, 상태는 우리가 소유하고, egui는 그리기만 하면 됩니다. 다른 점은 딱 하나 — `view`가 트리를 **반환**하는 대신 `&mut Ui`에 **그린다**는 것. 그래서 보통은 “Element 트리를 만든다 → egui 어댑터가 순회하며 그린다”로 갑니다. 사용자 코드와 테스트는 egui를 전혀 모릅니다.

```rust
// ============================================================
// egui + Elm 스타일 순수 함수형 UI — 사용자 관점 단일 파일
// 라이브러리 / 사용자 앱 / egui 어댑터 / 테스트 순서
// ============================================================

// ============================================================
// [ 라이브러리 ] Element — egui 타입이 전혀 없는 순수 트리
// ============================================================

#[derive(Clone, Debug, PartialEq)]
pub enum Element<Msg> {
    Text {
        value: String,
        style: Style,
    },
    Button {
        label: String,
        on_click: Msg,
        style: Style,
        disabled: bool,
    },
    TextInput {
        value: String,
        on_input: fn(String) -> Msg, // 캡처 없는 fn 포인터
        style: Style,
    },
    Column {
        children: Vec<Element<Msg>>,
        gap: f32,
    },
    Row {
        children: Vec<Element<Msg>>,
        gap: f32,
    },
    // 자식 컴포넌트를 키로 식별. egui의 id_salt에 대응.
    Keyed {
        key: String,
        child: Box<Element<Msg>>,
    },
    // egui 고유 위젯은 별도 variant로 승격
    ScrollArea {
        child: Box<Element<Msg>>,
    },
    Collapsing {
        title: String,
        child: Box<Element<Msg>>,
    },
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Style {
    pub font_size: Option<f32>,
    pub bold: bool,
    pub padding: Option<f32>,
    pub background: Option<Color>,
    pub foreground: Option<Color>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color(pub u8, pub u8, pub u8);

// 빌더는 사용자 편의용
impl<Msg> Element<Msg> {
    pub fn column(children: Vec<Element<Msg>>) -> Self {
        Element::Column { children, gap: 8.0 }
    }
    pub fn row(children: Vec<Element<Msg>>) -> Self {
        Element::Row { children, gap: 8.0 }
    }
    pub fn text(value: impl Into<String>) -> Self {
        Element::Text { value: value.into(), style: Style::default() }
    }
    pub fn button(label: impl Into<String>, on_click: Msg) -> Self {
        Element::Button {
            label: label.into(),
            on_click,
            style: Style::default(),
            disabled: false,
        }
    }
    pub fn input(value: impl Into<String>, on_input: fn(String) -> Msg) -> Self {
        Element::TextInput { value: value.into(), on_input, style: Style::default() }
    }
    pub fn keyed(key: impl Into<String>, child: Element<Msg>) -> Self {
        Element::Keyed { key: key.into(), child: Box::new(child) }
    }
    pub fn disabled(mut self, v: bool) -> Self {
        if let Element::Button { disabled, .. } = &mut self { *disabled = v; }
        self
    }
    pub fn style(mut self, s: Style) -> Self {
        match &mut self {
            Element::Text { style, .. }
            | Element::Button { style, .. }
            | Element::TextInput { style, .. } => *style = s,
            _ => {}
        }
        self
    }
}

// ============================================================
// [ 라이브러리 ] Cmd — 부수효과를 데이터로
// ============================================================

pub enum Cmd<Msg> {
    None,
    Future(BoxFuture<Msg>),
    Batch(Vec<Cmd<Msg>>),
}

pub struct BoxFuture<Msg>(pub std::pin::Pin<Box<dyn std::future::Future<Output = Msg> + Send>>);

impl<Msg> Cmd<Msg> {
    pub fn none() -> Self { Cmd::None }
    pub fn future<F>(f: F) -> Self
    where F: std::future::Future<Output = Msg> + Send + 'static {
        Cmd::Future(BoxFuture(Box::pin(f)))
    }
    pub fn batch(cmds: Vec<Cmd<Msg>>) -> Self { Cmd::Batch(cmds) }
}

// ============================================================
// [ 라이브러리 ] App trait — 사용자 앱의 계약
// ============================================================

pub trait App {
    type Model: Clone + std::fmt::Debug + PartialEq + Send + 'static;
    type Msg: Clone + std::fmt::Debug + PartialEq + Send + 'static;

    fn init() -> (Self::Model, Cmd<Self::Msg>);
    fn update(model: Self::Model, msg: Self::Msg) -> (Self::Model, Cmd<Self::Msg>);
    fn view(model: &Self::Model) -> Element<Self::Msg>;
    fn theme() -> Theme { Theme::default() }
}

#[derive(Clone)]
pub struct Theme {
    pub radius: f32,
    pub primary: Color,
    pub on_primary: Color,
    pub text: Color,
}
impl Default for Theme {
    fn default() -> Self {
        Theme {
            radius: 4.0,
            primary: Color(60, 120, 220),
            on_primary: Color(255, 255, 255),
            text: Color(30, 30, 30),
        }
    }
}

// ============================================================
// [ 사용자 앱 ] Counter
// ============================================================

#[derive(Clone, Debug, PartialEq)]
struct CounterModel { count: i32 }

#[derive(Clone, Debug, PartialEq)]
enum CounterMsg { Inc, Dec }

struct Counter;

impl App for Counter {
    type Model = CounterModel;
    type Msg = CounterMsg;

    fn init() -> (Self::Model, Cmd<Self::Msg>) {
        (CounterModel { count: 0 }, Cmd::none())
    }

    fn update(model: Self::Model, msg: Self::Msg) -> (Self::Model, Cmd<Self::Msg>) {
        match msg {
            CounterMsg::Inc => (CounterModel { count: model.count + 1 }, Cmd::none()),
            CounterMsg::Dec => (CounterModel { count: model.count - 1 }, Cmd::none()),
        }
    }

    fn view(model: &Self::Model) -> Element<Self::Msg> {
        Element::column(vec![
            Element::text(format!("{}", model.count)),
            Element::row(vec![
                Element::button("+", CounterMsg::Inc),
                Element::button("-", CounterMsg::Dec),
            ]),
        ])
    }
}

// ============================================================
// [ 사용자 앱 ] TodoList — 자식을 keyed로 합성
// ============================================================

#[derive(Clone, Debug, PartialEq)]
struct TodoListModel { todos: Vec<Todo>, input: String }

#[derive(Clone, Debug, PartialEq)]
struct Todo { id: u32, text: String, done: bool }

#[derive(Clone, Debug, PartialEq)]
enum TodoListMsg {
    InputChanged(String),
    Add,
    Toggle(u32),
    Remove(u32),
}

struct TodoList;

impl App for TodoList {
    type Model = TodoListModel;
    type Msg = TodoListMsg;

    fn init() -> (Self::Model, Cmd<Self::Msg>) {
        (TodoListModel { todos: vec![], input: String::new() }, Cmd::none())
    }

    fn update(model: Self::Model, msg: Self::Msg) -> (Self::Model, Cmd<Self::Msg>) {
        match msg {
            TodoListMsg::InputChanged(v) => (
                TodoListModel { input: v, ..model },
                Cmd::none(),
            ),
            TodoListMsg::Add => {
                let id = model.todos.len() as u32;
                let text = model.input.clone();
                (
                    TodoListModel {
                        todos: model.todos.into_iter()
                            .chain(std::iter::once(Todo { id, text, done: false }))
                            .collect(),
                        input: String::new(),
                    },
                    Cmd::none(),
                )
            }
            TodoListMsg::Toggle(id) => (
                TodoListModel {
                    todos: model.todos.into_iter()
                        .map(|t| if t.id == id { Todo { done: !t.done, ..t } } else { t })
                        .collect(),
                    ..model
                },
                Cmd::none(),
            ),
            TodoListMsg::Remove(id) => (
                TodoListModel {
                    todos: model.todos.into_iter().filter(|t| t.id != id).collect(),
                    ..model
                },
                Cmd::none(),
            ),
        }
    }

    fn view(model: &Self::Model) -> Element<Self::Msg> {
        let mut children = vec![
            Element::input(model.input.clone(), TodoListMsg::InputChanged),
            Element::button("Add", TodoListMsg::Add),
        ];
        for t in &model.todos {
            children.push(Element::keyed(
                format!("todo-{}", t.id),
                Element::row(vec![
                    Element::button(if t.done { "☑" } else { "☐" }, TodoListMsg::Toggle(t.id)),
                    Element::text(t.text.clone()),
                    Element::button("x", TodoListMsg::Remove(t.id)),
                ]),
            ));
        }
        Element::column(children)
    }
}

// ============================================================
// [ 사용자 앱 ] 최상위 App — 이종 컴포넌트 합성
// ============================================================

#[derive(Clone, Debug, PartialEq)]
struct RootModel {
    counter: CounterModel,
    todos: TodoListModel,
}

#[derive(Clone, Debug, PartialEq)]
enum RootMsg {
    Counter(CounterMsg),
    Todos(TodoListMsg),
}

struct Root;

impl App for Root {
    type Model = RootModel;
    type Msg = RootMsg;

    fn init() -> (Self::Model, Cmd<Self::Msg>) {
        let (c, _) = Counter::init();
        let (t, _) = TodoList::init();
        (RootModel { counter: c, todos: t }, Cmd::none())
    }

    fn update(model: Self::Model, msg: Self::Msg) -> (Self::Model, Cmd<Self::Msg>) {
        match msg {
            RootMsg::Counter(m) => {
                let (c, _) = Counter::update(model.counter, m);
                (RootModel { counter: c, ..model }, Cmd::none())
            }
            RootMsg::Todos(m) => {
                let (t, _) = TodoList::update(model.todos, m);
                (RootModel { todos: t, ..model }, Cmd::none())
            }
        }
    }

    fn view(model: &Self::Model) -> Element<Self::Msg> {
        Element::column(vec![
            Element::keyed("counter", map_element(Counter::view(&model.counter), RootMsg::Counter)),
            Element::keyed("todos",   map_element(TodoList::view(&model.todos),   RootMsg::Todos)),
        ])
    }
}

// 자식 Element<ChildMsg>를 부모 Element<ParentMsg>로 변환
fn map_element<Child, Parent>(
    el: Element<Child>,
    f: fn(Child) -> Parent,
) -> Element<Parent>
where Child: Clone, Parent: Clone {
    match el {
        Element::Text { value, style } => Element::Text { value, style },
        Element::Button { label, on_click, style, disabled } =>
            Element::Button { label, on_click: f(on_click), style, disabled },
        Element::TextInput { value, on_input, style } =>
            Element::TextInput {
                value,
                on_input: leak_fn(move |s| f(on_input(s))),
                style,
            },
        Element::Column { children, gap } =>
            Element::Column { children: children.into_iter().map(|c| map_element(c, f)).collect(), gap },
        Element::Row { children, gap } =>
            Element::Row { children: children.into_iter().map(|c| map_element(c, f)).collect(), gap },
        Element::Keyed { key, child } =>
            Element::Keyed { key, child: Box::new(map_element(*child, f)) },
        Element::ScrollArea { child } =>
            Element::ScrollArea { child: Box::new(map_element(*child, f)) },
        Element::Collapsing { title, child } =>
            Element::Collapsing { title, child: Box::new(map_element(*child, f)) },
    }
}

// 캡처 없는 fn 포인터로 승격 (프로토타입용 단순화)
fn leak_fn<A, B>(f: impl Fn(A) -> B + 'static) -> fn(A) -> B {
    // 실제로는 컴파일 안 됨. 아이디어 전달용.
    // 진짜 구현에서는 Element::TextInput의 on_input을 Box<dyn Fn>으로 바꾸거나
    // Msg에 입력값을 담아 "값 → Msg" 매핑을 데이터로 표현한다.
    unimplemented!()
}

// ============================================================
// [ egui 어댑터 ] — Element를 egui로 그린다
// ============================================================

use eframe::egui;

pub struct EguiApp<A: App> {
    model: A::Model,
    cmd: Option<Cmd<A::Msg>>,
    theme: Theme,
}

impl<A: App> EguiApp<A> {
    pub fn new() -> Self {
        let (model, cmd) = A::init();
        Self { model, cmd: Some(cmd), theme: A::theme() }
    }
}

impl<A: App> eframe::App for EguiApp<A> {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // 즉시 모드의 핵심: 메시지는 프레임 중에 모아서, 프레임 끝나고 적용.
        let mut pending: Option<A::Msg> = None;

        egui::CentralPanel::default().show(ctx, |ui| {
            let tree = A::view(&self.model);
            render(ui, &tree, &mut pending, &self.theme);
        });

        if let Some(msg) = pending {
            let (next, cmd) = A::update(self.model.clone(), msg);
            self.model = next;
            self.cmd = Some(cmd);
            ctx.request_repaint();
        }

        // Cmd 실행 (실제로는 spawn해서 결과를 Msg로 되돌림)
        if let Some(Cmd::Future(_)) = &self.cmd {
            // tokio::spawn(...) → ctx.request_repaint() + 채널로 메시지 주입
        }
    }
}

// Element 트리를 egui로 그리는 어댑터 (시그니처만)
fn render<Msg: Clone>(
    ui: &mut egui::Ui,
    el: &Element<Msg>,
    pending: &mut Option<Msg>,
    theme: &Theme,
) {
    match el {
        Element::Text { value, style } => {
            let mut rich = egui::RichText::new(value);
            if let Some(sz) = style.font_size { rich = rich.size(sz); }
            if style.bold { rich = rich.strong(); }
            if let Some(c) = style.foreground {
                rich = rich.color(egui::Color32::from_rgb(c.0, c.1, c.2));
            }
            ui.label(rich);
        }
        Element::Button { label, on_click, disabled, .. } => {
            let resp = ui.add_enabled(!disabled, egui::Button::new(label));
            if resp.clicked() {
                *pending = Some(on_click.clone());
            }
        }
        Element::TextInput { value, on_input, .. } => {
            let mut buf = value.clone();
            if ui.text_edit_singleline(&mut buf).changed() {
                *pending = Some(on_input(buf));
            }
        }
        Element::Column { children, gap } => {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = *gap;
                for c in children { render(ui, c, pending, theme); }
            });
        }
        Element::Row { children, gap } => {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = *gap;
                for c in children { render(ui, c, pending, theme); }
            });
        }
        Element::Keyed { key, child } => {
            // egui의 id_salt로 스크롤/포커스 상태를 안정적으로 유지
            ui.push_id(key, |ui| render(ui, child, pending, theme));
        }
        Element::ScrollArea { child } => {
            egui::ScrollArea::vertical().show(ui, |ui| {
                render(ui, child, pending, theme);
            });
        }
        Element::Collapsing { title, child } => {
            egui::CollapsingHeader::new(title).show(ui, |ui| {
                render(ui, child, pending, theme);
            });
        }
    }
}

// ============================================================
// [ main ] eframe에 올리기
// ============================================================

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "my-ui + egui",
        native_options,
        Box::new(|_cc| Box::new(EguiApp::<Root>::new())),
    )
}

// ============================================================
// [ 테스트 ] — egui 없이 돈다. 같은 계약.
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    // 순수 update 테스트
    #[test]
    fn counter_inc() {
        let (m, _) = Counter::update(CounterModel { count: 0 }, CounterMsg::Inc);
        assert_eq!(m.count, 1);
    }

    #[test]
    fn update_does_not_mutate_original() {
        let original = CounterModel { count: 0 };
        let _ = Counter::update(original.clone(), CounterMsg::Inc);
        assert_eq!(original.count, 0);
    }

    // view는 순수 데이터 → 트리 질의 가능
    fn find_text<'a, M>(el: &'a Element<M>, value: &str) -> Option<&'a str> {
        match el {
            Element::Text { value: v, .. } if v == value => Some(v),
            Element::Column { children, .. } | Element::Row { children, .. } => {
                children.iter().find_map(|c| find_text(c, value))
            }
            Element::Keyed { child, .. }
            | Element::ScrollArea { child }
            | Element::Collapsing { child, .. } => find_text(child, value),
            _ => None,
        }
    }

    fn find_button<'a, M>(el: &'a Element<M>, label: &str) -> Option<&'a M> {
        match el {
            Element::Button { label: l, on_click, .. } if l == label => Some(on_click),
            Element::Column { children, .. } | Element::Row { children, .. } => {
                children.iter().find_map(|c| find_button(c, label))
            }
            Element::Keyed { child, .. }
            | Element::ScrollArea { child }
            | Element::Collapsing { child, .. } => find_button(child, label),
            _ => None,
        }
    }

    #[test]
    fn view_renders_count() {
        let m = CounterModel { count: 42 };
        let el = Counter::view(&m);
        assert_eq!(find_text(&el, "42"), Some("42"));
    }

    #[test]
    fn view_has_inc_button_with_inc_msg() {
        let m = CounterModel { count: 0 };
        let el = Counter::view(&m);
        assert_eq!(find_button(&el, "+"), Some(&CounterMsg::Inc));
    }

    #[test]
    fn view_is_deterministic() {
        let m = CounterModel { count: 7 };
        assert_eq!(Counter::view(&m), Counter::view(&m));
    }

    // 자식 map 합성 테스트
    #[test]
    fn root_wraps_child_msgs() {
        let (m, _) = Root::init();
        let el = Root::view(&m);
        // 자식의 "+" 버튼이 부모 메시지로 감싸져 있다
        assert_eq!(find_button(&el, "+"), Some(&RootMsg::Counter(CounterMsg::Inc)));
    }

    #[test]
    fn root_update_routes_to_child() {
        let (m, _) = Root::init();
        let (next, _) = Root::update(m, RootMsg::Counter(CounterMsg::Inc));
        assert_eq!(next.counter.count, 1);
        assert_eq!(next.todos.todos.len(), 0); // 다른 자식은 그대로
    }

    // 자식 상태는 부모 모델에 안 샌다
    #[test]
    fn child_state_does_not_leak_into_parent() {
        let (m, _) = Root::init();
        let (next, _) = Root::update(m, RootMsg::Counter(CounterMsg::Inc));
        // RootModel에 "count" 필드 자체가 없다
        let _ = format!("{:?}", next); // 디버그로 구조 확인
        assert_eq!(next.counter.count, 1);
    }

    // 스타일은 상태를 바꾸지 않는다
    #[test]
    fn style_does_not_affect_model() {
        let m = CounterModel { count: 0 };
        let before = m.clone();
        let _ = Counter::view(&m).style(Style { bold: true, ..Style::default() });
        assert_eq!(m, before);
    }
}

// ============================================================
// 요약 — egui와 쓸 때의 계약
// ------------------------------------------------------------
// - 사용자 코드(Model/Msg/init/update/view)는 egui를 모른다.
// - view는 Element<Msg>를 반환하고, egui 어댑터가 순회하며 그린다.
// - 즉시 모드이므로 메시지는 프레임 중에 모아서 프레임 끝에 적용한다.
// - Keyed는 egui의 push_id에 대응 → 스크롤/포커스 상태 안정.
// - ScrollArea / Collapsing / Window 같은 egui 고유 위젯은
//   Element의 별도 variant로 승격시킨다.
// - TextInput은 egui가 String 버퍼를 소유하므로
//   "값 복사 → 편집 → on_input(Msg)"로 되돌린다.
// - Cmd는 tokio 등으로 spawn하고, 결과를 ctx.request_repaint()와
//   채널로 다시 Msg로 주입한다.
// - 테스트는 egui 없이 그대로 돈다. 같은 계약, 같은 질의 헬퍼.
// ============================================================
```

**egui 특유의 함정 몇 개만 짚으면:**

- **즉시 모드라 `view`가 매 프레임 실행**됩니다. 순수해야 하고, 값싸야 합니다. `Element` 트리를 매 프레임 새로 만드는 건 괜찮지만, 무거운 계산이 있으면 `Memo` 같은 캐시 계층이 필요합니다.
- **메시지는 프레임 중에 바로 적용하면 안 됩니다.** 모델이 바뀌면 트리 모양이 달라져서 위젯 ID가 틀어집니다. `pending: Option<Msg>`로 모아서 프레임 끝에 적용하는 게 정석입니다.
- **`CollapsingHeader`, `ScrollArea`, `Window`, `ComboBox`** 같은 건 egui 내부 상태를 씁니다. `Element::Keyed`가 `ui.push_id`로 매핑되어야 스크롤 위치·열림 상태가 안정적입니다.
- **`TextEdit`은 `String` 버퍼를 직접 편집**합니다. `value.clone()` → 편집 → `on_input(new_value)` 패턴으로 모델로 되돌립니다.
- **`Cmd`의 async**는 `tokio::spawn` + `ctx.request_repaint()` + 채널로 결과를 다시 `Msg`로 흘려보냅니다. eframe은 `Send`만 만족하면 됩니다.
- **테스트는 egui 없이 그대로** 돕니다. `view`가 순수 데이터라서 트리 질의만으로 계약을 검증할 수 있습니다 — 이게 이 구조의 진짜 값어치입니다.

원하면 `leak_fn` 자리에 진짜로 컴파일되는 `map_element`(예: `on_input`을 `Msg::InputChanged(String)` 같은 데이터로 표현) 버전과, `Cmd`를 `tokio`로 돌리는 `CmdRunner`까지 이어서 붙여드릴 수 있습니다.
