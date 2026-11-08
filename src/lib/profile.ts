/**
 * Profile Studio design model. This mirrors the Rust `ProfileDesign` struct:
 * the builder edits this object, Rust renders it from the live snapshot.
 */

export type SectionId =
  | "header"
  | "about"
  | "stats"
  | "pinned"
  | "contributions"
  | "achievements"
  | "orgs"
  | "custom";

export type Layout = "banner" | "split" | "compact" | "terminal";
export type Align = "center" | "left";
export type StatsStyle = "badges" | "table" | "inline";
export type PinnedStyle = "cards" | "list" | "table";
export type ContributionStyle = "heatmap" | "ascii" | "badge" | "summary";
export type BadgeStyle = "flat" | "flat-square" | "plastic" | "for-the-badge" | "social";

export type SectionDesign = {
  id: string;
  enabled: boolean;
  title?: string | null;
};

export type AchievementBadge = {
  name: string;
  tier: "bronze" | "silver" | "gold";
};

export type ProfileDesign = {
  version: number;
  preset: string;
  layout: Layout;
  align: Align;
  accent: string;
  showHeadings: boolean;
  sections: SectionDesign[];
  header: {
    avatar: boolean;
    avatarSize: number;
    showName: boolean;
    showLogin: boolean;
    tagline: string;
    showStatus: boolean;
  };
  about: {
    useGithubBio: boolean;
    bioOverride: string;
    company: boolean;
    location: boolean;
    website: boolean;
    joined: boolean;
    socials: boolean;
  };
  stats: {
    style: StatsStyle;
    badgeStyle: BadgeStyle;
    fields: string[];
  };
  pinned: {
    source: "pinned" | "top";
    style: PinnedStyle;
    limit: number;
    description: boolean;
    language: boolean;
    stars: boolean;
    forks: boolean;
  };
  contributions: {
    style: ContributionStyle;
    weeks: number;
    showTotals: boolean;
    showBreakdown: boolean;
    showStreak: boolean;
  };
  achievements: {
    showLink: boolean;
    badges: AchievementBadge[];
  };
  organizations: {
    limit: number;
    style: "avatars" | "list";
  };
  customMarkdown: string;
  footer: string;
};

export const SECTION_LABELS: Record<string, string> = {
  header: "Identity header",
  about: "About",
  stats: "Stats",
  pinned: "Pinned repositories",
  contributions: "Activity map",
  achievements: "Achievements",
  orgs: "Organizations",
  custom: "Custom Markdown",
};

export const STAT_FIELDS: { id: string; label: string }[] = [
  { id: "repos", label: "Repositories" },
  { id: "followers", label: "Followers" },
  { id: "following", label: "Following" },
  { id: "stars", label: "Stars" },
  { id: "contributions", label: "Contributions" },
  { id: "commits", label: "Commits" },
  { id: "pullRequests", label: "Pull requests" },
  { id: "issues", label: "Issues" },
  { id: "reviews", label: "Reviews" },
  { id: "gists", label: "Gists" },
  { id: "streak", label: "Current streak" },
  { id: "longest", label: "Longest streak" },
];

export const ACHIEVEMENT_CATALOG = [
  "Pull Shark",
  "Quickdraw",
  "Starstruck",
  "Pair Extraordinaire",
  "Galaxy Brain",
  "YOLO",
  "Public Sponsor",
  "Heart On Your Sleeve",
  "Open Sourcerer",
];

export const BADGE_STYLES: { id: BadgeStyle; label: string }[] = [
  { id: "flat-square", label: "Flat square" },
  { id: "flat", label: "Flat" },
  { id: "plastic", label: "Plastic" },
  { id: "for-the-badge", label: "For the badge" },
  { id: "social", label: "Social" },
];

export const PRESETS = [
  {
    id: "minimal",
    label: "Minimal",
    description: "Avatar, name and bio. Nothing loud.",
  },
  {
    id: "showcase",
    label: "Showcase",
    description: "Split header, stats and pinned repositories.",
  },
  {
    id: "signal",
    label: "Signal",
    description: "Stats table, activity map and achievements.",
  },
  {
    id: "terminal",
    label: "Terminal",
    description: "A text-card header with compact stats.",
  },
  {
    id: "everything",
    label: "Everything",
    description: "Every section, fully laid out.",
  },
] as const;

const ALL_SECTIONS: SectionId[] = [
  "header",
  "about",
  "stats",
  "pinned",
  "contributions",
  "achievements",
  "orgs",
  "custom",
];

export function defaultDesign(): ProfileDesign {
  return {
    version: 1,
    preset: "everything",
    layout: "split",
    align: "center",
    accent: "2ea043",
    showHeadings: true,
    sections: ALL_SECTIONS.map((id) => ({ id, enabled: id !== "custom", title: null })),
    header: {
      avatar: true,
      avatarSize: 120,
      showName: true,
      showLogin: true,
      tagline: "",
      showStatus: true,
    },
    about: {
      useGithubBio: true,
      bioOverride: "",
      company: true,
      location: true,
      website: true,
      joined: true,
      socials: true,
    },
    stats: {
      style: "badges",
      badgeStyle: "flat-square",
      fields: ["repos", "followers", "stars", "contributions", "streak"],
    },
    pinned: {
      source: "pinned",
      style: "cards",
      limit: 6,
      description: true,
      language: true,
      stars: true,
      forks: true,
    },
    contributions: {
      style: "heatmap",
      weeks: 52,
      showTotals: true,
      showBreakdown: false,
      showStreak: true,
    },
    achievements: { showLink: true, badges: [] },
    organizations: { limit: 8, style: "avatars" },
    customMarkdown: "",
    footer: "",
  };
}

export function presetDesign(id: string): ProfileDesign {
  const design = defaultDesign();
  design.preset = id;

  switch (id) {
    case "minimal":
      design.layout = "banner";
      design.stats.style = "inline";
      design.sections = design.sections.map((section) => ({
        ...section,
        enabled: section.id === "header" || section.id === "about",
      }));
      design.about.company = false;
      design.about.joined = false;
      break;
    case "showcase":
      design.layout = "split";
      design.stats.style = "badges";
      design.stats.fields = ["repos", "followers", "stars", "contributions"];
      design.pinned.style = "cards";
      design.contributions.style = "summary";
      design.sections = design.sections.map((section) => ({
        ...section,
        enabled: ["header", "about", "stats", "pinned", "contributions"].includes(section.id),
      }));
      break;
    case "signal":
      design.layout = "banner";
      design.stats.style = "table";
      design.stats.fields = [
        "repos",
        "followers",
        "following",
        "stars",
        "contributions",
        "commits",
        "pullRequests",
        "streak",
        "longest",
      ];
      design.contributions.style = "heatmap";
      design.contributions.showBreakdown = true;
      design.pinned.style = "list";
      design.sections = design.sections.map((section) => ({
        ...section,
        enabled: ["header", "about", "stats", "contributions", "achievements"].includes(section.id),
      }));
      break;
    case "terminal":
      design.layout = "terminal";
      design.stats.style = "inline";
      design.pinned.style = "list";
      design.sections = design.sections.map((section) => ({
        ...section,
        enabled: ["header", "about", "stats", "pinned"].includes(section.id),
      }));
      break;
    default:
      design.layout = "split";
      design.stats.style = "badges";
      design.pinned.style = "cards";
      design.contributions.style = "heatmap";
      design.sections = ALL_SECTIONS.map((section) => ({
        id: section,
        enabled: section !== "custom",
        title: null,
      }));
      break;
  }
  return design;
}

/** Applying a preset changes structure, never the words the user wrote. */
export function applyPreset(current: ProfileDesign, id: string): ProfileDesign {
  const next = presetDesign(id);
  next.header.tagline = current.header.tagline;
  next.header.avatarSize = current.header.avatarSize;
  next.about.bioOverride = current.about.bioOverride;
  next.about.useGithubBio = current.about.useGithubBio;
  next.customMarkdown = current.customMarkdown;
  next.footer = current.footer;
  next.achievements.badges = current.achievements.badges;
  next.accent = current.accent;
  next.showHeadings = current.showHeadings;
  return next;
}

function object(value: unknown): Record<string, unknown> {
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

function bool(value: unknown, fallback: boolean): boolean {
  return typeof value === "boolean" ? value : fallback;
}

function text(value: unknown, fallback: string): string {
  return typeof value === "string" ? value : fallback;
}

function numeric(value: unknown, fallback: number): number {
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

/** Merges a stored design over the defaults so old drafts keep loading. */
export function normalizeDesign(input: unknown): ProfileDesign {
  const base = defaultDesign();
  const raw = object(input);
  if (Object.keys(raw).length === 0) return base;

  const header = object(raw.header);
  const about = object(raw.about);
  const stats = object(raw.stats);
  const pinned = object(raw.pinned);
  const contributions = object(raw.contributions);
  const achievements = object(raw.achievements);
  const organizations = object(raw.organizations);

  const sections = Array.isArray(raw.sections)
    ? raw.sections
        .map((section) => object(section))
        .filter((section) => typeof section.id === "string" && section.id in SECTION_LABELS)
        .map((section) => ({
          id: section.id as string,
          enabled: bool(section.enabled, true),
          title: typeof section.title === "string" ? section.title : null,
        }))
    : base.sections;

  const fields = Array.isArray(stats.fields)
    ? stats.fields.filter((field): field is string => typeof field === "string")
    : base.stats.fields;

  const badges = Array.isArray(achievements.badges)
    ? achievements.badges
        .map((badge) => object(badge))
        .filter((badge) => typeof badge.name === "string" && badge.name.trim().length > 0)
        .map(
          (badge): AchievementBadge => ({
            name: String(badge.name).slice(0, 60),
            tier:
              badge.tier === "gold" || badge.tier === "silver" ? badge.tier : "bronze",
          }),
        )
    : [];

  const layout = ["banner", "split", "compact", "terminal"].includes(String(raw.layout))
    ? (raw.layout as Layout)
    : base.layout;
  const statsStyle = ["badges", "table", "inline"].includes(String(stats.style))
    ? (stats.style as StatsStyle)
    : base.stats.style;
  const pinnedStyle = ["cards", "list", "table"].includes(String(pinned.style))
    ? (pinned.style as PinnedStyle)
    : base.pinned.style;
  const contributionStyle = ["heatmap", "ascii", "badge", "summary"].includes(
    String(contributions.style),
  )
    ? (contributions.style as ContributionStyle)
    : base.contributions.style;

  return {
    version: numeric(raw.version, 1),
    preset: text(raw.preset, "custom"),
    layout,
    align: raw.align === "left" ? "left" : "center",
    accent: text(raw.accent, base.accent),
    showHeadings: bool(raw.showHeadings, true),
    sections,
    header: {
      avatar: bool(header.avatar, base.header.avatar),
      avatarSize: numeric(header.avatarSize, base.header.avatarSize),
      showName: bool(header.showName, base.header.showName),
      showLogin: bool(header.showLogin, base.header.showLogin),
      tagline: text(header.tagline, ""),
      showStatus: bool(header.showStatus, base.header.showStatus),
    },
    about: {
      useGithubBio: bool(about.useGithubBio, base.about.useGithubBio),
      bioOverride: text(about.bioOverride, ""),
      company: bool(about.company, true),
      location: bool(about.location, true),
      website: bool(about.website, true),
      joined: bool(about.joined, true),
      socials: bool(about.socials, true),
    },
    stats: {
      style: statsStyle,
      badgeStyle: BADGE_STYLES.some((style) => style.id === stats.badgeStyle)
        ? (stats.badgeStyle as BadgeStyle)
        : "flat-square",
      fields,
    },
    pinned: {
      source: pinned.source === "top" ? "top" : "pinned",
      style: pinnedStyle,
      limit: numeric(pinned.limit, base.pinned.limit),
      description: bool(pinned.description, true),
      language: bool(pinned.language, true),
      stars: bool(pinned.stars, true),
      forks: bool(pinned.forks, true),
    },
    contributions: {
      style: contributionStyle,
      weeks: numeric(contributions.weeks, base.contributions.weeks),
      showTotals: bool(contributions.showTotals, true),
      showBreakdown: bool(contributions.showBreakdown, false),
      showStreak: bool(contributions.showStreak, true),
    },
    achievements: {
      showLink: bool(achievements.showLink, true),
      badges,
    },
    organizations: {
      limit: numeric(organizations.limit, base.organizations.limit),
      style: organizations.style === "list" ? "list" : "avatars",
    },
    customMarkdown: text(raw.customMarkdown, ""),
    footer: text(raw.footer, ""),
  };
}
