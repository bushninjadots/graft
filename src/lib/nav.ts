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
    status: "ready",
    phase: 1,
    summary: "Real PTY sessions with project working directories and persistent tabs.",
    bullets: [
      "fish / bash / zsh sessions with native process control",
      "Tabs that stay alive while you move around GRAFT",
      "Linked project directories validated before launch",
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
      "Explore public repositories by keyword, language, topic or author",
      "Auto-refresh from GitHub on focus, on a timer and on demand",
    ],
  },
  {
    id: "review",
    label: "Code Review",
    group: "GitHub",
    status: "ready",
    phase: 3,
    summary: "Choose a repository and review pull requests with diffs and conversations.",
    bullets: [
      "Unified and side-by-side diffs with inline comment targets",
      "Approve, request changes or comment with explicit confirmation",
      "Review history and inline conversations loaded from GitHub",
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
      "Set or clear your live GitHub status from the builder",
      "App-only draft stored in SQLite — never auto-published",
      "Explicit publish to <login>/<login> with your own commit message",
    ],
  },
  {
    id: "explore",
    label: "Explore",
    group: "GitHub",
    status: "ready",
    phase: 3,
    summary: "Search public GitHub and keep the queries that matter.",
    bullets: [
      "Keyword, language, topic and author search through the GitHub API",
      "Saved searches stored locally in SQLite",
      "Quick topics for Hyprland, ricing, Linux, Arch, dotfiles, Wayland, NixOS and Rust",
    ],
  },
  {
    id: "collections",
    label: "Collections",
    group: "GitHub",
    status: "ready",
    phase: 3,
    summary: "A local library of repositories with notes and status.",
    bullets: [
      "Multiple collections with repository membership",
      "Statuses: want to try, studying, using, contributing, archived",
      "Local-first notes and status; metadata read from GitHub",
    ],
  },
  {
    id: "watchtower",
    label: "Watchtower",
    group: "Intelligence",
    status: "ready",
    phase: 2,
    summary: "What changed, what you starred, what you use and what to explore next.",
    bullets: [
      "Favourite any repository, yours or someone else's",
      "GitHub stars, local visit frequency and derived similar-repository recommendations",
      "Commits, pull requests, issues, releases, reviews, workflow failures, stars and forks",
      "ETag-conditional polling with a local alert history and native notifications",
      "Background monitoring is opt-in and can be paused at any time",
    ],
  },
  {
    id: "extract",
    label: "Extract Studio",
    group: "Intelligence",
    status: "ready",
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
    status: "ready",
    phase: 5,
    summary: "Your providers, your keys, your context — never a required dependency.",
    bullets: [
      "Anthropic, Gemini, OpenAI, OpenRouter, Ollama, LM Studio, llama.cpp and compatible endpoints",
      "Keys in the system keyring with streaming responses from the Rust engine",
      "Explicit file context and per-message approval before remote content is sent",
      "Saved conversations and assistance seeded from reviews and extraction plans",
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
