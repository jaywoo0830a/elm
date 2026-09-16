// ============================================================
// my-ui — 합성 버전 전체 (구현 + 컴포넌트 + 테스트) 단일 파일
// ------------------------------------------------------------
// 라이브러리 / 컴포넌트 / 테스트 순서.
// 테스트가 계약을 드러내고, 구현은 그 계약을 만족시킨다.
// ============================================================

import { describe, expect, it, vi } from 'vitest';

// ============================================================
// [ 라이브러리 ] Element — 플랫폼 중립 설명 트리
// ============================================================
const Element = {
  text: (value, opts = {}) => ({
    kind: 'text',
    value: String(value),
    style: opts.style ?? null,
    testId: opts.testId ?? null,
  }),
  box: (tag, opts = {}) => ({
    kind: 'box',
    tag,
    style: opts.style ?? null,
    children: opts.children ?? [],
    on: opts.on ?? {},
    disabled: opts.disabled ?? false,
    attrs: opts.attrs ?? {},
    testId: opts.testId ?? null,
  }),
  column: (opts = {}) => Element.box('column', opts),
  row:    (opts = {}) => Element.box('row', opts),
  button: (label, opts = {}) =>
    Element.box('button', { ...opts, attrs: { ...opts.attrs, label } }),
  input:  (value, opts = {}) =>
    Element.box('input', { ...opts, attrs: { ...opts.attrs, value } }),
  child:  (Comp, opts = {}) => ({
    kind: 'child',
    Comp,
    key: opts.key ?? null,
    props: opts.props ?? {},
    map: opts.map ?? ((m) => m),
  }),
};

// ============================================================
// [ 라이브러리 ] Cmd — 부수효과를 데이터로
// ============================================================
const Cmd = {
  none: () => ({ type: 'none' }),
  future: (thunk) => ({ type: 'future', thunk }),
  batch: (cmds) =>
    ({ type: 'batch', cmds: cmds.filter((c) => c && c.type !== 'none') }),
};

// ============================================================
// [ 라이브러리 ] Instance — 컴포넌트 인스턴스 트리
// ============================================================
class Instance {
  constructor(Comp, props, parent = null, map = (m) => m, key = null) {
    this.Comp = Comp;
    this.props = props;
    this.parent = parent;
    this.map = map;
    this.key = key;
    this.children = new Map();
    this.pending = [];
    this.messagesSeen = [];
    this.lastMsgSeen = null;
    this._warnings = [];

    const [model, cmd] = Comp.init(props);
    this.model = model;
    if (cmd && cmd.type !== 'none') this.pending.push(cmd);
  }

  // 자기 자신의 update 실행. 부모에게 map해서 올린다.
  dispatch(msg) {
    this.lastMsgSeen = msg;
    this.messagesSeen.push(msg);
    const [next, cmd] = this.Comp.update(this.props, this.model, msg);
    this.model = next;
    if (cmd && cmd.type !== 'none') this.pending.push(cmd);

    if (this.parent) {
      const mapped = this.map(msg);
      this.parent._receiveMapped(mapped);
    }
    return this.model;
  }

  // 자식으로부터 map된 메시지를 받는다.
  _receiveMapped(mapped) {
    this.lastMsgSeen = mapped;
    this.messagesSeen.push(mapped);
    const [next, cmd] = this.Comp.update(this.props, this.model, mapped);
    this.model = next;
    if (cmd && cmd.type !== 'none') this.pending.push(cmd);

    if (this.parent) {
      const outerMapped = this.map(mapped);
      this.parent._receiveMapped(outerMapped);
    }
  }

  // 매 렌더마다: 안 쓰는 자식은 버리고, 쓰는 자식은 재사용/새로 mount.
  view() {
    const raw = this.Comp.view(this.props, this.model, (msg) => this.dispatch(msg));

    const usedKeys = new Set();
    const collect = (n) => {
      if (!n || typeof n !== 'object') return;
      if (n.kind === 'child') usedKeys.add(n.key);
      if (n.kind === 'box') n.children.forEach(collect);
    };
    collect(raw);

    for (const k of [...this.children.keys()]) {
      if (!usedKeys.has(k)) this.children.delete(k); // unmount → 상태 초기화
    }

    return this._resolveChildren(raw);
  }

  _resolveChildren(node) {
    if (!node || typeof node !== 'object') return node;

    if (node.kind === 'child') {
      const childKey = node.key;
      let child = this.children.get(childKey);
      if (!child || child.Comp !== node.Comp) {
        child = new Instance(node.Comp, node.props, this, node.map, childKey);
        this.children.set(childKey, child);
      } else {
        child.props = node.props;
        child.map = node.map;
      }
      return { kind: 'child', key: childKey, view: child.view() };
    }

    if (node.kind === 'box') {
      const seen = new Set();
      const children = node.children.map((c) => {
        if (c && c.kind === 'child' && c.key != null) {
          if (seen.has(c.key)) {
            const w = `duplicate key: ${c.key}`;
            this._warnings.push(w);
            // eslint-disable-next-line no-console
            console.warn(w);
          }
          seen.add(c.key);
        }
        return this._resolveChildren(c);
      });
      return { ...node, children };
    }

    return node;
  }

  focus(key) {
    this.view(); // children 확보
    const parts = String(key).split('/');
    let cur = this;
    for (const p of parts) {
      const found = cur.children.get(p);
      if (!found) throw new Error(`focus: no child with key '${p}'`);
      cur = found;
    }
    return cur;
  }

  pendingCmds() {
    const out = [...this.pending];
    for (const c of this.children.values()) out.push(...c.pendingCmds());
    return out;
  }

  async flush() {
    const cmds = this.pending.splice(0);
    for (const cmd of cmds) {
      if (cmd.type === 'future') {
        const msg = await cmd.thunk();
        if (msg) this.dispatch(msg);
      } else if (cmd.type === 'batch') {
        for (const c of cmd.cmds) {
          if (c.type === 'future') {
            const msg = await c.thunk();
            if (msg) this.dispatch(msg);
          }
        }
      }
    }
    for (const child of this.children.values()) {
      await child.flush();
    }
  }

  warnings() { return [...this._warnings]; }
  messages() { return this.Comp.messages ?? []; }
}

// ============================================================
// [ 라이브러리 ] public API — mount / focus / flush / fire / query
// ============================================================
let _current = null;

function makeAppFromInstance(inst) {
  return {
    view:        ()  => inst.view(),
    model:       ()  => inst.model,
    dispatch:    (m) => inst.dispatch(m),
    lastMsg:     ()  => inst.lastMsgSeen,
    messages:    ()  => inst.messages(),
    focus:       (k) => makeAppFromInstance(inst.focus(k)),
    pendingCmds: ()  => inst.pendingCmds(),
    warnings:    ()  => inst.warnings(),
    flush:       ()  => inst.flush(),
  };
}

function mount(Comp, props = {}) {
  _current = makeAppFromInstance(new Instance(Comp, props));
  return _current;
}

async function flush() {
  if (_current) await _current.flush();
}

// ── 트리 순회 / 질의 ───────────────────────────────────────
function walk(node, fn) {
  if (!node || typeof node !== 'object') return;
  fn(node);
  if (node.kind === 'box') node.children.forEach((c) => walk(c, fn));
  else if (node.kind === 'child') walk(node.view, fn);
}

function findByTestId(tree, id) {
  let found = null;
  walk(tree, (n) => { if (!found && n.testId === id) found = n; });
  return found;
}

function findAllByTestId(tree, id) {
  const out = [];
  walk(tree, (n) => { if (n.testId === id) out.push(n); });
  return out;
}

function findByText(tree, text) {
  let found = null;
  walk(tree, (n) => { if (!found && n.kind === 'text' && n.value === text) found = n; });
  return found;
}

// ── 이벤트 발생 ───────────────────────────────────────────
function fire(element, data) {
  if (!element) throw new Error('fire: element not found');
  const on = element.on || {};
  if (on.click)  return on.click(data);
  if (on.input)  return on.input(data && data.value !== undefined ? data.value : data);
  if (on.change) return on.change(data && data.value !== undefined ? data.value : data);
  throw new Error('fire: element has no handler');
}

// ── 비교/클론용: 함수를 제거한 스냅샷 ───────────────────────
function snapshot(node) {
  if (typeof node === 'function') return '[fn]';
  if (Array.isArray(node)) return node.map(snapshot);
  if (node && typeof node === 'object') {
    const out = {};
    for (const k of Object.keys(node)) out[k] = snapshot(node[k]);
    return out;
  }
  return node;
}

// ============================================================
// [ 컴포넌트 ] Counter — 자기 상태를 가진 최소 컴포넌트
// ============================================================
const Counter = {
  init: (props) => [{ count: props.start ?? 0 }, Cmd.none()],
  update: (props, model, msg) => {
    switch (msg.type) {
      case 'Inc':  return [{ ...model, count: model.count + 1 }, Cmd.none()];
      case 'Dec':  return [{ ...model, count: model.count - 1 }, Cmd.none()];
      case 'Sync': return [model, Cmd.future(async () => ({ type: 'Noop' }))];
      default:     return [model, Cmd.none()];
    }
  },
  view: (props, model, dispatch) =>
    Element.column({
      testId: 'counter',
      children: [
        Element.text(String(model.count)),
        Element.button('+', { testId: 'inc', on: { click: () => dispatch({ type: 'Inc' }) } }),
        Element.button('-', { testId: 'dec', on: { click: () => dispatch({ type: 'Dec' }) } }),
      ],
    }),
  messages: [{ type: 'Inc' }, { type: 'Dec' }],
};

// ============================================================
// [ 컴포넌트 ] CounterWithNotify — 이벤트를 위로 올린다
// ============================================================
const CounterWithNotify = {
  init: (props) => [{ count: props.start ?? 0 }, Cmd.none()],
  update: (props, model, msg) => {
    switch (msg.type) {
      case 'Inc': {
        const next = { ...model, count: model.count + 1 };
        props.onChange?.(next.count); // 프로토타입: 직접 호출
        return [next, Cmd.none()];
      }
      case 'Dec': {
        const next = { ...model, count: model.count - 1 };
        props.onChange?.(next.count);
        return [next, Cmd.none()];
      }
      default: return [model, Cmd.none()];
    }
  },
  view: (props, model, dispatch) =>
    Element.column({
      children: [
        Element.text(String(model.count)),
        Element.button('+', { testId: 'inc', on: { click: () => dispatch({ type: 'Inc' }) } }),
        Element.button('-', { testId: 'dec', on: { click: () => dispatch({ type: 'Dec' }) } }),
      ],
    }),
};

// ============================================================
// [ 컴포넌트 ] TodoItem — 상태 없는 뷰, 메시지만 낸다
// ============================================================
const TodoItem = {
  init: () => [null, Cmd.none()],
  update: (props, model, msg) => [model, Cmd.none()],
  view: (props, model, dispatch) =>
    Element.row({
      testId: 'todo-item',
      style: { textDecoration: props.done ? 'line-through' : 'none' },
      children: [
        Element.box('checkbox', {
          testId: 'toggle',
          attrs: { checked: !!props.done },
          on: { click: () => dispatch({ type: 'Toggle', id: props.id }) },
        }),
        Element.text(props.text),
        Element.button('x', {
          testId: 'remove',
          on: { click: () => dispatch({ type: 'Remove', id: props.id }) },
        }),
      ],
    }),
};

// ============================================================
// [ 컴포넌트 ] TodoList — 자식을 map으로 합성
// ============================================================
const TodoList = {
  init: (props) => [
    {
      todos: (props.initial ?? []).map((t, i) => ({
        id: t.id ?? i, text: t.text, done: !!t.done,
      })),
      input: '',
      loading: false,
    },
    Cmd.none(),
  ],
  update: (props, model, msg) => {
    switch (msg.type) {
      case 'InputChange':
        return [{ ...model, input: msg.value }, Cmd.none()];

      case 'Add':
        return [
          {
            ...model,
            todos: [...model.todos, { id: model.todos.length, text: model.input, done: false }],
            input: '',
          },
          Cmd.none(),
        ];

      case 'Item': { // 자식이 map되어 올라온 메시지
        if (msg.msg.type === 'Toggle') {
          return [
            { ...model, todos: model.todos.map((t, i) => i === msg.index ? { ...t, done: !t.done } : t) },
            Cmd.none(),
          ];
        }
        if (msg.msg.type === 'Remove') {
          return [
            { ...model, todos: model.todos.filter((_, i) => i !== msg.index) },
            Cmd.none(),
          ];
        }
        return [model, Cmd.none()];
      }

      case 'Fetch':
        return [
          { ...model, loading: true },
          Cmd.future(async () => {
            const res = await props.api.load();
            return { type: 'Fetched', items: res.items };
          }),
        ];

      case 'Fetched':
        return [
          {
            ...model,
            loading: false,
            todos: msg.items.map((text, i) => ({ id: i, text, done: false })),
          },
          Cmd.none(),
        ];

      case 'Sync':
        return [model, Cmd.future(async () => ({ type: 'Noop' }))];

      default:
        return [model, Cmd.none()];
    }
  },
  view: (props, model, dispatch) =>
    Element.column({
      testId: 'todo-list',
      children: [
        Element.input(model.input, {
          testId: 'input',
          on: { input: (v) => dispatch({ type: 'InputChange', value: v }) },
        }),
        Element.button('Add',   { testId: 'add',        on: { click: () => dispatch({ type: 'Add' }) } }),
        Element.button('Fetch', { testId: 'todo-fetch', on: { click: () => dispatch({ type: 'Fetch' }) } }),
        ...model.todos.map((t, i) =>
          Element.child(TodoItem, {
            key: `todo-item:${i}`,
            props: { id: t.id, text: t.text, done: t.done },
            map: (childMsg) => ({ type: 'Item', index: i, msg: childMsg }),
          }),
        ),
      ],
    }),
};

// ============================================================
// [ 컴포넌트 ] App — 이종 컴포넌트 합성
// ============================================================
const App = {
  init: () => [{ theme: 'light', tab: 'both' }, Cmd.none()],
  update: (props, model, msg) => {
    switch (msg.type) {
      case 'ToggleTheme':
        return [{ ...model, theme: model.theme === 'light' ? 'dark' : 'light' }, Cmd.none()];
      case 'Tab':
        return [{ ...model, tab: msg.tab }, Cmd.none()];
      default:
        return [model, Cmd.none()];
    }
  },
  view: (props, model, dispatch) => {
    const children = [
      Element.button('theme',       { testId: 'toggle-theme', on: { click: () => dispatch({ type: 'ToggleTheme' }) } }),
      Element.button('tab-counter', { testId: 'tab-counter',  on: { click: () => dispatch({ type: 'Tab', tab: 'counter' }) } }),
      Element.button('tab-todos',   { testId: 'tab-todos',    on: { click: () => dispatch({ type: 'Tab', tab: 'todos' }) } }),
      Element.button('tab-both',    { testId: 'tab-both',     on: { click: () => dispatch({ type: 'Tab', tab: 'both' }) } }),
    ];

    if (model.tab === 'both' || model.tab === 'counter') {
      children.push(Element.child(Counter, {
        key: 'counter',
        props: { start: 0 },
        map: (m) => ({ type: 'Counter', msg: m }),
      }));
    }
    if (model.tab === 'both' || model.tab === 'todos') {
      children.push(Element.child(TodoList, {
        key: 'todo-list',
        props: { initial: [], api: props.api },
        map: (m) => ({ type: 'TodoList', msg: m }),
      }));
    }
    if (props.forceDuplicateKeys) {
      children.push(Element.child(Counter, { key: 'dup', props: { start: 0 }, map: (m) => m }));
      children.push(Element.child(Counter, { key: 'dup', props: { start: 0 }, map: (m) => m }));
    }

    return Element.column({ children });
  },
};

// ============================================================
// ============================================================
//                          테 스 트
// ============================================================
// ============================================================

// ------------------------------------------------------------
// 1. Counter — 단독
// ------------------------------------------------------------
describe('Counter — 단독', () => {
  it('props.start로 초기 상태를 정한다', () => {
    const app = mount(Counter, { start: 10 });
    expect(findByText(app.view(), '10')).toBeDefined();
  });

  it('+ 버튼을 누르면 자기 상태만 바뀐다', () => {
    const app = mount(Counter, { start: 0 });
    fire(findByTestId(app.view(), 'inc'));
    expect(findByText(app.view(), '1')).toBeDefined();
  });

  it('- 버튼을 누르면 1 줄어든다', () => {
    const app = mount(Counter, { start: 3 });
    fire(findByTestId(app.view(), 'dec'));
    expect(findByText(app.view(), '2')).toBeDefined();
  });

  it('자기 메시지는 자기 것만 안다', () => {
    const app = mount(Counter, { start: 0 });
    expect(app.messages()).toEqual(
      expect.arrayContaining([{ type: 'Inc' }, { type: 'Dec' }]),
    );
  });
});

// ------------------------------------------------------------
// 2. CounterWithNotify — 이벤트를 위로 올린다
// ------------------------------------------------------------
describe('CounterWithNotify — 이벤트를 위로 올린다', () => {
  it('변경될 때마다 onChange가 호출된다', () => {
    const onChange = vi.fn();
    const app = mount(CounterWithNotify, { start: 0, onChange });
    fire(findByTestId(app.view(), 'inc'));
    fire(findByTestId(app.view(), 'inc'));
    expect(onChange).toHaveBeenCalledTimes(2);
    expect(onChange).toHaveBeenLastCalledWith(2);
  });

  it('이벤트는 자기 상태가 아니라 콜백으로 나간다', () => {
    const onChange = vi.fn();
    const app = mount(CounterWithNotify, { start: 0, onChange });
    fire(findByTestId(app.view(), 'inc'));
    expect(app.model().count).toBe(1);
    expect(onChange).toHaveBeenCalledWith(1);
  });
});

// ------------------------------------------------------------
// 3. TodoItem — 상태 없는 뷰, 메시지만 낸다
// ------------------------------------------------------------
describe('TodoItem — 상태 없는 뷰', () => {
  it('텍스트를 그대로 보여준다', () => {
    const app = mount(TodoItem, { id: 1, text: '밥 먹기', done: false });
    expect(findByText(app.view(), '밥 먹기')).toBeDefined();
  });

  it('체크박스를 누르면 Toggle 메시지를 낸다', () => {
    const app = mount(TodoItem, { id: 7, text: 'x', done: false });
    fire(findByTestId(app.view(), 'toggle'));
    expect(app.lastMsg()).toEqual({ type: 'Toggle', id: 7 });
  });

  it('삭제 버튼은 Remove 메시지를 낸다', () => {
    const app = mount(TodoItem, { id: 7, text: 'x', done: false });
    fire(findByTestId(app.view(), 'remove'));
    expect(app.lastMsg()).toEqual({ type: 'Remove', id: 7 });
  });

  it('done 상태가 스타일에 반영된다', () => {
    const a = mount(TodoItem, { id: 1, text: 'x', done: false });
    const b = mount(TodoItem, { id: 1, text: 'x', done: true });
    expect(snapshot(a.view())).not.toEqual(snapshot(b.view()));
  });
});

// ------------------------------------------------------------
// 4. TodoList — 자식 map 합성
// ------------------------------------------------------------
describe('TodoList — 자식 map 합성', () => {
  const mountList = (todos = []) => mount(TodoList, { initial: todos });

  it('자식 개수만큼 아이템을 그린다', () => {
    const app = mountList([
      { id: 1, text: 'a', done: false },
      { id: 2, text: 'b', done: false },
    ]);
    expect(findAllByTestId(app.view(), 'todo-item')).toHaveLength(2);
  });

  it('입력 후 Add로 아이템이 추가된다', () => {
    const app = mountList([]);
    fire(findByTestId(app.view(), 'input'), { value: '우유 사기' });
    fire(findByTestId(app.view(), 'add'));
    expect(findAllByTestId(app.view(), 'todo-item')).toHaveLength(1);
  });

  it('자식이 낸 Toggle이 부모 상태를 바꾼다', () => {
    const app = mountList([{ id: 1, text: 'a', done: false }]);
    fire(findByTestId(app.view(), 'toggle'));
    expect(app.model().todos[0].done).toBe(true);
  });

  it('자식이 낸 Remove가 리스트에서 사라진다', () => {
    const app = mountList([
      { id: 1, text: 'a', done: false },
      { id: 2, text: 'b', done: false },
    ]);
    fire(findAllByTestId(app.view(), 'remove')[0]);
    expect(app.model().todos).toHaveLength(1);
    expect(app.model().todos[0].id).toBe(2);
  });

  it('부모가 자식 메시지를 map으로 감싼다', () => {
    const app = mountList([{ id: 1, text: 'a', done: false }]);
    const child = app.focus('todo-item:0');
    fire(findByTestId(child.view(), 'toggle'));
    expect(app.lastMsg()).toEqual({
      type: 'Item',
      index: 0,
      msg: { type: 'Toggle', id: 1 },
    });
  });
});

// ------------------------------------------------------------
// 5. App — 이종 컴포넌트 합성
// ------------------------------------------------------------
describe('App — 이종 컴포넌트 합성', () => {
  const mountApp = () => mount(App, { api: { load: async () => ({ items: [] }) } });

  it('Counter와 TodoList가 함께 그려진다', () => {
    const app = mountApp();
    expect(findByTestId(app.view(), 'counter')).toBeDefined();
    expect(findByTestId(app.view(), 'todo-list')).toBeDefined();
  });

  it('Counter만 조작해도 TodoList 상태는 그대로다', () => {
    const app = mountApp();
    const list = app.focus('todo-list');
    fire(findByTestId(list.view(), 'input'), { value: 'a' });
    fire(findByTestId(list.view(), 'add'));
    const before = JSON.parse(JSON.stringify(list.model()));

    fire(findByTestId(app.focus('counter').view(), 'inc'));
    fire(findByTestId(app.focus('counter').view(), 'inc'));

    expect(list.model()).toEqual(before);
  });

  it('자식 상태는 부모 모델에 새어들지 않는다', () => {
    const app = mountApp();
    fire(findByTestId(app.view(), 'inc'));
    fire(findByTestId(app.view(), 'inc'));
    expect(app.model()).not.toHaveProperty('count');
    expect(app.focus('counter').model().count).toBe(2);
  });

  it('자식 두 곳의 메시지가 서로 섞이지 않는다', () => {
    const app = mountApp();
    app.focus('counter').dispatch({ type: 'Inc' });
    app.focus('todo-list').dispatch({ type: 'Add' });
    expect(app.focus('counter').model().count).toBe(1);
    expect(app.focus('todo-list').model().todos).toHaveLength(1);
  });
});

// ------------------------------------------------------------
// 6. Cmd 합성 — 자식의 효과가 부모를 통해 흐른다
// ------------------------------------------------------------
describe('Cmd 합성 — 자식의 효과가 부모로 흐른다', () => {
  it('자식의 Fetch가 부모 런타임에서 실행된다', async () => {
    const api = { load: vi.fn().mockResolvedValue({ items: ['a', 'b'] }) };
    const app = mount(App, { api });
    fire(findByTestId(app.view(), 'todo-fetch'));
    expect(app.focus('todo-list').model().loading).toBe(true);
    await flush();
    expect(api.load).toHaveBeenCalledOnce();
    expect(app.focus('todo-list').model().todos).toHaveLength(2);
  });

  it('자식의 효과는 flush 전까지 실행되지 않는다', () => {
    const api = { load: vi.fn() };
    const app = mount(App, { api });
    fire(findByTestId(app.view(), 'todo-fetch'));
    expect(api.load).not.toHaveBeenCalled();
  });

  it('자식 여러 개가 낸 Cmd가 배치로 합쳐진다', async () => {
    const api = { load: vi.fn().mockResolvedValue({ items: [] }) };
    const app = mount(App, { api });
    app.focus('counter').dispatch({ type: 'Sync' });
    app.focus('todo-list').dispatch({ type: 'Sync' });
    expect(app.pendingCmds().length).toBeGreaterThanOrEqual(2);
    await flush();
    expect(app.pendingCmds()).toHaveLength(0);
  });
});

// ------------------------------------------------------------
// 7. 조건부 자식 — 키가 바뀌면 상태가 초기화된다
// ------------------------------------------------------------
describe('조건부 자식 — 키가 바뀌면 상태가 초기화된다', () => {
  const mountApp = () => mount(App, { api: { load: async () => ({ items: [] }) } });

  it('탭을 바꾸면 이전 탭 자식 상태는 사라진다', () => {
    const app = mountApp();
    fire(findByTestId(app.focus('counter').view(), 'inc'));
    fire(findByTestId(app.focus('counter').view(), 'inc'));
    expect(app.focus('counter').model().count).toBe(2);

    fire(findByTestId(app.view(), 'tab-todos'));
    fire(findByTestId(app.view(), 'tab-counter'));

    expect(app.focus('counter').model().count).toBe(0);
  });

  it('같은 키를 유지하면 상태가 보존된다', () => {
    const app = mountApp();
    fire(findByTestId(app.focus('counter').view(), 'inc'));
    fire(findByTestId(app.view(), 'toggle-theme'));
    fire(findByTestId(app.view(), 'toggle-theme'));
    expect(app.focus('counter').model().count).toBe(1);
  });
});

// ------------------------------------------------------------
// 8. 컴포넌트 계약 (회귀 방지)
// ------------------------------------------------------------
describe('컴포넌트 계약', () => {
  it.each([
    [Counter, { start: 0 }],
    [TodoList, { initial: [] }],
    [TodoItem, { id: 1, text: 'x', done: false }],
  ])('%s는 결정적이다', (Comp, props) => {
    const a = mount(Comp, props);
    const b = mount(Comp, props);
    expect(snapshot(a.view())).toEqual(snapshot(b.view()));
  });

  it.each([
    [Counter, { start: 0 }],
    [TodoList, { initial: [] }],
  ])('%s의 update는 원본을 변형하지 않는다', (Comp, props) => {
    const app = mount(Comp, props);
    const before = JSON.stringify(app.model());
    app.dispatch({ type: 'Inc' });
    expect(JSON.stringify(app.model())).not.toBe(before);
  });

  it.each([
    [Counter, { start: 0 }],
    [TodoList, { initial: [] }],
  ])('%s의 view는 플랫폼 중립적이다', (Comp, props) => {
    const el = mount(Comp, props).view();
    expect(() => structuredClone(snapshot(el))).not.toThrow();
  });
});

// ------------------------------------------------------------
// 9. 계약 위반 감지
// ------------------------------------------------------------
describe('계약 위반 감지', () => {
  it('자식 키가 중복되면 경고한다', () => {
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
    const app = mount(App, { forceDuplicateKeys: true });
    app.view();
    expect(warn).toHaveBeenCalledWith(expect.stringContaining('duplicate key'));
    expect(app.warnings().some((w) => w.includes('duplicate key'))).toBe(true);
    warn.mockRestore();
  });

  it('flush 없이 pending Cmd가 남아 있으면 알 수 있다', () => {
    const app = mount(App, { api: { load: async () => ({ items: [] }) } });
    fire(findByTestId(app.view(), 'todo-fetch'));
    expect(app.pendingCmds().length).toBeGreaterThan(0);
  });
});

// ============================================================
// 사용자 계약 요약
// ------------------------------------------------------------
// - 컴포넌트는 mount(Comp, props)로 단독 테스트 가능하다.
// - 부모는 자식을 Element.child(Comp, { key, props, map })로 합성한다.
// - 자식이 낸 메시지는 map을 거쳐 부모의 lastMsg()에 도착한다.
// - app.focus(key)로 자식에게 내려가 자식만 조작·검증할 수 있다.
// - 자식 상태는 부모 모델에 새지 않고,
//   뷰에서 사라진 key는 unmount되어 다음 mount에 초기화된다.
// - 자식의 Cmd는 트리 전체에서 배치로 flush된다.
// - view()는 함수(핸들러)만 빼면 직렬화 가능한 순수 데이터다.
// - update는 원본을 변형하지 않는다.
// ============================================================
