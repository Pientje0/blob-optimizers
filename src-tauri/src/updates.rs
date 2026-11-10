//! Update checking against the GitHub releases of a configured repository.
//!
//! The check is read-only: it asks GitHub for the latest published release, compares its tag with
//! the running version and, if newer, offers the release page. Nothing is downloaded or installed
//! automatically; the user runs the new installer themselves (settings and backups are kept).

use std::cmp::Ordering;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::{AppError, AppResult};
use crate::util::powershell;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    pub current: String,
    pub latest: String,
    pub available: bool,
    /// Release page on github.com.
    pub url: String,
    pub name: String,
    pub notes: String,
}

#[derive(Deserialize)]
struct Release {
    #[serde(default)]
    tag: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    status: Option<i32>,
}

/// `owner/repo` with GitHub's allowed characters only (it is interpolated into a URL).
pub fn valid_repo(repo: &str) -> bool {
    let mut parts = repo.split('/');
    let (Some(owner), Some(name), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let ok = |s: &str, extra: &str| !s.is_empty() && s.len() <= 100 && s.chars().all(|c| c.is_ascii_alphanumeric() || extra.contains(c));
    ok(owner, "-") && ok(name, "-_.") && name != "." && name != ".."
}

/// Numeric version parts of a tag such as `v1.2.3` or `1.2.0-beta.1` (pre-release suffix ignored).
fn parse_version(tag: &str) -> Option<Vec<u64>> {
    let core = tag.trim().trim_start_matches(['v', 'V']).split(['-', '+']).next()?;
    let parts: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| !p.is_empty())
}

/// Compares two versions, treating missing trailing parts as zero (1.2 == 1.2.0).
pub fn compare_versions(a: &str, b: &str) -> Option<Ordering> {
    let (mut a, mut b) = (parse_version(a)?, parse_version(b)?);
    let n = a.len().max(b.len());
    a.resize(n, 0);
    b.resize(n, 0);
    Some(a.cmp(&b))
}

fn script(repo: &str) -> String {
    format!(
        r#"
try {{
  [Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12
  $r = Invoke-RestMethod -UseBasicParsing -TimeoutSec 15 -Uri 'https://api.github.com/repos/{repo}/releases/latest' -Headers @{{ 'User-Agent' = 'BlobOptimizers-UpdateCheck'; 'Accept' = 'application/vnd.github+json' }}
  [pscustomobject]@{{ tag = [string]$r.tag_name; url = [string]$r.html_url; name = [string]$r.name; notes = [string]$r.body }} | ConvertTo-Json -Compress
}} catch {{
  $code = $null
  if ($_.Exception.Response) {{ $code = [int]$_.Exception.Response.StatusCode }}
  [pscustomobject]@{{ error = $_.Exception.Message; status = $code }} | ConvertTo-Json -Compress
}}
"#
    )
}

pub fn check(current: &str, repo: &str) -> AppResult<UpdateInfo> {
    if !valid_repo(repo) {
        return Err(AppError::Other("Set the release repository (owner/name) in Settings first.".into()));
    }
    let r: Release = powershell::run_json(&script(repo), Duration::from_secs(30))?;
    if let Some(err) = r.error {
        return Err(AppError::Other(match r.status {
            Some(404) => format!("No published release found for {repo}."),
            Some(403) => "GitHub's rate limit was reached. Try again in an hour.".into(),
            _ => format!("Could not reach GitHub: {err}"),
        }));
    }
    let expected_prefix = format!("https://github.com/{repo}/");
    if !r.url.to_ascii_lowercase().starts_with(&expected_prefix.to_ascii_lowercase()) {
        return Err(AppError::Other("GitHub returned an unexpected release address.".into()));
    }
    let ord = compare_versions(&r.tag, current).ok_or_else(|| AppError::Other(format!("The latest release tag '{}' is not a version number.", r.tag)))?;
    let mut notes = r.notes;
    if notes.chars().count() > 4000 {
        notes = notes.chars().take(4000).collect::<String>() + "…";
    }
    Ok(UpdateInfo {
        current: current.to_string(),
        latest: r.tag.trim_start_matches(['v', 'V']).to_string(),
        available: ord == Ordering::Greater,
        url: r.url,
        name: r.name,
        notes,
    })
}

/// Opens a release page of the configured repository in the default browser.
pub fn open_release_page(repo: &str, url: &str) -> AppResult<()> {
    let prefix = format!("https://github.com/{repo}/");
    let safe = valid_repo(repo)
        && url.to_ascii_lowercase().starts_with(&prefix.to_ascii_lowercase())
        && url.chars().all(|c| c.is_ascii_graphic());
    if !safe {
        return Err(AppError::Other("Refusing to open an address outside the release repository.".into()));
    }
    std::process::Command::new("explorer.exe").arg(url).spawn()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare() {
        assert_eq!(compare_versions("v0.2.0", "0.1.0"), Some(Ordering::Greater));
        assert_eq!(compare_versions("1.2", "1.2.0"), Some(Ordering::Equal));
        assert_eq!(compare_versions("0.1.0", "0.1.1"), Some(Ordering::Less));
        assert_eq!(compare_versions("v1.0.0-beta.1", "1.0.0"), Some(Ordering::Equal));
        assert_eq!(compare_versions("0.10.0", "0.9.9"), Some(Ordering::Greater));
        assert_eq!(compare_versions("nightly", "1.0.0"), None);
    }

    #[test]
    fn repo_names_are_validated() {
        assert!(valid_repo("blob-optimizers/blob-optimizers"));
        assert!(valid_repo("someone/my.repo_1"));
        for bad in ["", "owner", "a/b/c", "o'x/r", "o/r;calc", "o/..", "o/r?x=1", "o /r"] {
            assert!(!valid_repo(bad), "{bad}");
        }
    }

    #[test]
    fn foreign_urls_are_not_opened() {
        assert!(open_release_page("a/b", "https://evil.example/a/b/").is_err());
        assert!(open_release_page("a/b", "https://github.com/a/bc/releases").is_err());
        assert!(open_release_page("a/b", "https://github.com/a/b/x\" & calc").is_err());
    }

    #[test]
    #[ignore = "manual: needs internet; checks a real GitHub repository"]
    fn live_check_against_github() {
        let u = check("0.0.1", "BurntSushi/ripgrep").expect("check");
        println!("{u:?}");
        assert!(u.available && u.url.starts_with("https://github.com/BurntSushi/ripgrep/"));
        assert!(check("0.0.1", "tauri-apps/this-repo-does-not-exist-xyz").unwrap_err().to_string().contains("No published release"));
    }
}
