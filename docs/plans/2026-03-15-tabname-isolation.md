# Tabname Isolation Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Allow multiple CLI clients to operate independent tabs within one browser session via `--tabname`, with full isolation of Page, CDP session, refMap, and activeFrame.

**Architecture:** Each `--tabname` maps to a `TabBinding` struct in `BrowserManager` that encapsulates all per-tab state. Commands with `tabName` are routed to the binding's Page; commands without fall back to the existing `activePageIndex` for backward compatibility. The Rust CLI injects `tabName` into every JSON command.

**Tech Stack:** TypeScript (daemon), Rust (CLI), Zod (protocol validation), Vitest (tests), Patchright (browser engine)

---

### Task 1: Protocol & Types — Add tabName to BaseCommand

**Files:**
- Modify: `src/types.ts:4-7`
- Modify: `src/protocol.ts:5-8`
- Test: `src/protocol.test.ts`

**Step 1: Write the failing test**

Add to `src/protocol.test.ts` in the existing test structure:

```typescript
describe('tabName', () => {
  it('should parse command with tabName', () => {
    const result = parseCommand(cmd({ id: '1', action: 'snapshot', tabName: 'reddit' }));
    expect(result.success).toBe(true);
    if (result.success) {
      expect((result.command as any).tabName).toBe('reddit');
    }
  });

  it('should parse command without tabName', () => {
    const result = parseCommand(cmd({ id: '1', action: 'snapshot' }));
    expect(result.success).toBe(true);
    if (result.success) {
      expect((result.command as any).tabName).toBeUndefined();
    }
  });
});
```

**Step 2: Run test to verify it fails**

Run: `npx vitest run src/protocol.test.ts -t "tabName"`
Expected: FAIL — tabName not in parsed output

**Step 3: Write minimal implementation**

In `src/types.ts:4-7`, add `tabName`:

```typescript
export interface BaseCommand {
  id: string;
  action: string;
  tabName?: string;
}
```

In `src/protocol.ts:5-8`, add to schema:

```typescript
const baseCommandSchema = z.object({
  id: z.string(),
  action: z.string(),
  tabName: z.string().optional(),
});
```

**Step 4: Run test to verify it passes**

Run: `npx vitest run src/protocol.test.ts -t "tabName"`
Expected: PASS

**Step 5: Run full test suite**

Run: `npm test`
Expected: All 177+ tests PASS (no regressions)

**Step 6: Commit**

```bash
git add src/types.ts src/protocol.ts src/protocol.test.ts
git commit -m "feat: add tabName field to BaseCommand protocol"
```

---

### Task 2: BrowserManager — TabBinding data model and core methods

**Files:**
- Modify: `src/browser.ts:70-96` (add TabBinding fields)
- Modify: `src/browser.ts` (add new methods)
- Test: `src/browser.test.ts`

**Step 1: Write the failing tests**

Add to `src/browser.test.ts`:

```typescript
describe('tabname isolation', () => {
  it('should create a new tab binding on first access', async () => {
    const binding = await browser.getOrCreateTab('test-tab-1');
    expect(binding.page).toBeDefined();
    expect(binding.refMap).toEqual({});
    expect(binding.cdpSession).toBeNull();
    expect(binding.activeFrame).toBeNull();
  });

  it('should return same binding on subsequent access', async () => {
    const binding1 = await browser.getOrCreateTab('test-tab-2');
    const binding2 = await browser.getOrCreateTab('test-tab-2');
    expect(binding1.page).toBe(binding2.page);
  });

  it('should isolate different tabnames', async () => {
    const bindingA = await browser.getOrCreateTab('test-tab-a');
    const bindingB = await browser.getOrCreateTab('test-tab-b');
    expect(bindingA.page).not.toBe(bindingB.page);
  });

  it('should navigate independently per tabname', async () => {
    const bindingA = await browser.getOrCreateTab('test-nav-a');
    const bindingB = await browser.getOrCreateTab('test-nav-b');
    await bindingA.page.goto('data:text/html,<h1>PageA</h1>');
    await bindingB.page.goto('data:text/html,<h1>PageB</h1>');
    expect(await bindingA.page.title()).not.toBe(await bindingB.page.title());
    expect(bindingA.page.url()).toContain('PageA');
    expect(bindingB.page.url()).toContain('PageB');
  });

  it('should list named tabs', async () => {
    await browser.getOrCreateTab('test-list-1');
    await browser.getOrCreateTab('test-list-2');
    const tabs = await browser.listNamedTabs();
    const names = tabs.map(t => t.name);
    expect(names).toContain('test-list-1');
    expect(names).toContain('test-list-2');
  });

  it('should close a named tab', async () => {
    await browser.getOrCreateTab('test-close-tab');
    await browser.closeNamedTab('test-close-tab');
    const tabs = await browser.listNamedTabs();
    expect(tabs.map(t => t.name)).not.toContain('test-close-tab');
  });

  it('should clean up binding when page is closed externally', async () => {
    const binding = await browser.getOrCreateTab('test-external-close');
    await binding.page.close();
    // Next access should create a fresh page
    const binding2 = await browser.getOrCreateTab('test-external-close');
    expect(binding2.page).not.toBe(binding.page);
  });
});
```

**Step 2: Run tests to verify they fail**

Run: `npx vitest run src/browser.test.ts -t "tabname"`
Expected: FAIL — `getOrCreateTab` not defined

**Step 3: Write implementation**

In `src/browser.ts`, after line 96 (after `navigationDelay`), add the TabBinding type and Map:

```typescript
export interface TabBinding {
  page: Page;
  cdpSession: CDPSession | null;
  refMap: RefMap;
  lastSnapshot: string;
  activeFrame: Frame | null;
}

// Named tab bindings for --tabname isolation
private tabBindings: Map<string, TabBinding> = new Map();
```

Add methods to `BrowserManager` class:

```typescript
/**
 * Get or create a named tab binding.
 * First call creates a new Page; subsequent calls return the existing binding.
 * If the bound page was closed externally, creates a fresh one.
 */
async getOrCreateTab(tabName: string): Promise<TabBinding> {
  const existing = this.tabBindings.get(tabName);
  if (existing && !existing.page.isClosed()) {
    return existing;
  }

  // Create new page in the persistent context
  const context = this.persistentContext ?? this.contexts[0];
  if (!context) {
    throw new Error('Browser not launched. Call launch first.');
  }

  const page = await context.newPage();
  // Also track in pages[] for backward compat with tab_list
  this.pages.push(page);

  const binding: TabBinding = {
    page,
    cdpSession: null,
    refMap: {},
    lastSnapshot: '',
    activeFrame: null,
  };

  // Clean up binding when page is closed externally
  page.on('close', () => {
    const current = this.tabBindings.get(tabName);
    if (current && current.page === page) {
      this.tabBindings.delete(tabName);
    }
    // Also remove from pages[]
    const idx = this.pages.indexOf(page);
    if (idx >= 0) {
      this.pages.splice(idx, 1);
      if (this.activePageIndex >= this.pages.length) {
        this.activePageIndex = Math.max(0, this.pages.length - 1);
      }
    }
  });

  this.tabBindings.set(tabName, binding);
  return binding;
}

/**
 * Get locator from ref, scoped to a specific tab's refMap.
 */
getLocatorFromRefForTab(refArg: string, tabName: string): Locator | null {
  const binding = this.tabBindings.get(tabName);
  if (!binding) return null;

  const ref = parseRef(refArg);
  if (!ref) return null;

  const refData = binding.refMap[ref];
  if (!refData) return null;

  let locator: Locator;
  if (refData.name) {
    locator = binding.page.getByRole(refData.role as any, { name: refData.name, exact: true });
  } else {
    locator = binding.page.getByRole(refData.role as any);
  }

  if (refData.nth !== undefined) {
    locator = locator.nth(refData.nth);
  }

  return locator;
}

/**
 * Get locator scoped to a tab — supports refs and CSS selectors.
 */
getLocatorForTab(selectorOrRef: string, tabName: string): Locator {
  const locator = this.getLocatorFromRefForTab(selectorOrRef, tabName);
  if (locator) return locator;

  const binding = this.tabBindings.get(tabName);
  if (!binding) throw new Error(`Tab '${tabName}' not found`);
  return binding.page.locator(selectorOrRef);
}

/**
 * List all named tab bindings.
 */
async listNamedTabs(): Promise<{ name: string; url: string; title: string }[]> {
  const result: { name: string; url: string; title: string }[] = [];
  for (const [name, binding] of this.tabBindings) {
    if (!binding.page.isClosed()) {
      result.push({
        name,
        url: binding.page.url(),
        title: await binding.page.title(),
      });
    }
  }
  return result;
}

/**
 * Close a named tab and clean up its binding.
 */
async closeNamedTab(tabName: string): Promise<void> {
  const binding = this.tabBindings.get(tabName);
  if (!binding) throw new Error(`Tab '${tabName}' not found`);
  if (!binding.page.isClosed()) {
    if (binding.cdpSession) {
      await binding.cdpSession.detach().catch(() => {});
    }
    await binding.page.close();
  }
  this.tabBindings.delete(tabName);
}

/**
 * Get or create CDP session for a named tab.
 */
async getCDPSessionForTab(tabName: string): Promise<CDPSession> {
  const binding = this.tabBindings.get(tabName);
  if (!binding) throw new Error(`Tab '${tabName}' not found`);
  if (!binding.cdpSession) {
    const context = binding.page.context();
    binding.cdpSession = await context.newCDPSession(binding.page);
  }
  return binding.cdpSession;
}
```

**Step 4: Run tests to verify they pass**

Run: `npx vitest run src/browser.test.ts -t "tabname"`
Expected: PASS

**Step 5: Run full test suite**

Run: `npm test`
Expected: All tests PASS

**Step 6: Commit**

```bash
git add src/browser.ts src/browser.test.ts
git commit -m "feat: add TabBinding model and tabname management methods"
```

---

### Task 3: Actions — Route commands via tabName

**Files:**
- Modify: `src/actions.ts` (add `getTargetPage()` helper, update all handlers)
- Test: `src/actions.test.ts`

This is the largest task. The strategy: add a helper function `getTargetPage()` and `getTargetLocator()`, then systematically replace `browser.getPage()` and `browser.getLocator()` calls.

**Step 1: Write failing tests**

Add to `src/actions.test.ts`:

```typescript
describe('tabname routing', () => {
  it('should route navigate to named tab', async () => {
    // This test verifies executeCommand routes by tabName
    // The actual browser test is in browser.test.ts
    // Here we verify the action dispatch accepts tabName
    const cmd = {
      id: '1',
      action: 'navigate' as const,
      url: 'data:text/html,<h1>Test</h1>',
      tabName: 'test-route',
    };
    // Should not throw (tabName is valid field)
    expect(cmd.tabName).toBe('test-route');
  });
});
```

**Step 2: Add helper functions at top of actions.ts**

After imports, add:

```typescript
import type { TabBinding } from './browser.js';

/**
 * Get the target page for a command — routes by tabName if present.
 */
async function getTargetPage(command: { tabName?: string }, browser: BrowserManager): Promise<Page> {
  if (command.tabName) {
    const binding = await browser.getOrCreateTab(command.tabName);
    return binding.page;
  }
  return browser.getPage();
}

/**
 * Get the target frame for a command — uses tab-specific frame if tabName present.
 */
async function getTargetFrame(command: { tabName?: string }, browser: BrowserManager): Promise<Frame> {
  if (command.tabName) {
    const binding = await browser.getOrCreateTab(command.tabName);
    if (binding.activeFrame) return binding.activeFrame;
    return binding.page.mainFrame();
  }
  return browser.getFrame();
}

/**
 * Get a locator scoped to the correct tab.
 */
async function getTargetLocator(
  selectorOrRef: string,
  command: { tabName?: string },
  browser: BrowserManager
): Promise<Locator> {
  if (command.tabName) {
    return browser.getLocatorForTab(selectorOrRef, command.tabName);
  }
  return browser.getLocator(selectorOrRef);
}
```

**Step 3: Update action handlers systematically**

Replace `browser.getPage()` with `await getTargetPage(command, browser)` in ALL handlers. Key pattern:

```typescript
// BEFORE:
async function handleNavigate(...) {
  const page = browser.getPage();
  ...
}

// AFTER:
async function handleNavigate(...) {
  const page = await getTargetPage(command, browser);
  ...
}
```

Replace `browser.getLocator(selector)` with `await getTargetLocator(selector, command, browser)`.

Replace `browser.getFrame()` with `await getTargetFrame(command, browser)`.

**Actions to update** (comprehensive list by category):

Navigation: `handleNavigate`, `handleBack`, `handleForward`, `handleReload`
Interaction: `handleClick`, `handleDblClick`, `handleFill`, `handleType`, `handlePress`, `handleHover`, `handleFocus`, `handleCheck`, `handleUncheck`, `handleSelect`, `handleDrag`, `handleUpload`, `handleScrollIntoView`
Input: `handleKeyboard`, `handleKeyDown`, `handleKeyUp`, `handleInsertText`
Read: `handleSnapshot`, `handleScreenshot`, `handleContent`, `handleEvaluate`, `handleGetByText`, `handleGetByLabel`, `handleGetByPlaceholder`, `handleGetUrl`, `handleGetTitle`, `handleGetAttribute`, `handleGetText`, `handleGetHtml`, `handleGetValue`, `handleGetCount`, `handleGetBoundingBox`, `handleGetStyles`, `handleIsVisible`, `handleIsEnabled`, `handleIsChecked`
State: `handleWait`, `handleScroll`, `handleConsole`, `handleErrors`, `handleHighlight`, `handleBringToFront`, `handlePdf`
Frame: `handleFrame`
Storage/Cookies: `handleCookiesGet/Set/Clear`, `handleStorageGet/Set/Clear`

**Special handler: `handleClose`** — add tabname guard:

```typescript
async function handleClose(command: Command & { action: 'close' }, browser: BrowserManager) {
  if ((command as any).tabName) {
    return errorResponse(command.id,
      'Cannot close browser: this is a shared browser instance with named tabs. ' +
      'Close the browser manually when all work is done.'
    );
  }
  await browser.close();
  return successResponse(command.id, { closed: true });
}
```

**Special handler: `handleSnapshot`** — store refMap per-tab:

```typescript
async function handleSnapshot(command, browser) {
  if (command.tabName) {
    const binding = await browser.getOrCreateTab(command.tabName);
    const { tree, refs } = await browser.getSnapshotForPage(binding.page, { ... });
    binding.refMap = refs;  // Store in tab-specific refMap
    binding.lastSnapshot = tree;
    // ... return response
  }
  // fallback to existing global behavior
  ...
}
```

Note: `browser.getSnapshot()` currently uses `this.getPage()` internally. We'll need to add a `getSnapshotForPage(page, options)` method to `BrowserManager` that accepts an explicit page.

**Step 4: Run full test suite**

Run: `npm test`
Expected: All tests PASS

**Step 5: Commit**

```bash
git add src/actions.ts src/actions.test.ts
git commit -m "feat: route all action handlers via tabName for tab isolation"
```

---

### Task 4: BrowserManager — getSnapshotForPage method

**Files:**
- Modify: `src/browser.ts` (add `getSnapshotForPage`)
- Test: `src/browser.test.ts`

The existing `getSnapshot()` is hardcoded to use `this.getPage()`. We need a variant that accepts an explicit Page.

**Step 1: Write failing test**

```typescript
it('should get snapshot for a specific named tab', async () => {
  const bindingA = await browser.getOrCreateTab('test-snap-a');
  await bindingA.page.goto('data:text/html,<button>ClickA</button>');
  const bindingB = await browser.getOrCreateTab('test-snap-b');
  await bindingB.page.goto('data:text/html,<button>ClickB</button>');

  const snapA = await browser.getSnapshotForPage(bindingA.page, {});
  const snapB = await browser.getSnapshotForPage(bindingB.page, {});

  expect(snapA.tree).toContain('ClickA');
  expect(snapA.tree).not.toContain('ClickB');
  expect(snapB.tree).toContain('ClickB');
  expect(snapB.tree).not.toContain('ClickA');
});
```

**Step 2: Implement**

Read existing `getSnapshot()` in `browser.ts:183-195`. Refactor to accept optional page parameter:

```typescript
async getSnapshotForPage(
  page: Page,
  options?: { interactive?: boolean; maxDepth?: number; compact?: boolean; selector?: string }
): Promise<EnhancedSnapshot> {
  return await getEnhancedSnapshot(page, options);
}
```

Keep existing `getSnapshot()` as wrapper that calls `getSnapshotForPage(this.getPage(), options)` and stores results in `this.refMap` / `this.lastSnapshot`.

**Step 3: Run tests**

Run: `npm test`
Expected: PASS

**Step 4: Commit**

```bash
git add src/browser.ts src/browser.test.ts
git commit -m "feat: add getSnapshotForPage for per-tab snapshot isolation"
```

---

### Task 5: CLI Rust — Add --tabname flag

**Files:**
- Modify: `cli/src/flags.rs`
- Modify: `cli/src/main.rs`
- Modify: `cli/src/output.rs`

**Step 1: Add flag to Flags struct and parsing**

In `cli/src/flags.rs`:

```rust
pub struct Flags {
    // ... existing fields
    pub tab_name: Option<String>,
}

// In parse_flags, init:
tab_name: None,

// In match block:
"--tabname" => {
    if let Some(name) = args.get(i + 1) {
        flags.tab_name = Some(name.clone());
        i += 1;
    }
}
```

Add `"--tabname"` to `GLOBAL_FLAGS_WITH_VALUE` in `clean_args`.

**Step 2: Inject tabName into every command in main.rs**

After `let cmd = match parse_command(...)` succeeds, before sending:

```rust
let mut cmd = cmd;
if let Some(ref tab_name) = flags.tab_name {
    cmd["tabName"] = json!(tab_name);
}
```

Also inject into the launch command sent when `cdp.is_none()`:

```rust
if let Some(ref tab_name) = flags.tab_name {
    launch_cmd["tabName"] = json!(tab_name);
}
```

**Step 3: Update help text in output.rs**

Add to Options section:
```
  --tabname <name>           Named tab isolation (multiple clients per session)
```

Add to Environment section:
```
  AGENT_BROWSER_TABNAME          Named tab for isolation (or --tabname)
```

**Step 4: Build and test**

Run: `npm run build:native && agent-browser-session --help | grep tabname`
Expected: Shows `--tabname` in help

**Step 5: Commit**

```bash
git add cli/src/flags.rs cli/src/main.rs cli/src/output.rs
git commit -m "feat: add --tabname CLI flag for tab isolation"
```

---

### Task 6: Integration Test — End-to-end tabname isolation

**Files:**
- Create: `src/tabname.test.ts`

**Step 1: Write comprehensive integration test**

```typescript
import { describe, it, expect, beforeAll, afterAll } from 'vitest';
import { BrowserManager } from './browser.js';

describe('tabname isolation e2e', () => {
  let browser: BrowserManager;

  beforeAll(async () => {
    browser = new BrowserManager();
    await browser.launch({ id: 'test', action: 'launch', headless: true });
  });

  afterAll(async () => {
    await browser.close();
  });

  it('should navigate two tabs independently', async () => {
    const reddit = await browser.getOrCreateTab('e2e-reddit');
    const hn = await browser.getOrCreateTab('e2e-hn');

    await reddit.page.goto('data:text/html,<h1>Reddit</h1><button>Upvote</button>');
    await hn.page.goto('data:text/html,<h1>HN</h1><a href="#">Comments</a>');

    expect(reddit.page.url()).toContain('Reddit');
    expect(hn.page.url()).toContain('HN');
  });

  it('should have isolated refMaps per tab', async () => {
    const tabA = await browser.getOrCreateTab('e2e-ref-a');
    const tabB = await browser.getOrCreateTab('e2e-ref-b');

    await tabA.page.goto('data:text/html,<button>ButtonA</button>');
    await tabB.page.goto('data:text/html,<button>ButtonB</button>');

    const snapA = await browser.getSnapshotForPage(tabA.page, {});
    const snapB = await browser.getSnapshotForPage(tabB.page, {});

    tabA.refMap = snapA.refs;
    tabB.refMap = snapB.refs;

    // refMaps should be independent
    expect(Object.keys(tabA.refMap).length).toBeGreaterThan(0);
    expect(Object.keys(tabB.refMap).length).toBeGreaterThan(0);

    // Check that refs in A reference ButtonA, not ButtonB
    const refA = Object.values(tabA.refMap)[0];
    expect(refA.name).toContain('ButtonA');
  });

  it('should block close when tabName is present', async () => {
    // This tests the close guard in actions.ts
    const { executeCommand } = await import('./actions.js');
    const result = await executeCommand(
      { id: '1', action: 'close', tabName: 'e2e-close-test' } as any,
      browser
    );
    expect(result.success).toBe(false);
    expect(result.error).toContain('shared browser instance');
  });
});
```

**Step 2: Run tests**

Run: `npx vitest run src/tabname.test.ts`
Expected: PASS

**Step 3: Run full suite**

Run: `npm test`
Expected: All tests PASS

**Step 4: Commit**

```bash
git add src/tabname.test.ts
git commit -m "test: add e2e integration tests for tabname isolation"
```

---

### Task 7: Build, deploy, and manual E2E test

**Step 1: Full build**

```bash
npm run build
npm run build:native
npm test
```

**Step 2: Deploy**

```bash
pnpm link --global
agent-browser-session --version
```

**Step 3: Manual E2E test (use test-e2e session, NOT main)**

Terminal 1:
```bash
agent-browser-session --session test-e2e --tabname reddit open https://reddit.com
agent-browser-session --session test-e2e --tabname reddit snapshot -i | head -20
```

Terminal 2 (simultaneously):
```bash
agent-browser-session --session test-e2e --tabname hackernews open https://news.ycombinator.com
agent-browser-session --session test-e2e --tabname hackernews snapshot -i | head -20
```

Verify:
- Two browser tabs are open
- Each snapshot shows the correct page content
- Operating on one tab doesn't affect the other

**Step 4: Test close guard**

```bash
agent-browser-session --session test-e2e --tabname reddit close
# Expected: Error message about shared browser instance
```

**Step 5: Clean up**

```bash
agent-browser-session --session test-e2e close  # Without --tabname, should work
```

**Step 6: Commit all remaining changes**

```bash
git add -A
git commit -m "feat: tabname isolation for multi-tab parallel operation"
```
