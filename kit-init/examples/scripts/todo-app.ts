import "@scriptkit/sdk";

export const metadata = {
  name: "Todo App",
  description:
    "Todo app: projects, labels, priorities, due dates, Today/Upcoming views, and ;todo capture sync",
  alias: "todo",
  icon: "list-todo",
  sdkCapabilities: ["arg", "div", "fields", "hud", "select", "skPath"],
  executionTopology: "typescript-script",
  menuSyntax: [
    {
      family: "capture.v1",
      targets: ["todo"],
      accepts: ["tags", "date", "relativeDate", "recurrence", "daily", "multiWeekday", "priority", "url", "kv"],
      label: "Add to Todo App",
      payloadSchema: "kit://schema/menu-syntax/payload-v1",
      defaultHandler: false,
    },
  ],
};

// ---------------------------------------------------------------------------
// Types & persistence
// ---------------------------------------------------------------------------

type Priority = 1 | 2 | 3 | 4;

type Project = {
  id: string;
  name: string;
  color: string;
  order: number;
};

type Label = {
  id: string;
  name: string;
  color: string;
};

type Task = {
  id: string;
  title: string;
  notes: string;
  projectId: string;
  labelIds: string[];
  priority: Priority;
  due: string | null;
  completed: boolean;
  createdAt: string;
  completedAt: string | null;
  captureRaw?: string;
};

type Store = {
  version: 1;
  projects: Project[];
  labels: Label[];
  tasks: Task[];
};

function storeFilePath(): string {
  return process.env.TODO_APP_STORE_PATH || skPath("todo-app", "store.json");
}

const CAPTURE_LOG = skPath("menu-syntax", "todos.jsonl");

const PROJECT_COLORS = ["#db4034", "#ff9933", "#fad000", "#7ecc49", "#299438", "#6accbc", "#158fad", "#14aaf5", "#884dff", "#af38eb", "#eb96eb", "#e05194"];
const LABEL_COLORS = ["#eb96eb", "#14aaf5", "#7ecc49", "#ff9933", "#db4034"];

async function readText(filePath: string): Promise<string> {
  return await Bun.file(filePath).text();
}

async function writeText(filePath: string, content: string): Promise<void> {
  await Bun.write(filePath, content);
}

async function appendText(filePath: string, content: string): Promise<void> {
  const file = Bun.file(filePath);
  const previous = await file.exists() ? await file.text() : "";
  await writeText(filePath, previous + content);
}

function uid(prefix: string): string {
  return `${prefix}_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

function defaultStore(): Store {
  const inbox = { id: "inbox", name: "Inbox", color: "#14aaf5", order: 0 };
  const work = { id: uid("proj"), name: "Work", color: "#ff9933", order: 1 };
  const personal = { id: uid("proj"), name: "Personal", color: "#7ecc49", order: 2 };
  return {
    version: 1,
    projects: [inbox, work, personal],
    labels: [
      { id: uid("lbl"), name: "errands", color: "#14aaf5" },
      { id: uid("lbl"), name: "deep-work", color: "#884dff" },
    ],
    tasks: [
      {
        id: uid("task"),
        title: "Welcome to Todo App",
        notes: "Use Today, Upcoming, or Projects from the main menu. Capture tasks from the launcher with `;todo Buy milk p1 tomorrow #errands`.",
        projectId: inbox.id,
        labelIds: [],
        priority: 4,
        due: startOfDay(new Date()).toISOString(),
        completed: false,
        createdAt: new Date().toISOString(),
        completedAt: null,
      },
      {
        id: uid("task"),
        title: "Review quarterly goals",
        notes: "Block 45 minutes on the calendar.",
        projectId: work.id,
        labelIds: [],
        priority: 2,
        due: addDays(startOfDay(new Date()), 2).toISOString(),
        completed: false,
        createdAt: new Date().toISOString(),
        completedAt: null,
      },
      {
        id: uid("task"),
        title: "Pick up groceries",
        notes: "Milk, eggs, coffee beans",
        projectId: personal.id,
        labelIds: [],
        priority: 3,
        due: addDays(startOfDay(new Date()), 1).toISOString(),
        completed: false,
        createdAt: new Date().toISOString(),
        completedAt: null,
      },
    ],
  };
}

async function loadStore(): Promise<Store> {
  try {
    const raw = await readText(storeFilePath());
    const parsed = JSON.parse(raw) as Store;
    if (parsed?.version === 1 && Array.isArray(parsed.tasks)) return parsed;
  } catch {
    // first run
  }
  const seeded = defaultStore();
  await saveStore(seeded);
  return seeded;
}

async function saveStore(store: Store): Promise<void> {
  await writeText(storeFilePath(), JSON.stringify(store, null, 2));
}

// ---------------------------------------------------------------------------
// Dates (Todo shorthand subset)
// ---------------------------------------------------------------------------

function startOfDay(d: Date): Date {
  const x = new Date(d);
  x.setHours(0, 0, 0, 0);
  return x;
}

function addDays(d: Date, n: number): Date {
  const x = new Date(d);
  x.setDate(x.getDate() + n);
  return x;
}

function parseDue(input: string, now = new Date()): string | null {
  const raw = input.trim();
  if (!raw) return null;

  const lower = raw.toLowerCase();
  const today = startOfDay(now);

  if (lower === "today") return today.toISOString();
  if (lower === "tomorrow") return addDays(today, 1).toISOString();
  if (lower === "next week") return addDays(today, 7).toISOString();
  if (lower === "noon") {
    const d = new Date(today);
    d.setHours(12, 0, 0, 0);
    return d.toISOString();
  }
  if (lower === "eod" || lower === "end of day") {
    const d = new Date(today);
    d.setHours(23, 59, 0, 0);
    return d.toISOString();
  }

  const weekdays = ["sun", "mon", "tue", "wed", "thu", "fri", "sat"];
  const dayIdx = weekdays.findIndex((w) => lower === w || lower.startsWith(`${w} `));
  if (dayIdx >= 0) {
    const target = new Date(today);
    const delta = (dayIdx - target.getDay() + 7) % 7 || 7;
    return addDays(target, delta).toISOString();
  }

  const rel = lower.match(/^\+(\d+)\s*([dwm])$/);
  if (rel) {
    const n = Number(rel[1]);
    const unit = rel[2];
    if (unit === "d") return addDays(today, n).toISOString();
    if (unit === "w") return addDays(today, n * 7).toISOString();
    if (unit === "m") {
      const d = new Date(today);
      d.setMonth(d.getMonth() + n);
      return d.toISOString();
    }
  }

  if (/^\d{4}-\d{2}-\d{2}$/.test(raw)) {
    const [y, m, d] = raw.split("-").map(Number);
    return new Date(y, m - 1, d).toISOString();
  }

  const parsed = Date.parse(raw);
  if (!Number.isNaN(parsed)) return new Date(parsed).toISOString();

  return null;
}

function formatDue(iso: string | null): string {
  if (!iso) return "No date";
  const d = new Date(iso);
  const today = startOfDay(new Date());
  const dueDay = startOfDay(d);
  const diff = Math.round((dueDay.getTime() - today.getTime()) / 86_400_000);
  const time = d.getHours() || d.getMinutes() ? d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) : "";
  if (diff === 0) return time ? `Today ${time}` : "Today";
  if (diff === 1) return "Tomorrow";
  if (diff < 0) return `${Math.abs(diff)}d overdue`;
  if (diff < 7) return d.toLocaleDateString([], { weekday: "short" });
  return d.toLocaleDateString([], { month: "short", day: "numeric" });
}

function isDueTodayOrOverdue(task: Task, now = new Date()): boolean {
  if (!task.due || task.completed) return false;
  return startOfDay(new Date(task.due)) <= startOfDay(now);
}

function isUpcoming(task: Task, now = new Date()): boolean {
  if (!task.due || task.completed) return false;
  const due = startOfDay(new Date(task.due));
  const end = addDays(startOfDay(now), 7);
  return due > startOfDay(now) && due <= end;
}

function priorityLabel(p: Priority): string {
  return `P${p}`;
}

function priorityColor(p: Priority): string {
  switch (p) {
    case 1:
      return "#db4034";
    case 2:
      return "#ff9933";
    case 3:
      return "#14aaf5";
    default:
      return "#808080";
  }
}

function parsePriority(input: string): Priority {
  const m = input.trim().match(/^p?([1-4])$/i);
  const n = m ? Number(m[1]) : Number(input);
  if (n >= 1 && n <= 4) return n as Priority;
  return 4;
}

// ---------------------------------------------------------------------------
// Store helpers
// ---------------------------------------------------------------------------

function projectById(store: Store, id: string): Project | undefined {
  return store.projects.find((p) => p.id === id);
}

function labelById(store: Store, id: string): Label | undefined {
  return store.labels.find((l) => l.id === id);
}

function openTasks(store: Store): Task[] {
  return store.tasks.filter((t) => !t.completed);
}

function taskChoice(task: Task, store: Store) {
  const project = projectById(store, task.projectId);
  const labels = task.labelIds.map((id) => labelById(store, id)?.name).filter(Boolean);
  const due = formatDue(task.due);
  const pColor = priorityColor(task.priority);
  return {
    name: task.title,
    value: task.id,
    description: [priorityLabel(task.priority), due, project?.name, labels.length ? `#${labels.join(" #")}` : ""]
      .filter(Boolean)
      .join(" · "),
    preview: `
      <motion.div class="p-6 space-y-4 h-full">
        <motion.div class="flex items-start gap-3">
          <motion.div class="w-3 h-3 rounded-full mt-1.5" style="background:${pColor}"></motion.div>
          <motion.div>
            <motion.h2 class="text-xl font-semibold text-white">${escapeHtml(task.title)}</motion.h2>
            <motion.p class="text-sm text-gray-400 mt-1">${escapeHtml(project?.name ?? "Inbox")} · ${escapeHtml(due)}</motion.p>
          </motion.div>
        </motion.div>
        ${task.notes ? `<motion.p class="text-gray-300 text-sm leading-relaxed">${escapeHtml(task.notes)}</motion.p>` : ""}
        ${labels.length ? `<motion.div class="flex flex-wrap gap-2">${labels.map((l) => `<motion.span class="px-2 py-0.5 rounded text-xs" style="background:#333;color:#ccc">#${escapeHtml(l!)}</motion.span>`).join("")}</motion.div>` : ""}
      </motion.div>
    `,
  };
}

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/"/g, "&quot;");
}

async function importMenuSyntaxCaptures(store: Store): Promise<{ added: number; store: Store }> {
  let added = 0;
  try {
    const raw = await readText(CAPTURE_LOG);
    const lines = raw.split("\n").filter(Boolean);
    const inbox = store.projects.find((p) => p.id === "inbox") ?? store.projects[0];
    for (const line of lines) {
      const row = JSON.parse(line) as {
        body?: string;
        tags?: string[];
        priority?: number;
        due?: string | null;
        raw?: string;
        createdAt?: string;
      };
      const title = (row.body ?? "").trim();
      if (!title) continue;
      if (store.tasks.some((t) => t.captureRaw && t.captureRaw === row.raw)) continue;

      const labelIds: string[] = [];
      for (const tag of row.tags ?? []) {
        const name = tag.replace(/^#/, "").toLowerCase();
        let label = store.labels.find((l) => l.name === name);
        if (!label) {
          label = { id: uid("lbl"), name, color: LABEL_COLORS[store.labels.length % LABEL_COLORS.length] };
          store.labels.push(label);
        }
        labelIds.push(label.id);
      }

      store.tasks.push({
        id: uid("task"),
        title,
        notes: row.raw ? `Captured: ${row.raw}` : "",
        projectId: inbox.id,
        labelIds,
        priority: parsePriority(String(row.priority ?? 4)),
        due: row.due ?? null,
        completed: false,
        createdAt: row.createdAt ?? new Date().toISOString(),
        completedAt: null,
        captureRaw: row.raw,
      });
      added++;
    }
  } catch {
    // no capture file yet
  }
  if (added) await saveStore(store);
  return { added, store };
}

// ---------------------------------------------------------------------------
// Interactive flows
// ---------------------------------------------------------------------------

async function pickProject(store: Store, hint = "Project"): Promise<string> {
  const sorted = [...store.projects].sort((a, b) => a.order - b.order);
  return arg(
    hint,
    sorted.map((p) => ({
      name: p.name,
      value: p.id,
      description: `${openTasks(store).filter((t) => t.projectId === p.id).length} open tasks`,
      preview: `<motion.div class="h-full" style="background: linear-gradient(135deg, ${p.color}55, #111)"></motion.div>`,
    })),
  );
}

async function pickLabels(store: Store): Promise<string[]> {
  if (!store.labels.length) return [];
  const picked = await select(
    "Labels (multi-select, submit when done)",
    store.labels.map((l) => ({ name: `#${l.name}`, value: l.id, description: l.color })),
  );
  return picked;
}

async function addTaskFlow(store: Store): Promise<Store> {
  const [title, notes, dueText, priorityText] = await fields([
    { name: "title", label: "Task name" },
    { name: "notes", label: "Description", placeholder: "Optional details" },
    { name: "due", label: "Due date", placeholder: "today, tomorrow, Fri, 2026-05-20, +3d" },
    { name: "priority", label: "Priority", value: "4", placeholder: "1 (urgent) – 4 (normal)" },
  ]);

  if (!title?.trim()) {
    hud("Task needs a title");
    return store;
  }

  const projectId = await pickProject(store, "Add to project");
  const labelIds = await pickLabels(store);
  const due = parseDue(dueText ?? "");

  store.tasks.push({
    id: uid("task"),
    title: title.trim(),
    notes: (notes ?? "").trim(),
    projectId,
    labelIds,
    priority: parsePriority(priorityText ?? "4"),
    due,
    completed: false,
    createdAt: new Date().toISOString(),
    completedAt: null,
  });
  await saveStore(store);
  hud("Task added");
  return store;
}

async function editTaskFlow(store: Store, task: Task): Promise<Store> {
  const action = await arg(task.title, [
    { name: "Edit details", value: "edit", description: "Title, notes, due, priority" },
    { name: "Move project", value: "move", description: projectById(store, task.projectId)?.name ?? "Inbox" },
    { name: "Set labels", value: "labels", description: `${task.labelIds.length} labels` },
    { name: task.completed ? "Mark incomplete" : "Complete task", value: "toggle", description: "Toggle done state" },
    { name: "Delete task", value: "delete", description: "Permanent" },
    { name: "Back", value: "back", description: "Return to list" },
  ]);

  switch (action) {
    case "edit": {
      const [title, notes, dueText, priorityText] = await fields([
        { name: "title", label: "Task name", value: task.title },
        { name: "notes", label: "Description", value: task.notes },
        { name: "due", label: "Due date", value: task.due ? formatDue(task.due) : "", placeholder: "today, tomorrow, +1w" },
        { name: "priority", label: "Priority", value: String(task.priority) },
      ]);
      if (title?.trim()) task.title = title.trim();
      task.notes = (notes ?? "").trim();
      task.priority = parsePriority(priorityText ?? String(task.priority));
      const parsedDue = parseDue(dueText ?? "");
      task.due = dueText?.trim() ? parsedDue : null;
      await saveStore(store);
      hud("Task updated");
      break;
    }
    case "move":
      task.projectId = await pickProject(store, "Move to project");
      await saveStore(store);
      hud("Moved");
      break;
    case "labels":
      task.labelIds = await pickLabels(store);
      await saveStore(store);
      hud("Labels updated");
      break;
    case "toggle":
      task.completed = !task.completed;
      task.completedAt = task.completed ? new Date().toISOString() : null;
      await saveStore(store);
      hud(task.completed ? "Completed" : "Reopened");
      break;
    case "delete": {
      const ok = await confirm(`Delete “${task.title}”?`);
      if (ok) {
        store.tasks = store.tasks.filter((t) => t.id !== task.id);
        await saveStore(store);
        hud("Deleted");
      }
      break;
    }
    default:
      break;
  }
  return store;
}

async function browseTasks(store: Store, title: string, tasks: Task[]): Promise<Store> {
  if (!tasks.length) {
    await div(`<motion.div class="p-8 text-gray-400">No tasks in ${escapeHtml(title)}. Add one from the main menu.</motion.div>`);
    return store;
  }

  const sorted = [...tasks].sort((a, b) => {
    const ad = a.due ? new Date(a.due).getTime() : Number.MAX_SAFE_INTEGER;
    const bd = b.due ? new Date(b.due).getTime() : Number.MAX_SAFE_INTEGER;
    if (ad !== bd) return ad - bd;
    return a.priority - b.priority;
  });

  const pickedId = await arg(title, sorted.map((t) => taskChoice(t, store)));
  const task = store.tasks.find((t) => t.id === pickedId);
  if (!task) return store;
  return editTaskFlow(store, task);
}

async function manageProjects(store: Store): Promise<Store> {
  const action = await arg("Projects", [
    { name: "New project", value: "new" },
    { name: "Browse projects", value: "browse" },
    { name: "Back", value: "back" },
  ]);

  if (action === "new") {
    const [name] = await fields([{ name: "name", label: "Project name" }]);
    if (!name?.trim()) return store;
    store.projects.push({
      id: uid("proj"),
      name: name.trim(),
      color: PROJECT_COLORS[store.projects.length % PROJECT_COLORS.length],
      order: store.projects.length,
    });
    await saveStore(store);
    hud("Project created");
    return store;
  }

  if (action === "browse") {
    const projectId = await pickProject(store, "Open project");
    const tasks = openTasks(store).filter((t) => t.projectId === projectId);
    const project = projectById(store, projectId);
    return browseTasks(store, project?.name ?? "Project", tasks);
  }

  return store;
}

async function manageLabels(store: Store): Promise<Store> {
  const action = await arg("Labels", [
    { name: "New label", value: "new" },
    { name: "Filter by label", value: "filter" },
    { name: "Back", value: "back" },
  ]);

  if (action === "new") {
    const [name] = await fields([{ name: "name", label: "Label name", placeholder: "errands" }]);
    if (!name?.trim()) return store;
    store.labels.push({
      id: uid("lbl"),
      name: name.trim().replace(/^#/, "").toLowerCase(),
      color: LABEL_COLORS[store.labels.length % LABEL_COLORS.length],
    });
    await saveStore(store);
    hud("Label created");
    return store;
  }

  if (action === "filter") {
    const labelId = await arg(
      "Pick a label",
      store.labels.map((l) => ({
        name: `#${l.name}`,
        value: l.id,
        description: `${openTasks(store).filter((t) => t.labelIds.includes(l.id)).length} tasks`,
      })),
    );
    const label = labelById(store, labelId);
    const tasks = openTasks(store).filter((t) => t.labelIds.includes(labelId));
    return browseTasks(store, `#${label?.name ?? "label"}`, tasks);
  }

  return store;
}

async function showDashboard(store: Store): Promise<void> {
  const open = openTasks(store);
  const today = open.filter((t) => isDueTodayOrOverdue(t));
  const upcoming = open.filter((t) => isUpcoming(t));
  const p1 = open.filter((t) => t.priority === 1);

  await div(`
    <motion.div class="p-8 space-y-6">
      <motion.h1 class="text-2xl font-bold text-white">Todo App</motion.h1>
      <motion.p class="text-gray-400 text-sm">Proof that Script Kit can host a full task manager: projects, labels, priorities, due dates, views, capture sync, and CRUD.</motion.p>
      <motion.div class="grid grid-cols-2 gap-4">
        <motion.div class="rounded-xl p-4" style="background:#db403422;border:1px solid #db403455">
          <motion.p class="text-3xl font-bold" style="color:#db4034">${today.length}</motion.p>
          <motion.p class="text-gray-300 text-sm">Today & overdue</motion.p>
        </motion.div>
        <motion.div class="rounded-xl p-4" style="background:#14aaf522;border:1px solid #14aaf555">
          <motion.p class="text-3xl font-bold" style="color:#14aaf5">${upcoming.length}</motion.p>
          <motion.p class="text-gray-300 text-sm">Upcoming (7 days)</motion.p>
        </motion.div>
        <motion.div class="rounded-xl p-4" style="background:#3336;border:1px solid #4448">
          <motion.p class="text-3xl font-bold text-white">${open.length}</motion.p>
          <motion.p class="text-gray-300 text-sm">Open tasks</motion.p>
        </motion.div>
        <motion.div class="rounded-xl p-4" style="background:#ff993322;border:1px solid #ff993355">
          <motion.p class="text-3xl font-bold" style="color:#ff9933">${p1.length}</motion.p>
          <motion.p class="text-gray-300 text-sm">Priority 1</motion.p>
        </motion.div>
      </motion.div>
      <motion.p class="text-xs text-gray-500">Data: ${escapeHtml(storeFilePath())}</motion.p>
    </motion.div>
  `);
}

async function searchTasks(store: Store): Promise<Store> {
  const query = await arg("Search tasks");
  const q = query.trim().toLowerCase();
  if (!q) return store;
  const matches = openTasks(store).filter(
    (t) => t.title.toLowerCase().includes(q) || t.notes.toLowerCase().includes(q),
  );
  return browseTasks(store, `Search: ${query}`, matches);
}

async function runMenuSyntaxCapture(store: Store): Promise<Store> {
  const payloadPath = process.env.KIT_MENU_SYNTAX_PAYLOAD_PATH;
  if (!payloadPath) {
    hud("Run from `;todo …` in the main menu to capture here");
    return store;
  }

  const payload = JSON.parse(await readText(payloadPath)) as {
    body?: string;
    tags?: string[];
    priority?: number;
    dates?: { iso?: string }[];
    raw?: string;
  };

  await appendText(
    CAPTURE_LOG,
    JSON.stringify({
      body: payload.body,
      tags: payload.tags,
      priority: payload.priority,
      due: payload.dates?.[0]?.iso ?? null,
      raw: payload.raw,
      createdAt: new Date().toISOString(),
    }) + "\n",
  );

  const { added, store: next } = await importMenuSyntaxCaptures(store);
  hud(added ? "Captured from menu syntax" : "Already in inbox");
  return next;
}

// ---------------------------------------------------------------------------
// SK_VERIFY — non-interactive proof path
// ---------------------------------------------------------------------------

async function runVerify(): Promise<void> {
  const verifyPath = skPath("todo-app", "verify-store.json");
  process.env.TODO_APP_STORE_PATH = verifyPath;

  const store = defaultStore();
  store.tasks = [];
  await writeText(verifyPath, JSON.stringify(store, null, 2));

  const inbox = store.projects[0].id;
  store.tasks.push({
    id: uid("task"),
    title: "Verify task",
    notes: "",
    projectId: inbox,
    labelIds: [],
    priority: 1,
    due: startOfDay(new Date()).toISOString(),
    completed: false,
    createdAt: new Date().toISOString(),
    completedAt: null,
  });
  await writeText(verifyPath, JSON.stringify(store, null, 2));

  const loaded = JSON.parse(await readText(verifyPath)) as Store;
  const task = loaded.tasks[0];
  task.completed = true;
  task.completedAt = new Date().toISOString();
  await writeText(verifyPath, JSON.stringify(loaded, null, 2));

  const finalStore = JSON.parse(await readText(verifyPath)) as Store;
  const dueOk = parseDue("tomorrow") !== null && parseDue("+3d") !== null;
  const todayCount = finalStore.tasks.filter((t) => t.completed).length;

  console.log(
    JSON.stringify({
      ok: dueOk && todayCount === 1,
      tasks: finalStore.tasks.length,
      parseDue: dueOk,
      storePath: verifyPath,
      verifyStore: verifyPath,
    }),
  );
}

// ---------------------------------------------------------------------------
// Main loop
// ---------------------------------------------------------------------------

const isVerify = process.env.SK_VERIFY === "1";
const isMenuSyntaxCapture = Boolean(process.env.KIT_MENU_SYNTAX_PAYLOAD_PATH);

if (isVerify) {
  await runVerify();
} else {
  let store = await loadStore();

  if (isMenuSyntaxCapture) {
    store = await runMenuSyntaxCapture(store);
  } else {
    // eslint-disable-next-line no-constant-condition
    while (true) {
      const open = openTasks(store);
      const todayCount = open.filter((t) => isDueTodayOrOverdue(t)).length;
      const upcomingCount = open.filter((t) => isUpcoming(t)).length;

      const action = await arg("Todo App", [
        {
          name: "Today",
          value: "today",
          description: `${todayCount} tasks due today or overdue`,
          preview: `<motion.div class="h-full flex items-center justify-center text-4xl" style="color:#db4034">☀️</motion.div>`,
        },
        {
          name: "Upcoming",
          value: "upcoming",
          description: `Next 7 days · ${upcomingCount} tasks`,
          preview: `<motion.div class="h-full flex items-center justify-center text-4xl" style="color:#14aaf5">📅</motion.div>`,
        },
        {
          name: "Inbox",
          value: "inbox",
          description: `${open.filter((t) => t.projectId === "inbox").length} tasks`,
        },
        { name: "All tasks", value: "all", description: `${open.length} open` },
        { name: "Add task", value: "add", description: "Title, due date, priority, project, labels" },
        { name: "Search", value: "search", description: "Filter open tasks" },
        { name: "Projects", value: "projects", description: `${store.projects.length} projects` },
        { name: "Labels", value: "labels", description: `${store.labels.length} labels` },
        {
          name: "Sync ;todo captures",
          value: "sync",
          description: "Import ~/.scriptkit/menu-syntax/todos.jsonl",
        },
        { name: "Dashboard", value: "dashboard", description: "Stats overview" },
        { name: "Quit", value: "quit", description: "Exit" },
      ]);

      switch (action) {
        case "today":
          store = await browseTasks(store, "Today", open.filter((t) => isDueTodayOrOverdue(t)));
          break;
        case "upcoming":
          store = await browseTasks(store, "Upcoming", open.filter((t) => isUpcoming(t)));
          break;
        case "inbox":
          store = await browseTasks(store, "Inbox", open.filter((t) => t.projectId === "inbox"));
          break;
        case "all":
          store = await browseTasks(store, "All tasks", open);
          break;
        case "add":
          store = await addTaskFlow(store);
          break;
        case "search":
          store = await searchTasks(store);
          break;
        case "projects":
          store = await manageProjects(store);
          break;
        case "labels":
          store = await manageLabels(store);
          break;
        case "sync": {
          const { added, store: next } = await importMenuSyntaxCaptures(store);
          store = next;
          hud(added ? `Imported ${added} capture(s)` : "Nothing new to import");
          break;
        }
        case "dashboard":
          await showDashboard(store);
          break;
        case "quit":
        default:
          process.exit(0);
      }
    }
  }
}
