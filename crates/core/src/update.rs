//! Whether a newer release exists, from GitHub's answer for the latest
//! release. Pinwheel only says so: it never downloads or installs anything.

use serde::{Deserialize, Serialize};

/// The repository the releases come from.
pub const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Release {
    /// Such as "0.3.0".
    pub version: String,
    /// The release page.
    pub url: String,
}

/// The path of GitHub's API for the latest release, which leaves out drafts
/// and pre-releases.
pub fn latest_release_path() -> String {
    let repository = REPOSITORY.trim_start_matches("https://github.com/");
    format!("/repos/{repository}/releases/latest")
}

/// The release GitHub answered with, when it is newer than `current`. An
/// answer that is not a release, such as a proxy's page, is an error. So is
/// a newer release whose page is not on this repository, as the page opens
/// in the browser.
pub fn newer_release(json: &str, current: &str) -> Result<Option<Release>, String> {
    #[derive(Deserialize)]
    struct Latest {
        tag_name: String,
        html_url: String,
    }
    let latest: Latest = serde_json::from_str(json).map_err(|e| format!("the answer is not a release: {e}"))?;
    let version = latest.tag_name.trim_start_matches('v');
    let newer = matches!((parse(version), parse(current)), (Some(theirs), Some(ours)) if theirs > ours);
    if !newer {
        return Ok(None);
    }
    if !latest.html_url.starts_with(&format!("{REPOSITORY}/releases/")) {
        return Err(format!(
            "the release page is not on this repository: {}",
            latest.html_url
        ));
    }
    Ok(Some(Release {
        version: version.into(),
        url: latest.html_url,
    }))
}

/// "1.2.3" as numbers. Anything else, a pre-release included, is `None`.
fn parse(version: &str) -> Option<(u32, u32, u32)> {
    let mut parts = version.split('.').map(str::parse);
    let parsed = (parts.next()?.ok()?, parts.next()?.ok()?, parts.next()?.ok()?);
    parts.next().is_none().then_some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn answer(tag: &str) -> String {
        format!(r#"{{"tag_name":"{tag}","html_url":"{REPOSITORY}/releases/tag/{tag}","assets":[]}}"#)
    }

    #[test]
    fn finds_a_newer_release() {
        let release = newer_release(&answer("v0.10.0"), "0.9.3").unwrap().unwrap();
        assert_eq!(release.version, "0.10.0");
        assert_eq!(release.url, format!("{REPOSITORY}/releases/tag/v0.10.0"));
    }

    #[test]
    fn ignores_the_same_or_an_older_release() {
        assert_eq!(newer_release(&answer("v0.2.0"), "0.2.0"), Ok(None));
        assert_eq!(newer_release(&answer("v0.1.9"), "0.2.0"), Ok(None));
        assert_eq!(newer_release(&answer("v1.0.0-rc.1"), "0.2.0"), Ok(None));
    }

    #[test]
    fn fails_on_what_is_not_a_release() {
        assert!(newer_release(r#"{"message":"API rate limit exceeded"}"#, "0.2.0").is_err());
        assert!(newer_release("<html>", "0.2.0").is_err());
    }

    #[test]
    fn fails_on_a_page_elsewhere() {
        let json = r#"{"tag_name":"v9.0.0","html_url":"https://example.com/releases/v9"}"#;
        assert!(newer_release(json, "0.2.0").is_err());
    }

    #[test]
    fn asks_for_this_repository() {
        assert_eq!(
            latest_release_path(),
            "/repos/KewinDousse/pinwheel-audio/releases/latest"
        );
    }
}
