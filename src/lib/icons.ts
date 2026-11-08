/**
 * Icon registry.
 *
 * Lucide 1.x exposes one component per subpath (`lucide-svelte/icons/<name>`),
 * so icons are imported individually and mapped to the stable keys used by
 * `Icon.svelte`. Brand marks (GitHub) are local because Lucide dropped brands.
 */
import House from "lucide-svelte/icons/house";
import FolderGit2 from "lucide-svelte/icons/folder-git-2";
import Terminal from "lucide-svelte/icons/terminal";
import GitBranch from "lucide-svelte/icons/git-branch";
import GitPullRequest from "lucide-svelte/icons/git-pull-request";
import UserRound from "lucide-svelte/icons/user-round";
import Compass from "lucide-svelte/icons/compass";
import Library from "lucide-svelte/icons/library";
import Radar from "lucide-svelte/icons/radar";
import Boxes from "lucide-svelte/icons/boxes";
import Sparkles from "lucide-svelte/icons/sparkles";
import Plug from "lucide-svelte/icons/plug";
import Settings from "lucide-svelte/icons/settings";
import Search from "lucide-svelte/icons/search";
import ChevronLeft from "lucide-svelte/icons/chevron-left";
import ChevronRight from "lucide-svelte/icons/chevron-right";
import ChevronUp from "lucide-svelte/icons/chevron-up";
import ChevronDown from "lucide-svelte/icons/chevron-down";
import Plus from "lucide-svelte/icons/plus";
import Trash2 from "lucide-svelte/icons/trash-2";
import RefreshCw from "lucide-svelte/icons/refresh-cw";
import Check from "lucide-svelte/icons/check";
import X from "lucide-svelte/icons/x";
import Pin from "lucide-svelte/icons/pin";
import Eye from "lucide-svelte/icons/eye";
import EyeOff from "lucide-svelte/icons/eye-off";
import GripVertical from "lucide-svelte/icons/grip-vertical";
import SlidersHorizontal from "lucide-svelte/icons/sliders-horizontal";
import ArrowUpRight from "lucide-svelte/icons/arrow-up-right";
import Circle from "lucide-svelte/icons/circle";
import GitHubMark from "./components/GitHubMark.svelte";

/** Lucide components share one generated type; reuse it instead of re-declaring props. */
type Glyph = typeof House;

export const icons: Record<string, Glyph> = {
  home: House,
  projects: FolderGit2,
  "folder-git": FolderGit2,
  terminal: Terminal,
  repositories: GitBranch,
  review: GitPullRequest,
  profile: UserRound,
  explore: Compass,
  collections: Library,
  watchtower: Radar,
  extract: Boxes,
  ai: Sparkles,
  plugins: Plug,
  settings: Settings,
  search: Search,
  "chevron-left": ChevronLeft,
  "chevron-right": ChevronRight,
  "chevron-up": ChevronUp,
  "chevron-down": ChevronDown,
  plus: Plus,
  trash: Trash2,
  refresh: RefreshCw,
  check: Check,
  x: X,
  pin: Pin,
  eye: Eye,
  "eye-off": EyeOff,
  grip: GripVertical,
  sliders: SlidersHorizontal,
  "arrow-up-right": ArrowUpRight,
  dot: Circle,
  circle: Circle,
  github: GitHubMark as unknown as Glyph,
};
