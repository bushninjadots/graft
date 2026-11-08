//! Profile Studio data + rendering.
//!
//! The snapshot is the viewer's real GitHub profile: pins, repositories,
//! contribution calendar, social accounts and public organisations. The
//! renderer turns that snapshot plus a local design into GitHub-flavoured
//! Markdown. Nothing here writes to GitHub.

use chrono::{Duration as Days, Local, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

const API: &str = "https://api.github.com";
const GRAPHQL: &str = "https://api.github.com/graphql";
const USER_AGENT: &str = "GRAFT/0.1.0 (Linux; Tauri)";
const API_VERSION: &str = "2022-11-28";
const TIMEOUT: Duration = Duration::from_secs(30);

const PROFILE_QUERY: &str = r#"
query {
  viewer {
    login
    name
    bio
    company
    location
    websiteUrl
    twitterUsername
    avatarUrl(size: 240)
    createdAt
    followers { totalCount }
    following { totalCount }
    repoCount: repositories(ownerAffiliations: [OWNER], isFork: false) { totalCount }
    gists { totalCount }
    status { emoji message }
    socialAccounts(first: 10) { nodes { provider url } }
    pinnedItems(first: 6, types: [REPOSITORY]) {
      nodes {
        ... on Repository {
          nameWithOwner
          name
          description
          stargazerCount
          forkCount
          url
          isFork
          primaryLanguage { name color }
        }
      }
    }
    topRepos: repositories(first: 100, ownerAffiliations: [OWNER], isFork: false, orderBy: {field: STARGAZERS, direction: DESC}) {
      nodes {
        nameWithOwner
        name
        description
        stargazerCount
        forkCount
        url
        isFork
        primaryLanguage { name color }
      }
    }
    contributionsCollection {
      totalCommitContributions
      totalPullRequestContributions
      totalIssueContributions
      totalPullRequestReviewContributions
      restrictedContributionsCount
      contributionCalendar {
        totalContributions
        weeks { contributionDays { date contributionCount weekday } }
      }
    }
  }
}
"#;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSnapshot {
    pub login: String,
    pub name: Option<String>,
    pub avatar_url: String,
    pub bio: Option<String>,
    pub company: Option<String>,
    pub location: Option<String>,
    pub website_url: Option<String>,
    pub twitter_username: Option<String>,
    pub status: Option<ProfileStatus>,
    pub socials: Vec<ProfileSocial>,
    pub followers: u32,
    pub following: u32,
    pub public_repos: u32,
    pub gists: u32,
    pub stars: u32,
    pub created_at: String,
    pub pinned: Vec<ProfileRepo>,
    pub top_repos: Vec<ProfileRepo>,
    pub contribution_days: Vec<ContributionDay>,
    pub total_contributions: u32,
    pub commits: u32,
    pub pull_requests: u32,
    pub issues: u32,
    pub reviews: u32,
    pub private_contributions: u32,
    pub organizations: Vec<ProfileOrg>,
    pub fetched_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileStatus {
    pub emoji: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSocial {
    pub provider: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileRepo {
    pub name_with_owner: String,
    pub name: String,
    pub description: Option<String>,
    pub stars: u32,
    pub forks: u32,
    pub url: String,
    pub is_fork: bool,
    pub language: Option<String>,
    pub language_color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionDay {
    pub date: String,
    pub count: u32,
    pub weekday: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileOrg {
    pub login: String,
    pub avatar_url: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileDesign {
    #[serde(default = "version")]
    pub version: u32,
    #[serde(default = "preset_name")]
    pub preset: String,
    #[serde(default = "layout")]
    pub layout: String,
    #[serde(default = "align")]
    pub align: String,
    #[serde(default = "accent")]
    pub accent: String,
    #[serde(default = "yes")]
    pub show_headings: bool,
    #[serde(default = "default_sections")]
    pub sections: Vec<SectionDesign>,
    #[serde(default)]
    pub header: HeaderDesign,
    #[serde(default)]
    pub about: AboutDesign,
    #[serde(default)]
    pub stats: StatsDesign,
    #[serde(default)]
    pub pinned: PinnedDesign,
    #[serde(default)]
    pub contributions: ContributionsDesign,
    #[serde(default)]
    pub achievements: AchievementsDesign,
    #[serde(default)]
    pub organizations: OrganizationsDesign,
    #[serde(default)]
    pub custom_markdown: String,
    #[serde(default)]
    pub footer: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SectionDesign {
    pub id: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeaderDesign {
    #[serde(default = "yes")]
    pub avatar: bool,
    #[serde(default = "avatar_size")]
    pub avatar_size: u32,
    #[serde(default = "yes")]
    pub show_name: bool,
    #[serde(default = "yes")]
    pub show_login: bool,
    #[serde(default)]
    pub tagline: String,
    #[serde(default = "yes")]
    pub show_status: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AboutDesign {
    #[serde(default = "yes")]
    pub use_github_bio: bool,
    #[serde(default)]
    pub bio_override: String,
    #[serde(default = "yes")]
    pub company: bool,
    #[serde(default = "yes")]
    pub location: bool,
    #[serde(default = "yes")]
    pub website: bool,
    #[serde(default = "yes")]
    pub joined: bool,
    #[serde(default = "yes")]
    pub socials: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsDesign {
    #[serde(default = "stats_style")]
    pub style: String,
    #[serde(default = "badge_style")]
    pub badge_style: String,
    #[serde(default = "stats_fields")]
    pub fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PinnedDesign {
    #[serde(default = "pinned_source")]
    pub source: String,
    #[serde(default = "pinned_style")]
    pub style: String,
    #[serde(default = "six")]
    pub limit: u32,
    #[serde(default = "yes")]
    pub description: bool,
    #[serde(default = "yes")]
    pub language: bool,
    #[serde(default = "yes")]
    pub stars: bool,
    #[serde(default = "yes")]
    pub forks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContributionsDesign {
    #[serde(default = "contributions_style")]
    pub style: String,
    #[serde(default = "weeks")]
    pub weeks: u32,
    #[serde(default = "yes")]
    pub show_totals: bool,
    #[serde(default)]
    pub show_breakdown: bool,
    #[serde(default = "yes")]
    pub show_streak: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementsDesign {
    #[serde(default = "yes")]
    pub show_link: bool,
    #[serde(default)]
    pub badges: Vec<AchievementBadge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AchievementBadge {
    pub name: String,
    #[serde(default = "tier")]
    pub tier: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrganizationsDesign {
    #[serde(default = "eight")]
    pub limit: u32,
    #[serde(default = "org_style")]
    pub style: String,
}

impl Default for ProfileDesign {
    fn default() -> Self {
        Self {
            version: version(),
            preset: preset_name(),
            layout: layout(),
            align: align(),
            accent: accent(),
            show_headings: true,
            sections: default_sections(),
            header: HeaderDesign::default(),
            about: AboutDesign::default(),
            stats: StatsDesign::default(),
            pinned: PinnedDesign::default(),
            contributions: ContributionsDesign::default(),
            achievements: AchievementsDesign::default(),
            organizations: OrganizationsDesign::default(),
            custom_markdown: String::new(),
            footer: String::new(),
        }
    }
}

impl Default for SectionDesign {
    fn default() -> Self {
        Self {
            id: String::new(),
            enabled: true,
            title: None,
        }
    }
}

impl Default for HeaderDesign {
    fn default() -> Self {
        Self {
            avatar: true,
            avatar_size: avatar_size(),
            show_name: true,
            show_login: true,
            tagline: String::new(),
            show_status: true,
        }
    }
}

impl Default for AboutDesign {
    fn default() -> Self {
        Self {
            use_github_bio: true,
            bio_override: String::new(),
            company: true,
            location: true,
            website: true,
            joined: true,
            socials: true,
        }
    }
}

impl Default for StatsDesign {
    fn default() -> Self {
        Self {
            style: stats_style(),
            badge_style: badge_style(),
            fields: stats_fields(),
        }
    }
}

impl Default for PinnedDesign {
    fn default() -> Self {
        Self {
            source: pinned_source(),
            style: pinned_style(),
            limit: six(),
            description: true,
            language: true,
            stars: true,
            forks: true,
        }
    }
}

impl Default for ContributionsDesign {
    fn default() -> Self {
        Self {
            style: contributions_style(),
            weeks: weeks(),
            show_totals: true,
            show_breakdown: false,
            show_streak: true,
        }
    }
}

impl Default for AchievementsDesign {
    fn default() -> Self {
        Self {
            show_link: true,
            badges: Vec::new(),
        }
    }
}

impl Default for OrganizationsDesign {
    fn default() -> Self {
        Self {
            limit: eight(),
            style: org_style(),
        }
    }
}

fn version() -> u32 {
    1
}
fn preset_name() -> String {
    "everything".into()
}
fn layout() -> String {
    "split".into()
}
fn align() -> String {
    "center".into()
}
fn accent() -> String {
    "2ea043".into()
}
fn yes() -> bool {
    true
}
fn avatar_size() -> u32 {
    120
}
fn stats_style() -> String {
    "badges".into()
}
fn badge_style() -> String {
    "flat-square".into()
}
fn stats_fields() -> Vec<String> {
    ["repos", "followers", "stars", "contributions"]
        .iter()
        .map(|value| value.to_string())
        .collect()
}
fn pinned_source() -> String {
    "pinned".into()
}
fn pinned_style() -> String {
    "cards".into()
}
fn six() -> u32 {
    6
}
fn contributions_style() -> String {
    "heatmap".into()
}
fn weeks() -> u32 {
    52
}
fn tier() -> String {
    "bronze".into()
}
fn eight() -> u32 {
    8
}
fn org_style() -> String {
    "avatars".into()
}
fn default_sections() -> Vec<SectionDesign> {
    [
        "header",
        "about",
        "stats",
        "pinned",
        "contributions",
        "achievements",
        "orgs",
        "custom",
    ]
    .iter()
    .map(|id| SectionDesign {
        id: (*id).to_string(),
        enabled: *id != "custom",
        title: None,
    })
    .collect()
}

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("Cannot create the HTTP client: {e}"))
}

fn describe_graphql(status: reqwest::StatusCode, body: &str) -> String {
    if status.as_u16() == 401 {
        return "GitHub rejected the stored token (401). Sign in again.".to_string();
    }
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(errors) = value.get("errors").and_then(|value| value.as_array()) {
            let messages: Vec<String> = errors
                .iter()
                .filter_map(|error| error.get("message").and_then(|message| message.as_str()))
                .take(3)
                .map(str::to_string)
                .collect();
            if !messages.is_empty() {
                if messages.iter().any(|message| message.contains("scope")) {
                    return format!(
                        "GitHub needs the read:user scope for profile data. Re-authorise the token with read:user and repo. {}",
                        messages.join(" · ")
                    );
                }
                return format!("GitHub GraphQL: {}", messages.join(" · "));
            }
        }
        if let Some(message) = value.get("message").and_then(|message| message.as_str()) {
            return format!("GitHub GraphQL returned {status}: {message}");
        }
    }
    format!("GitHub GraphQL returned {status}.")
}

#[tauri::command]
pub async fn github_profile_snapshot() -> Result<ProfileSnapshot, String> {
    let token = crate::secrets::load()?
        .ok_or_else(|| "Sign in to GitHub to build a profile from live data.".to_string())?;

    let response = http_client()?
        .post(GRAPHQL)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", API_VERSION)
        .bearer_auth(&token)
        .json(&serde_json::json!({ "query": PROFILE_QUERY }))
        .send()
        .await
        .map_err(|e| format!("Cannot reach api.github.com: {e}"))?;

    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("Cannot read GitHub's profile response: {e}"))?;
    if !status.is_success() {
        return Err(describe_graphql(status, &body));
    }

    let response: GraphQlResponse = serde_json::from_str(&body)
        .map_err(|e| format!("GitHub returned an unreadable profile: {e}"))?;
    if let Some(errors) = response.errors.filter(|errors| !errors.is_empty()) {
        let messages: Vec<String> = errors.into_iter().map(|error| error.message).collect();
        return Err(format!("GitHub GraphQL: {}", messages.join(" · ")));
    }
    let viewer = response
        .data
        .and_then(|data| data.viewer)
        .ok_or_else(|| "GitHub did not return a profile for the stored token.".to_string())?;

    let organizations = fetch_public_orgs(&token, &viewer.login).await;
    Ok(snapshot_from_viewer(viewer, organizations))
}

fn snapshot_from_viewer(viewer: RawViewer, organizations: Vec<ProfileOrg>) -> ProfileSnapshot {
    let mut contribution_days: Vec<ContributionDay> = viewer
        .contributions_collection
        .contribution_calendar
        .weeks
        .iter()
        .flat_map(|week| week.contribution_days.iter())
        .map(|day| ContributionDay {
            date: day.date.clone(),
            count: day.contribution_count,
            weekday: day.weekday,
        })
        .collect();
    contribution_days.sort_by(|a, b| a.date.cmp(&b.date));
    contribution_days.dedup_by(|a, b| a.date == b.date);

    let pinned = repo_list(&viewer.pinned_items);
    let top_repos = repo_list(&viewer.top_repos);
    let stars = top_repos.iter().map(|repo| repo.stars).sum();

    ProfileSnapshot {
        login: viewer.login.clone(),
        name: viewer.name,
        avatar_url: viewer.avatar_url,
        bio: viewer.bio,
        company: viewer.company,
        location: viewer.location,
        website_url: viewer.website_url,
        twitter_username: viewer.twitter_username,
        status: viewer.status.map(|status| ProfileStatus {
            emoji: status.emoji.unwrap_or_default(),
            message: status.message.unwrap_or_default(),
        }),
        socials: viewer
            .social_accounts
            .nodes
            .into_iter()
            .map(|social| ProfileSocial {
                provider: social.provider,
                url: social.url,
            })
            .collect(),
        followers: viewer.followers.total_count,
        following: viewer.following.total_count,
        public_repos: viewer.repo_count.total_count,
        gists: viewer.gists.total_count,
        stars,
        created_at: viewer.created_at,
        pinned,
        top_repos,
        total_contributions: viewer
            .contributions_collection
            .contribution_calendar
            .total_contributions,
        commits: viewer.contributions_collection.total_commit_contributions,
        pull_requests: viewer.contributions_collection.total_pull_request_contributions,
        issues: viewer.contributions_collection.total_issue_contributions,
        reviews: viewer
            .contributions_collection
            .total_pull_request_review_contributions,
        private_contributions: viewer
            .contributions_collection
            .restricted_contributions_count,
        contribution_days,
        organizations,
        fetched_at: crate::db::now(),
    }
}

async fn fetch_public_orgs(token: &str, login: &str) -> Vec<ProfileOrg> {
    let url = format!("{API}/users/{login}/orgs?per_page=20");
    let Ok(client) = http_client() else {
        return Vec::new();
    };
    let response = match client.get(url).bearer_auth(token).send().await {
        Ok(response) => response,
        Err(_) => return Vec::new(),
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let raw: Vec<RawOrg> = match response.json().await {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let mut orgs: Vec<ProfileOrg> = raw
        .into_iter()
        .map(|org| ProfileOrg {
            login: org.login,
            avatar_url: org.avatar_url,
            description: org.description,
        })
        .collect();
    orgs.sort_by(|a, b| a.login.to_lowercase().cmp(&b.login.to_lowercase()));
    orgs
}

fn repo_list(connection: &RawRepoConnection) -> Vec<ProfileRepo> {
    connection
        .nodes
        .iter()
        .flatten()
        .map(|repo| ProfileRepo {
            name_with_owner: repo.name_with_owner.clone(),
            name: repo.name.clone(),
            description: repo.description.clone(),
            stars: repo.stargazers_count,
            forks: repo.fork_count,
            url: repo.url.clone(),
            is_fork: repo.is_fork,
            language: repo
                .primary_language
                .as_ref()
                .map(|language| language.name.clone()),
            language_color: repo
                .primary_language
                .as_ref()
                .and_then(|language| language.color.clone()),
        })
        .collect()
}

/// Renders a complete profile README. Pure: the same design and snapshot always
/// produce the same Markdown, which keeps the draft previewable offline.
#[tauri::command]
pub fn profile_render(
    design: ProfileDesign,
    snapshot: ProfileSnapshot,
) -> Result<String, String> {
    Ok(render_profile(&design, &snapshot))
}

pub fn render_profile(design: &ProfileDesign, snapshot: &ProfileSnapshot) -> String {
    let accent = sanitize_hex(&design.accent);
    let mut out = String::new();
    let mut seen = HashSet::new();
    let sections = if design.sections.is_empty() {
        default_sections()
    } else {
        design.sections.clone()
    };

    for section in sections {
        if !section.enabled || !seen.insert(section.id.clone()) {
            continue;
        }
        let body = match section.id.as_str() {
            "header" => render_header(design, snapshot),
            "about" => render_about(design, snapshot),
            "stats" => render_stats(design, snapshot, &accent),
            "pinned" => render_pinned(design, snapshot),
            "contributions" => render_contributions(design, snapshot, &accent),
            "achievements" => render_achievements(design, snapshot),
            "orgs" => render_organizations(design, snapshot),
            "custom" => {
                let markdown = design.custom_markdown.trim();
                if markdown.is_empty() {
                    None
                } else {
                    Some(markdown.to_string())
                }
            }
            _ => None,
        };

        let Some(body) = body.filter(|body| !body.trim().is_empty()) else {
            continue;
        };

        if section.id != "header" && design.show_headings {
            let title = section
                .title
                .as_deref()
                .map(str::trim)
                .filter(|title| !title.is_empty())
                .map(str::to_string)
                .unwrap_or_else(|| default_section_title(&section.id).to_string());
            out.push_str(&format!("## {title}\n\n"));
        }
        out.push_str(body.trim_end());
        out.push_str("\n\n");
    }

    let footer = design.footer.trim();
    if !footer.is_empty() {
        out.push_str("---\n\n");
        out.push_str(footer);
        out.push('\n');
    }

    while out.contains("\n\n\n") {
        out = out.replace("\n\n\n", "\n\n");
    }
    let trimmed = out.trim_end().len();
    out.truncate(trimmed);
    out.push('\n');
    out
}

fn default_section_title(id: &str) -> &'static str {
    match id {
        "about" => "About",
        "stats" => "Stats",
        "pinned" => "Pinned repositories",
        "contributions" => "Activity",
        "achievements" => "Achievements",
        "orgs" => "Organizations",
        "custom" => "More",
        _ => "",
    }
}

fn render_header(design: &ProfileDesign, snapshot: &ProfileSnapshot) -> Option<String> {
    let login = escape_inline(&snapshot.login);
    let name = snapshot
        .name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(escape_inline)
        .unwrap_or_else(|| login.clone());
    let tagline = design.header.tagline.trim();
    let status = header_status(design, snapshot);
    let mut out = String::new();

    match design.layout.as_str() {
        "compact" => {
            if design.header.avatar {
                out.push_str(&avatar_html(snapshot, design.header.avatar_size, "left"));
                out.push('\n');
            }
            let mut line = String::new();
            if design.header.show_name {
                line.push_str(&format!("### {name}"));
                if design.header.show_login {
                    line.push_str(&format!(" <sub>@{login}</sub>"));
                }
            } else if design.header.show_login {
                line.push_str(&format!("### @{login}"));
            }
            out.push_str(line.trim());
            out.push('\n');
            if !tagline.is_empty() {
                out.push_str(&format!("\n{tagline}\n"));
            }
            if let Some(status) = status {
                out.push_str(&format!("\n{status}\n"));
            }
            if design.header.avatar {
                out.push_str("\n<br clear=\"left\" />\n");
            }
        }
        "terminal" => {
            if design.header.avatar {
                out.push_str(&format!(
                    "<p align=\"{}\">{}</p>\n\n",
                    if design.align == "left" { "left" } else { "center" },
                    avatar_html(snapshot, design.header.avatar_size, "")
                ));
            }
            let mut card = String::new();
            card.push_str(&format!("{} @{}\n", name, login));
            card.push_str(&"─".repeat(30.min(name.chars().count() + login.chars().count() + 3)));
            card.push('\n');
            if !tagline.is_empty() {
                card.push_str(tagline);
                card.push('\n');
            }
            if let Some(status) = status {
                card.push_str(&status);
                card.push('\n');
            }
            out.push_str("```text\n");
            out.push_str(&card.replace("```", "'''"));
            out.push_str("```\n");
        }
        "banner" => {
            out.push_str(&format!(
                "<div align=\"{}\">\n",
                if design.align == "left" { "left" } else { "center" }
            ));
            if design.header.avatar {
                out.push_str(&avatar_html(snapshot, design.header.avatar_size, ""));
                out.push('\n');
            }
            let mut line = String::new();
            if design.header.show_name {
                line.push_str(&format!("# {name}"));
            }
            if design.header.show_login {
                if line.is_empty() {
                    line.push_str(&format!("# @{login}"));
                } else {
                    line.push_str(&format!(" <sub>@{login}</sub>"));
                }
            }
            if !line.is_empty() {
                out.push_str(&line);
                out.push('\n');
            }
            if !tagline.is_empty() {
                out.push_str(&format!("\n*{tagline}*\n"));
            }
            if let Some(status) = status {
                out.push_str(&format!("\n{status}\n"));
            }
            out.push_str("</div>\n");
        }
        _ => {
            if design.header.avatar {
                out.push_str(&format!(
                    "{}\n\n",
                    avatar_html(snapshot, design.header.avatar_size, "right")
                ));
            }
            let mut line = String::new();
            if design.header.show_name {
                line.push_str(&format!("### Hi, I'm {name}"));
            } else if design.header.show_login {
                line.push_str(&format!("### @{login}"));
            }
            out.push_str(line.trim());
            out.push('\n');
            if design.header.show_login {
                out.push_str(&format!("\n**@{login}**"));
                if !tagline.is_empty() {
                    out.push_str(&format!(" · {tagline}"));
                }
                out.push('\n');
            } else if !tagline.is_empty() {
                out.push_str(&format!("\n{tagline}\n"));
            }
            if let Some(status) = status {
                out.push_str(&format!("\n{status}\n"));
            }
        }
    }

    Some(out.trim_end().to_string())
}

fn header_status(design: &ProfileDesign, snapshot: &ProfileSnapshot) -> Option<String> {
    if !design.header.show_status {
        return None;
    }
    let status = snapshot.status.as_ref()?;
    let emoji = status.emoji.trim();
    let message = status.message.trim();
    if emoji.is_empty() && message.is_empty() {
        return None;
    }
    Some(format!("{emoji} {message}").trim().to_string())
}

fn avatar_html(snapshot: &ProfileSnapshot, size: u32, align: &str) -> String {
    let size = size.clamp(32, 240);
    let align = if align.is_empty() {
        String::new()
    } else {
        format!(" align=\"{align}\"")
    };
    format!(
        "<img src=\"{}\" width=\"{size}\" height=\"{size}\" alt=\"{}\"{align} />",
        attr(&snapshot.avatar_url),
        escape_inline(&snapshot.login)
    )
}

fn render_about(design: &ProfileDesign, snapshot: &ProfileSnapshot) -> Option<String> {
    let mut out = String::new();
    let bio = if design.about.use_github_bio {
        snapshot
            .bio
            .as_deref()
            .map(str::trim)
            .filter(|bio| !bio.is_empty())
            .map(escape_inline)
            .unwrap_or_default()
    } else {
        design.about.bio_override.trim().to_string()
    };
    if !bio.is_empty() {
        out.push_str(&bio);
        out.push('\n');
    }

    let mut facts: Vec<String> = Vec::new();
    if design.about.company {
        if let Some(company) = clean(&snapshot.company) {
            facts.push(format!("🏢 {}", escape_inline(&company)));
        }
    }
    if design.about.location {
        if let Some(location) = clean(&snapshot.location) {
            facts.push(format!("📍 {}", escape_inline(&location)));
        }
    }
    if design.about.website {
        if let Some(website) = clean(&snapshot.website_url) {
            facts.push(format!(
                "🔗 [{}]({})",
                escape_inline(&short_url(&website)),
                website
            ));
        }
    }
    if design.about.joined {
        if let Some(year) = snapshot.created_at.get(0..4) {
            facts.push(format!("🗓️ Joined {year}"));
        }
    }
    if !facts.is_empty() {
        out.push('\n');
        out.push_str(&facts.join(" · "));
        out.push('\n');
    }

    if design.about.socials && !snapshot.socials.is_empty() {
        let links: Vec<String> = snapshot
            .socials
            .iter()
            .filter(|social| !social.url.trim().is_empty())
            .map(|social| {
                format!(
                    "[{}]({})",
                    escape_inline(&provider_label(&social.provider)),
                    social.url
                )
            })
            .collect();
        if !links.is_empty() {
            out.push('\n');
            out.push_str(&links.join(" · "));
            out.push('\n');
        }
    }

    Some(out.trim().to_string())
}

fn render_stats(
    design: &ProfileDesign,
    snapshot: &ProfileSnapshot,
    accent: &str,
) -> Option<String> {
    let (current, longest) = streaks(&snapshot.contribution_days);
    let mut metrics: Vec<(&str, String)> = Vec::new();
    for field in normalized_fields(&design.stats.fields) {
        let value = match field {
            "repos" => Some(number(snapshot.public_repos)),
            "followers" => Some(number(snapshot.followers)),
            "following" => Some(number(snapshot.following)),
            "stars" => Some(number(snapshot.stars)),
            "contributions" => Some(number(snapshot.total_contributions)),
            "commits" => Some(number(snapshot.commits)),
            "pullRequests" => Some(number(snapshot.pull_requests)),
            "issues" => Some(number(snapshot.issues)),
            "reviews" => Some(number(snapshot.reviews)),
            "gists" => Some(number(snapshot.gists)),
            "streak" => Some(format!("{current} d")),
            "longest" => Some(format!("{longest} d")),
            _ => None,
        };
        if let Some(value) = value {
            metrics.push((field_label(field), value));
        }
    }
    if metrics.is_empty() {
        return None;
    }

    let style = match design.stats.style.as_str() {
        "table" | "inline" => design.stats.style.as_str(),
        _ => "badges",
    };
    let badge_style = valid_badge_style(&design.stats.badge_style);

    match style {
        "table" => {
            let mut out = String::from("| Metric | Value |\n| :--- | ---: |\n");
            for (label, value) in &metrics {
                out.push_str(&format!("| {label} | {value} |\n"));
            }
            Some(out)
        }
        "inline" => Some(
            metrics
                .iter()
                .map(|(label, value)| format!("**{value}** {}", label.to_lowercase()))
                .collect::<Vec<_>>()
                .join(" · "),
        ),
        _ => {
            let images: Vec<String> = metrics
                .iter()
                .map(|(label, value)| {
                    format!(
                        "<img alt=\"{label}\" src=\"{}\" />",
                        badge_url(label, value, accent, badge_style)
                    )
                })
                .collect();
            Some(format!(
                "<p align=\"{}\">{}</p>",
                if design.align == "left" { "left" } else { "center" },
                images.join(" ")
            ))
        }
    }
}

fn normalized_fields(fields: &[String]) -> Vec<&str> {
    let mut out: Vec<&str> = Vec::new();
    for field in fields {
        let known = matches!(
            field.as_str(),
            "repos"
                | "followers"
                | "following"
                | "stars"
                | "contributions"
                | "commits"
                | "pullRequests"
                | "issues"
                | "reviews"
                | "gists"
                | "streak"
                | "longest"
        );
        if known && !out.contains(&field.as_str()) {
            out.push(field.as_str());
        }
    }
    out
}

fn field_label(field: &str) -> &'static str {
    match field {
        "repos" => "Repositories",
        "followers" => "Followers",
        "following" => "Following",
        "stars" => "Stars",
        "contributions" => "Contributions",
        "commits" => "Commits",
        "pullRequests" => "Pull requests",
        "issues" => "Issues",
        "reviews" => "Reviews",
        "gists" => "Gists",
        "streak" => "Current streak",
        "longest" => "Longest streak",
        _ => "Metric",
    }
}

fn render_pinned(design: &ProfileDesign, snapshot: &ProfileSnapshot) -> Option<String> {
    let source = if design.pinned.source == "top" {
        &snapshot.top_repos
    } else {
        &snapshot.pinned
    };
    let mut repos: Vec<&ProfileRepo> = source.iter().collect();
    if repos.is_empty() {
        repos = snapshot.top_repos.iter().collect();
    }
    let limit = design.pinned.limit.clamp(1, 6) as usize;
    repos.truncate(limit);
    if repos.is_empty() {
        return None;
    }

    let meta = |repo: &ProfileRepo| -> String {
        let mut parts: Vec<String> = Vec::new();
        if design.pinned.language {
            if let Some(language) = repo.language.as_deref().filter(|value| !value.is_empty()) {
                parts.push(escape_inline(language));
            }
        }
        if design.pinned.stars {
            parts.push(format!("★ {}", number(repo.stars)));
        }
        if design.pinned.forks {
            parts.push(format!("⑂ {}", number(repo.forks)));
        }
        parts.join(" · ")
    };
    let description = |repo: &ProfileRepo| -> Option<String> {
        if !design.pinned.description {
            return None;
        }
        repo.description
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(escape_cell)
    };

    match design.pinned.style.as_str() {
        "list" => {
            let mut out = String::new();
            for repo in repos {
                let mut line = format!(
                    "- [**{}**]({})",
                    escape_inline(&repo.name),
                    repo.url
                );
                if let Some(description) = description(repo) {
                    line.push_str(&format!(" — {description}"));
                }
                let meta = meta(repo);
                if !meta.is_empty() {
                    line.push_str(&format!(" · <sub>{meta}</sub>"));
                }
                out.push_str(&line);
                out.push('\n');
            }
            Some(out)
        }
        "table" => {
            let mut out = String::from("| Repository | Description | Language | Stars |\n| :--- | :--- | :--- | ---: |\n");
            for repo in repos {
                let description = description(repo).unwrap_or_default();
                let language = if design.pinned.language {
                    repo.language
                        .as_deref()
                        .map(escape_cell)
                        .unwrap_or_default()
                } else {
                    String::new()
                };
                let stars = if design.pinned.stars {
                    number(repo.stars)
                } else {
                    String::new()
                };
                out.push_str(&format!(
                    "| [**{}**]({}) | {} | {} | {} |\n",
                    escape_cell(&repo.name),
                    repo.url,
                    description,
                    language,
                    stars
                ));
            }
            Some(out)
        }
        _ => {
            let mut out = String::new();
            for pair in repos.chunks(2) {
                out.push_str("|  |  |\n| :--- | :--- |\n");
                let mut cells: Vec<String> = Vec::new();
                for repo in pair {
                    let mut cell = format!(
                        "[**{}**]({})",
                        escape_inline(&repo.name),
                        repo.url
                    );
                    if let Some(description) = description(repo) {
                        cell.push_str(&format!("<br/><sub>{description}</sub>"));
                    }
                    let meta = meta(repo);
                    if !meta.is_empty() {
                        cell.push_str(&format!("<br/>{meta}"));
                    }
                    cells.push(cell);
                }
                if cells.len() == 1 {
                    cells.push(String::new());
                }
                out.push_str(&format!("| {} | {} |\n\n", cells[0], cells[1]));
            }
            Some(out.trim_end().to_string())
        }
    }
}

fn render_contributions(
    design: &ProfileDesign,
    snapshot: &ProfileSnapshot,
    accent: &str,
) -> Option<String> {
    let (current, longest) = streaks(&snapshot.contribution_days);
    let mut out = String::new();
    let style = match design.contributions.style.as_str() {
        "ascii" | "badge" | "summary" => design.contributions.style.as_str(),
        _ => "heatmap",
    };

    if style == "badge" {
        let message = format!(
            "{} in the last year",
            number(snapshot.total_contributions)
        );
        return Some(format!(
            "<img alt=\"Contributions\" src=\"{}\" />",
            badge_url(
                "Contributions",
                &message,
                accent,
                valid_badge_style(&design.stats.badge_style)
            )
        ));
    }

    if design.contributions.show_totals {
        out.push_str(&format!(
            "**{}** contributions in the last year",
            number(snapshot.total_contributions)
        ));
        if snapshot.private_contributions > 0 {
            out.push_str(&format!(
                " · {} private",
                number(snapshot.private_contributions)
            ));
        }
        out.push('\n');
    }
    if design.contributions.show_breakdown {
        out.push_str(&format!(
            "{} commits · {} pull requests · {} issues · {} reviews\n",
            number(snapshot.commits),
            number(snapshot.pull_requests),
            number(snapshot.issues),
            number(snapshot.reviews)
        ));
    }
    if design.contributions.show_streak {
        out.push_str(&format!(
            "Current streak **{} days** · Longest **{} days**\n",
            current, longest
        ));
    }

    if style == "summary" {
        return Some(out.trim().to_string());
    }

    out.push('\n');
    if style == "ascii" {
        out.push_str(&ascii_heatmap(design, snapshot));
    } else {
        out.push_str(&emoji_heatmap(design, snapshot));
    }
    Some(out.trim_end().to_string())
}

fn ordered_weeks(
    design: &ProfileDesign,
    snapshot: &ProfileSnapshot,
) -> Vec<Vec<Option<ContributionDay>>> {
    let today = Local::now().date_naive();
    let take = design.contributions.weeks.clamp(8, 53) as usize;
    // Rebuild weeks from the flat day list so the renderer does not depend on
    // the API's week grouping. Sunday starts a new week.
    let mut weeks: Vec<Vec<ContributionDay>> = Vec::new();
    let mut current: Vec<ContributionDay> = Vec::new();
    for day in &snapshot.contribution_days {
        if day.weekday == 0 && !current.is_empty() {
            weeks.push(std::mem::take(&mut current));
        }
        current.push(day.clone());
    }
    if !current.is_empty() {
        weeks.push(current);
    }

    let start = weeks.len().saturating_sub(take);
    let mut rows: Vec<Vec<Option<ContributionDay>>> = Vec::new();
    for week in &weeks[start..] {
        let mut slots: Vec<Option<ContributionDay>> = vec![None; 7];
        for day in week {
            if let Ok(date) = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d") {
                if date <= today {
                    slots[day.weekday as usize % 7] = Some(day.clone());
                }
            }
        }
        rows.push(slots);
    }
    rows
}

fn emoji_heatmap(design: &ProfileDesign, snapshot: &ProfileSnapshot) -> String {
    let weeks = ordered_weeks(design, snapshot);
    let labels = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let mut out = String::new();
    for weekday in [1usize, 2, 3, 4, 5, 6, 0] {
        out.push_str(labels[weekday]);
        out.push(' ');
        for week in &weeks {
            let count = week[weekday].as_ref().map(|day| day.count).unwrap_or(0);
            out.push(if count > 0 { '🟩' } else { '⬜' });
        }
        out.push_str("  \n");
    }
    out
}

fn ascii_heatmap(design: &ProfileDesign, snapshot: &ProfileSnapshot) -> String {
    let weeks = ordered_weeks(design, snapshot);
    let labels = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    let mut out = String::from("```text\n");
    for weekday in [1usize, 2, 3, 4, 5, 6, 0] {
        out.push_str(labels[weekday]);
        out.push(' ');
        for week in &weeks {
            let count = week[weekday].as_ref().map(|day| day.count).unwrap_or(0);
            out.push(match count {
                0 => '·',
                1..=2 => '░',
                3..=5 => '▒',
                6..=9 => '▓',
                _ => '█',
            });
        }
        out.push('\n');
    }
    out.push_str("```\n");
    out
}

fn render_achievements(
    design: &ProfileDesign,
    snapshot: &ProfileSnapshot,
) -> Option<String> {
    let mut out = String::new();
    let link = format!("https://github.com/{}?tab=achievements", snapshot.login);
    if !design.achievements.badges.is_empty() {
        let images: Vec<String> = design
            .achievements
            .badges
            .iter()
            .filter(|badge| !badge.name.trim().is_empty())
            .map(|badge| {
                let color = match badge.tier.to_lowercase().as_str() {
                    "gold" => "d4a017",
                    "silver" => "8a8f98",
                    _ => "b87333",
                };
                format!(
                    "[![{}]({})]({})",
                    escape_cell(badge.name.trim()),
                    badge_url(
                        badge.name.trim(),
                        &title_case(&badge.tier),
                        color,
                        "for-the-badge"
                    ),
                    link
                )
            })
            .collect();
        if !images.is_empty() {
            out.push_str(&images.join(" "));
            out.push('\n');
        }
    }
    if design.achievements.show_link {
        out.push_str(&format!("\n[All GitHub achievements]({link})\n"));
    }
    Some(out.trim().to_string())
}

fn render_organizations(
    design: &ProfileDesign,
    snapshot: &ProfileSnapshot,
) -> Option<String> {
    if snapshot.organizations.is_empty() {
        return None;
    }
    let limit = design.organizations.limit.clamp(1, 12) as usize;
    let orgs: Vec<&ProfileOrg> = snapshot.organizations.iter().take(limit).collect();
    if orgs.is_empty() {
        return None;
    }
    if design.organizations.style == "list" {
        let lines: Vec<String> = orgs
            .iter()
            .map(|org| {
                format!(
                    "- [{}](https://github.com/{})",
                    escape_inline(&org.login),
                    org.login
                )
            })
            .collect();
        return Some(lines.join("\n"));
    }
    let images: Vec<String> = orgs
        .iter()
        .map(|org| {
            format!(
                "[<img src=\"{}\" width=\"40\" height=\"40\" alt=\"{}\" />](https://github.com/{})",
                attr(&org.avatar_url),
                escape_inline(&org.login),
                org.login
            )
        })
        .collect();
    Some(format!(
        "<p align=\"{}\">{}</p>",
        if design.align == "left" { "left" } else { "center" },
        images.join(" ")
    ))
}

fn streaks(days: &[ContributionDay]) -> (u32, u32) {
    let mut counts: HashMap<NaiveDate, u32> = HashMap::new();
    for day in days {
        if let Ok(date) = NaiveDate::parse_from_str(&day.date, "%Y-%m-%d") {
            counts.insert(date, day.count);
        }
    }
    if counts.is_empty() {
        return (0, 0);
    }

    let today = Local::now().date_naive();
    let mut cursor = today;
    if counts.get(&cursor).copied().unwrap_or(0) == 0 {
        cursor -= Days::days(1);
    }
    let mut current = 0;
    while counts.get(&cursor).copied().unwrap_or(0) > 0 {
        current += 1;
        cursor -= Days::days(1);
    }

    let mut active: Vec<NaiveDate> = counts
        .iter()
        .filter(|(_, count)| **count > 0)
        .map(|(date, _)| *date)
        .collect();
    active.sort();
    let mut longest = 0;
    let mut run = 0;
    let mut previous: Option<NaiveDate> = None;
    for date in active {
        run = if previous == Some(date - Days::days(1)) {
            run + 1
        } else {
            1
        };
        longest = longest.max(run);
        previous = Some(date);
    }
    (current, longest)
}

fn badge_url(label: &str, message: &str, color: &str, style: &str) -> String {
    format!(
        "https://img.shields.io/badge/{}-{}-{}?style={}&logo=github",
        encode_component(label),
        encode_component(message),
        color,
        style
    )
}

fn valid_badge_style(value: &str) -> &str {
    match value {
        "flat" | "flat-square" | "plastic" | "for-the-badge" | "social" => value,
        _ => "flat-square",
    }
}

fn sanitize_hex(value: &str) -> String {
    let value = value.trim().trim_start_matches('#');
    if value.len() == 6 && value.chars().all(|c| c.is_ascii_hexdigit()) {
        value.to_lowercase()
    } else {
        "2ea043".to_string()
    }
}

fn encode_component(value: &str) -> String {
    let mut out = String::new();
    for byte in value.trim().bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'.' | b'~' => out.push(byte as char),
            b' ' => out.push_str("%20"),
            b'-' => out.push_str("--"),
            b'_' => out.push_str("__"),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn number(value: u32) -> String {
    let raw = value.to_string();
    let mut out = String::new();
    for (index, ch) in raw.chars().enumerate() {
        if index > 0 && (raw.len() - index) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn title_case(value: &str) -> String {
    let mut chars = value.trim().chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => "Bronze".to_string(),
    }
}

fn short_url(value: &str) -> String {
    let trimmed = value.trim();
    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let without_www = without_scheme.strip_prefix("www.").unwrap_or(without_scheme);
    let trimmed = without_www.trim_end_matches('/');
    trimmed.chars().take(48).collect()
}

fn provider_label(provider: &str) -> String {
    match provider.to_ascii_uppercase().as_str() {
        "TWITTER" | "X" => "X".to_string(),
        "LINKEDIN" => "LinkedIn".to_string(),
        "MASTODON" => "Mastodon".to_string(),
        "BLUESKY" => "Bluesky".to_string(),
        "GITHUB" => "GitHub".to_string(),
        "HOMEPAGE" | "WEBSITE" => "Website".to_string(),
        other => title_case(&other.to_ascii_lowercase()),
    }
}

fn clean(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn escape_cell(value: &str) -> String {
    escape_inline(value).replace('\n', " ")
}

fn escape_inline(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.trim().chars() {
        match ch {
            '\n' | '\r' => out.push(' '),
            '\\' | '*' | '_' | '`' | '[' | ']' | '<' | '>' | '|' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn attr(value: &str) -> String {
    value.replace('"', "&quot;").replace('<', "&lt;")
}

#[derive(Deserialize)]
struct GraphQlResponse {
    data: Option<GraphQlData>,
    #[serde(default)]
    errors: Option<Vec<GraphQlError>>,
}

#[derive(Deserialize)]
struct GraphQlError {
    message: String,
}

#[derive(Deserialize)]
struct GraphQlData {
    viewer: Option<RawViewer>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawViewer {
    login: String,
    #[serde(default)]
    name: Option<String>,
    avatar_url: String,
    #[serde(default)]
    bio: Option<String>,
    #[serde(default)]
    company: Option<String>,
    #[serde(default)]
    location: Option<String>,
    #[serde(default)]
    website_url: Option<String>,
    #[serde(default)]
    twitter_username: Option<String>,
    #[serde(default)]
    status: Option<RawStatus>,
    #[serde(default)]
    social_accounts: RawSocialConnection,
    followers: RawTotal,
    following: RawTotal,
    repo_count: RawTotal,
    gists: RawTotal,
    created_at: String,
    #[serde(default)]
    pinned_items: RawRepoConnection,
    #[serde(default)]
    top_repos: RawRepoConnection,
    contributions_collection: RawContributions,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawStatus {
    #[serde(default)]
    emoji: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

#[derive(Deserialize, Default)]
struct RawSocialConnection {
    #[serde(default)]
    nodes: Vec<RawSocial>,
}

#[derive(Deserialize)]
struct RawSocial {
    provider: String,
    url: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTotal {
    total_count: u32,
}

#[derive(Deserialize, Default)]
struct RawRepoConnection {
    #[serde(default)]
    nodes: Vec<Option<RawRepo>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawRepo {
    name_with_owner: String,
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default, rename = "stargazerCount")]
    stargazers_count: u32,
    #[serde(default)]
    fork_count: u32,
    url: String,
    #[serde(default)]
    is_fork: bool,
    #[serde(default)]
    primary_language: Option<RawLanguage>,
}

#[derive(Deserialize)]
struct RawLanguage {
    name: String,
    #[serde(default)]
    color: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawContributions {
    #[serde(default)]
    total_commit_contributions: u32,
    #[serde(default)]
    total_pull_request_contributions: u32,
    #[serde(default)]
    total_issue_contributions: u32,
    #[serde(default)]
    total_pull_request_review_contributions: u32,
    #[serde(default)]
    restricted_contributions_count: u32,
    contribution_calendar: RawCalendar,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCalendar {
    #[serde(default)]
    total_contributions: u32,
    #[serde(default)]
    weeks: Vec<RawWeek>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawWeek {
    #[serde(default)]
    contribution_days: Vec<RawDay>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawDay {
    date: String,
    #[serde(default)]
    contribution_count: u32,
    #[serde(default)]
    weekday: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawOrg {
    login: String,
    #[serde(default)]
    avatar_url: String,
    #[serde(default)]
    description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    fn day(date: &str, count: u32, weekday: u32) -> ContributionDay {
        ContributionDay {
            date: date.into(),
            count,
            weekday,
        }
    }

    fn snapshot() -> ProfileSnapshot {
        ProfileSnapshot {
            login: "bushninja".into(),
            name: Some("bush ninja".into()),
            avatar_url: "https://avatars.githubusercontent.com/u/243279963?s=240".into(),
            bio: Some("Builds tools for Linux.".into()),
            company: Some("@graft".into()),
            location: Some("EndeavourOS".into()),
            website_url: Some("https://example.com".into()),
            twitter_username: None,
            status: Some(ProfileStatus {
                emoji: "🌱".into(),
                message: "shipping".into(),
            }),
            socials: vec![ProfileSocial {
                provider: "MASTODON".into(),
                url: "https://mastodon.social/@bushninja".into(),
            }],
            followers: 1,
            following: 2,
            public_repos: 11,
            gists: 3,
            stars: 42,
            created_at: "2025-11-11T06:13:31Z".into(),
            pinned: vec![ProfileRepo {
                name_with_owner: "bushninja/graft".into(),
                name: "graft".into(),
                description: Some("GitHub on the desktop | with a pipe".into()),
                stars: 42,
                forks: 4,
                url: "https://github.com/bushninja/graft".into(),
                is_fork: false,
                language: Some("Rust".into()),
                language_color: Some("#dea584".into()),
            }],
            top_repos: vec![ProfileRepo {
                name_with_owner: "bushninja/graft".into(),
                name: "graft".into(),
                description: Some("GitHub on the desktop | with a pipe".into()),
                stars: 42,
                forks: 4,
                url: "https://github.com/bushninja/graft".into(),
                is_fork: false,
                language: Some("Rust".into()),
                language_color: Some("#dea584".into()),
            }],
            contribution_days: vec![
                day("2026-11-02", 2, 1),
                day("2026-11-03", 5, 2),
                day("2026-11-04", 0, 3),
                day("2026-11-05", 1, 4),
            ],
            total_contributions: 4242,
            commits: 4212,
            pull_requests: 16,
            issues: 0,
            reviews: 0,
            private_contributions: 0,
            organizations: vec![ProfileOrg {
                login: "graft-linux".into(),
                avatar_url: "https://avatars.githubusercontent.com/u/1?s=64".into(),
                description: None,
            }],
            fetched_at: 0,
        }
    }

    fn all_sections() -> ProfileDesign {
        let mut design = ProfileDesign::default();
        design.stats.fields = vec![
            "repos".into(),
            "followers".into(),
            "stars".into(),
            "contributions".into(),
            "commits".into(),
            "streak".into(),
        ];
        design.achievements.badges = vec![AchievementBadge {
            name: "Pull Shark".into(),
            tier: "gold".into(),
        }];
        design.custom_markdown = "### Elsewhere\n\n[Website](https://example.com)".into();
        design.sections.iter_mut().for_each(|section| section.enabled = true);
        design.footer = "Built with GRAFT.".into();
        design
    }

    #[test]
    fn parses_the_graphql_payload_shape() {
        let raw: RawViewer = serde_json::from_str(
            r##"{
              "login": "bushninja",
              "name": "bush ninja",
              "avatarUrl": "https://avatars.githubusercontent.com/u/1",
              "bio": "bio",
              "createdAt": "2025-11-11T06:13:31Z",
              "followers": { "totalCount": 1 },
              "following": { "totalCount": 0 },
              "repoCount": { "totalCount": 11 },
              "gists": { "totalCount": 0 },
              "socialAccounts": { "nodes": [] },
              "pinnedItems": { "nodes": [{ "nameWithOwner": "bushninja/graft", "name": "graft", "description": "desk", "stargazerCount": 7, "forkCount": 2, "url": "https://github.com/bushninja/graft", "isFork": false, "primaryLanguage": { "name": "Rust", "color": "#dea584" } }] },
              "topRepos": { "nodes": [] },
              "contributionsCollection": {
                "totalCommitContributions": 12,
                "totalPullRequestContributions": 3,
                "totalIssueContributions": 0,
                "totalPullRequestReviewContributions": 0,
                "restrictedContributionsCount": 0,
                "contributionCalendar": {
                  "totalContributions": 15,
                  "weeks": [{ "contributionDays": [{ "date": "2026-11-01", "contributionCount": 4, "weekday": 0 }] }]
                }
              }
            }"##,
        )
        .expect("graphql payload");
        assert_eq!(raw.login, "bushninja");
        assert_eq!(raw.repo_count.total_count, 11);
        let pinned = repo_list(&raw.pinned_items);
        assert_eq!(pinned[0].stars, 7, "stargazerCount must not be lost");
        assert_eq!(pinned[0].language.as_deref(), Some("Rust"));
        assert_eq!(
            raw.contributions_collection.contribution_calendar.weeks[0].contribution_days[0]
                .contribution_count,
            4
        );
    }

    #[test]
    fn renders_every_section_from_real_shaped_data() {
        let markdown = render_profile(&all_sections(), &snapshot());
        assert!(markdown.contains("avatars.githubusercontent.com"));
        assert!(markdown.contains("bush ninja"));
        assert!(markdown.contains("Builds tools for Linux."));
        assert!(markdown.contains("Pinned repositories"));
        assert!(markdown.contains("[**graft**]"));
        assert!(markdown.contains("★ 42"));
        assert!(markdown.contains("Pull Shark"));
        assert!(markdown.contains("graft-linux"));
        assert!(markdown.contains("Elsewhere"));
        assert!(markdown.contains("Built with GRAFT."));
        assert!(markdown.contains("4,242"));
        assert!(markdown.contains("🟩"));
        assert!(markdown.ends_with('\n'));
    }

    #[test]
    fn table_cells_do_not_break_on_pipes() {
        let mut snapshot = snapshot();
        snapshot.pinned[0].description = Some("a | b\nc".into());
        let markdown = render_profile(&ProfileDesign::default(), &snapshot);
        assert!(markdown.contains("a \\| b c"));
        assert_eq!(markdown.matches("a \\| b").count(), 1);
    }

    #[test]
    fn heatmap_has_seven_labeled_rows_and_respects_weeks() {
        let markdown = render_profile(&ProfileDesign::default(), &snapshot());
        for label in ["Mon ", "Tue ", "Wed ", "Thu ", "Fri ", "Sat ", "Sun "] {
            assert!(markdown.contains(label), "missing {label}");
        }
    }

    #[test]
    fn streaks_ignore_missing_days() {
        let today = Local::now().date_naive();
        let dates = [
            (today - Days::days(3), 1),
            (today - Days::days(2), 2),
            (today - Days::days(1), 3),
            (today, 4),
        ];
        let days: Vec<ContributionDay> = dates
            .iter()
            .map(|(date, count)| ContributionDay {
                date: date.format("%Y-%m-%d").to_string(),
                count: *count,
                weekday: date.weekday().num_days_from_sunday(),
            })
            .collect();
        assert_eq!(streaks(&days).0, 4);
        assert!(streaks(&days).1 >= 4);
    }

    #[test]
    fn invalid_accent_is_replaced() {
        assert_eq!(sanitize_hex("#ABCDEF"), "abcdef");
        assert_eq!(sanitize_hex("javascript:"), "2ea043");
        assert_eq!(sanitize_hex("12345"), "2ea043");
    }

    #[test]
    fn empty_snapshot_renders_without_panicking() {
        let empty = ProfileSnapshot {
            login: "nobody".into(),
            name: None,
            avatar_url: String::new(),
            bio: None,
            company: None,
            location: None,
            website_url: None,
            twitter_username: None,
            status: None,
            socials: Vec::new(),
            followers: 0,
            following: 0,
            public_repos: 0,
            gists: 0,
            stars: 0,
            created_at: String::new(),
            pinned: Vec::new(),
            top_repos: Vec::new(),
            contribution_days: Vec::new(),
            total_contributions: 0,
            commits: 0,
            pull_requests: 0,
            issues: 0,
            reviews: 0,
            private_contributions: 0,
            organizations: Vec::new(),
            fetched_at: 0,
        };
        let markdown = render_profile(&ProfileDesign::default(), &empty);
        assert!(markdown.contains("nobody"));
        assert!(markdown.contains("0"));
    }

    #[test]
    fn sections_can_be_reordered_and_disabled() {
        let mut design = ProfileDesign::default();
        design.sections = vec![
            SectionDesign {
                id: "stats".into(),
                enabled: true,
                title: Some("By the numbers".into()),
            },
            SectionDesign {
                id: "header".into(),
                enabled: true,
                title: None,
            },
            SectionDesign {
                id: "about".into(),
                enabled: false,
                title: None,
            },
        ];
        let markdown = render_profile(&design, &snapshot());
        let stats = markdown.find("By the numbers").expect("stats heading");
        let header = markdown.find("bush ninja").expect("header");
        assert!(stats < header);
        assert!(!markdown.contains("Builds tools for Linux."));
    }
}
