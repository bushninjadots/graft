# GRAFT — visual system

Identity: **modern industrial developer interface**. Precise, dense, quiet.
Linear's hierarchy, GitHub's familiarity, Vercel's panels, Raycast's speed.
Not a dashboard template: no oversized cards, no glassmorphism, no gradient hero.

## Principles

1. **Density is the feature.** Default font 13 px, 26 px rows, 10 px gutters.
   Density is user-adjustable (compact / comfortable / roomy) and token-driven.
2. **Borders do the work.** 1 px hairlines in `--border`, stronger only on hover or
   focus. Shadows are rare and subtle.
3. **Accent is a signal, not decoration.** Amber accent for active nav, primary
   action, and focus. Everything else is neutral.
4. **Typographic contrast over size contrast.** Monospace uppercase 10 px labels with
   0.11em tracking separate regions; body copy stays at one size.
5. **Motion explains state.** 90–120 ms, `cubic-bezier(.2,.7,.2,1)`. No bouncing,
   no parallax, no animation on scroll.
6. **Geometry:** 3 px default radius (0–12 px adjustable), 1 px borders, square
   corners inside panels. Nothing pill-shaped except the status dot.

## Token contract

Defined in `src/lib/themes.ts` as `ThemeTokens`, applied to `:root` by
`applyAppearance()`. Components may only use these variables.

| Token | Role |
| ----- | ---- |
| `bg` | app canvas, content background |
| `bg-elev` | inputs, embedded wells, status bar |
| `surface` | panels, cards |
| `surface-alt` | hover rows, secondary buttons |
| `border` / `border-strong` | dividers / hover and emphasis |
| `text` / `text-muted` / `text-faint` | primary / secondary / labels |
| `accent` / `accent-soft` / `accent-ink` | signal, tint, text-on-accent |
| `ok` / `warn` / `danger` / `info` | status only |
| `shadow` | overlay and palette elevation |

Geometry tokens: `--radius`, `--row-h`, `--pad`, `--gap`, `--font-base`,
`--sidebar-w`, `--topbar-h`, `--statusbar-h`, `--motion`.

## Themes

| id | Name | Character |
| -- | ---- | --------- |
| `graft` | GRAFT Industrial | graphite, steel, restrained amber (default) |
| `arch` | Arch | deep blue, cyan accents |
| `void` | Void | true black, near-mono |
| `crt` | CRT | phosphor green terminal |
| `blueprint` | Blueprint | drafting navy + cyan |
| `forest` | Forest | muted green, earthy |
| `minimal` | Minimal | light monochrome |

A theme is a token set, not a stylesheet. Adding one requires no component changes.
Per-project and imported themes reuse the same contract (Phase 7).

## Shell layout

```
┌ sidebar 224 ┬ topbar 46 ─────────────────────────┐
│ brand       │ breadcrumb · command · phase       │
│ Pinned      ├────────────────────────────────────┤
│ groups      │ content (scrolls)                  │
│ …           │                                    │
│ footer      │                                    │
├─────────────┴────────────────────────────────────┤
│ statusbar 26: engine · db · theme · version      │
└──────────────────────────────────────────────────┘
```

Sidebar collapses to 56 px icons. Groups: Workspace, GitHub, Intelligence, System.
Pinned modules float above the groups. Hidden modules disappear from the sidebar
but stay reachable from the command palette.

## Components

- `.panel` — 1 px border, surface fill, header row with mono label + hint.
- `.btn` — 2 px taller than a row, 1 px border; `.primary` fills with accent;
  `.ghost` for icon-only or tertiary actions; `.icon` squares.
- `.field` — inset well using `bg-elev`, accent focus ring at 35 %.
- `.badge` — mono uppercase 10 px; `.accent`, `.ok`, `.warn` variants for status.
- `.kbd` — mono 10 px with a 2 px bottom border to read as a physical key.
- Empty states: one honest sentence + one action. Never placeholder repositories.

## Content rules

- Real data or an explicit state ("Not connected · Phase 1"). Never fake success.
- Numbers use the mono face and tabular figures.
- Dates are relative (`2 h ago`) with an absolute value in `title`.
- Destructive actions are named after their effect ("Delete project", not "OK").
