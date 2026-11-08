/**
 * Navigation registry. Icons live in `lib/icons.ts` so this file stays data-only
 * and can be imported by state modules without pulling in components.
 */

export type NavGroup = "Workspace" | "GitHub" | "Intelligence" | "System";

export type NavStatus = "ready" | "planned";

export type NavItem = {
  id: string;
  label: string;
  group: NavGroup;
  status: NavStatus;
  phase: number;
  summary: string;
  bullets: string[];
};

export const NAV_GROUPS: NavGroup[] = ["Workspace", "GitHub", "Intelligence", "System"];

export const NAV_ITEMS: NavItem[] = [
  {
    id: "home",
    label: "Home",
    group: "Workspace",
    status: "ready",
    phase: 0,
    summary: "Your environment, at a glance.",
    bullets: [],
  },
  {
    id: "projects",
    label: "My Projects",
    group: "Workspace",
    status: "ready",
    phase: 0,
    summary: "Projects group repositories, branches and local directories.",
    bullets: [
      "Local project registry stored in SQLite",
      "Directory validation and live Git state per project",
      "Fetch, pull, push, commit and branch create/switch",
    ],
  },
  {
    id: "terminal",
    label: "Terminal",
    group: "Workspace",
    status: "planned",
    phase: 1,
    summary: "A real PTY session, profiled per project.",
    bullets: [
      "fish / bash / zsh sessions with native process control",
      "Tabs, splits and project working directories",
      "Quick actions wired to the active project",
    ],
  },
  {
    id: "repositories",
    label: "Repositories",
    group: "GitHub",
    status: "ready",
    phase: 1,
    summary: "Your GitHub repositories, read from the API — not a webpage in a window.",
    bullets: [
      "OAuth device flow with tokens in the system keyring",
      "Public and private repositories, organisations included",
      "README, files, commits, pull requests, issues and branches inside GRAFT",
      "Edit and commit text files when you have push access",
      "Star, fork, clone and create issues without leaving the app",
    ],
  },
  {
    id: "review",
    label: "Code Review",
    group: "GitHub",
    status: "planned",
    phase: 3,
    summary: "Pull requests, diffs and review conversations in one workspace.",
    bullets: [
      "Side-by-side diffs with inline comments",
      "Review, approve and request changes",
      "Optional AI review suggestions, clearly marked as suggestions",
    ],
  },
  {
    id: "profile",
    label: "Profile Studio",
    group: "GitHub",
    status: "ready",
    phase: 3,
    summary: "Design your GitHub profile README visually, then publish deliberately.",
    bullets: [
      "Live snapshot: avatar, pins, stats and contribution calendar",
      "Five presets, four layouts and reorderable sections",
      "Achievements, organisations and custom Markdown",
      "App-only draft stored in SQLite — never auto-published",
      "Explicit publish to <login>/<login> with your own commit message",
    ],
  },
  {
    id: "explore",
    label: "Explore",
    group: "GitHub",
    status: "planned",
    phase: 3,
    summary: "Find repositories by language, topic, activity and health.",
    bullets: [
      "Search and sort with rate-limit awareness",
      "Popularity, activity and health presented separately",
      "Curated collections such as Arch, Wayland, Rust and tooling",
    ],
  },
  {
    id: "collections",
    label: "Collections",
    group: "GitHub",
    status: "planned",
    phase: 3,
    summary: "Your own library of repositories, with notes and status.",
    bullets: [
      "Tags, notes, priority and review state",
      "Statuses: want to try, studying, using, contributing, archived",
      "Local-first storage, exportable",
    ],
  },
  {
    id: "watchtower",
    label: "Watchtower",
    group: "Intelligence",
    status: "planned",
    phase: 2,
    summary: "A single place for what changed in the repositories you care about.",
    bullets: [
      "Favourite any repository, yours or someone else's",
      "Releases, commits, pull requests, issues and workflow failures",
      "Conditional requests and sensible polling — no runaway API usage",
    ],
  },
  {
    id: "extract",
    label: "Extract Studio",
    group: "Intelligence",
    status: "planned",
    phase: 4,
    summary: "Extract a component from a large repository into its own project.",
    bullets: [
      "Repository scanner with language-aware dependency analysis",
      "Copy, move, split and fork strategies with validation",
      "Reviewable plans before anything is written or published",
    ],
  },
  {
    id: "ai",
    label: "AI Workspace",
    group: "Intelligence",
    status: "planned",
    phase: 5,
    summary: "Your providers, your keys, your context — never a required dependency.",
    bullets: [
      "Anthropic, OpenAI, OpenRouter, Ollama, LM Studio and compatible endpoints",
      "Explicit context selection before anything leaves the machine",
      "Assistance for reviews, extraction and explanations; GRAFT works without it",
    ],
  },
  {
    id: "plugins",
    label: "Plugin Marketplace",
    group: "System",
    status: "planned",
    phase: 6,
    summary: "Optional capabilities with declared, reviewable permissions.",
    bullets: [
      "Capability-based permissions with explicit approval",
      "Themes, integrations and native GRAFT plugins kept distinct",
      "Signed plugins and version checks",
    ],
  },
  {
    id: "settings",
    label: "Settings",
    group: "System",
    status: "ready",
    phase: 0,
    summary: "Appearance, navigation, dashboard, data and environment.",
    bullets: [],
  },
];

export function navById(id: string): NavItem | undefined {
  return NAV_ITEMS.find((item) => item.id === id);
}

export function defaultNavOrder(): string[] {
  return NAV_ITEMS.map((item) => item.id);
}

export type NavSection = { name: string; items: NavItem[] };

/** Applies the persisted order, hides disabled modules and floats pinned ones. */
export function navGroups(order: string[], hidden: string[], pinned: string[]): NavSection[] {
  const known = new Set(NAV_ITEMS.map((item) => item.id));
  const sanitized = order.filter((id) => known.has(id));
  for (const item of NAV_ITEMS) {
    if (!sanitized.includes(item.id)) sanitized.push(item.id);
  }

  const visible = sanitized
    .map((id) => navById(id))
    .filter((item): item is NavItem => !!item && !hidden.includes(item.id));

  const pinnedItems = visible.filter((item) => pinned.includes(item.id));
  const rest = visible.filter((item) => !pinned.includes(item.id));

  const sections: NavSection[] = NAV_GROUPS.map((name) => ({
    name,
    items: rest.filter((item) => item.group === name),
  })).filter((section) => section.items.length > 0);

  return pinnedItems.length > 0 ? [{ name: "Pinned", items: pinnedItems }, ...sections] : sections;
}
