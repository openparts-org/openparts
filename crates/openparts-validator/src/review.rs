//! The `review` block of a Part, Device, Package or Source (Canonical
//! Data Specification section 26.1): when present it must say who
//! reviewed the document (GitHub logins) and in which openparts-data
//! pull request.
//!
//! What can't be checked here: that a reviewed document hasn't changed
//! since. That is a rule for whoever edits it (drop the block), and the
//! pull request's fact table shows reviewers what changed.

use crate::Diagnostic;
use openparts_core::DocumentReview;
use std::collections::BTreeSet;

/// A GitHub login: 1-39 ASCII letters, digits or hyphens, not starting
/// or ending with a hyphen.
pub fn is_github_login(s: &str) -> bool {
    (1..=39).contains(&s.len())
        && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        && !s.starts_with('-')
        && !s.ends_with('-')
}

/// Problems with `entity`'s `review` block (none when absent).
pub fn check_review(entity: &str, review: Option<&DocumentReview>) -> Vec<Diagnostic> {
    let Some(r) = review else {
        return vec![];
    };
    let mut out = Vec::new();
    let mut bad = |path: &str, message: String| {
        out.push(Diagnostic::error(
            "review.block",
            entity,
            Some(path),
            message,
        ));
    };
    if r.by.is_empty() {
        bad(
            "/review/by",
            "a reviewed document needs `by`: the GitHub login(s) of its reviewer(s)".into(),
        );
    }
    let mut seen = BTreeSet::new();
    for (i, login) in r.by.iter().enumerate() {
        if !is_github_login(login) {
            bad(
                &format!("/review/by/{i}"),
                format!("\"{login}\" is not a GitHub login (letters, digits and hyphens, no `@`)"),
            );
        } else if !seen.insert(login.to_ascii_lowercase()) {
            bad(
                &format!("/review/by/{i}"),
                format!("\"{login}\" is listed twice"),
            );
        }
    }
    match r.pr {
        None => bad(
            "/review/pr",
            "a reviewed document needs `pr`: the openparts-data pull request that reviewed it"
                .into(),
        ),
        Some(0) => bad(
            "/review/pr",
            "`pr` is a pull request number (1 or more)".into(),
        ),
        Some(_) => {}
    }
    if let Some(date) = &r.date {
        if !crate::cross::is_date(date, 3) {
            bad("/review/date", format!("date \"{date}\" is not YYYY-MM-DD"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn review(yaml: &str) -> DocumentReview {
        serde_yaml::from_str(yaml).unwrap()
    }

    fn problems(yaml: &str) -> Vec<String> {
        check_review("part:ex/A", Some(&review(yaml)))
            .into_iter()
            .map(|d| format!("{} {}", d.field_path.unwrap_or_default(), d.message))
            .collect()
    }

    #[test]
    fn absent_and_complete_blocks_pass() {
        assert!(check_review("part:ex/A", None).is_empty());
        assert!(
            problems("status: reviewed\nby: [alice, bob-2]\npr: 7\ndate: 2026-11-20\n").is_empty()
        );
        assert!(problems("status: reviewed\nby: [alice]\npr: 7\n").is_empty());
    }

    #[test]
    fn by_and_pr_are_required() {
        let p = problems("status: reviewed\n");
        assert_eq!(p.len(), 2, "{p:?}");
        assert!(p[0].starts_with("/review/by "), "{p:?}");
        assert!(p[1].starts_with("/review/pr "), "{p:?}");
    }

    #[test]
    fn malformed_values_are_reported() {
        let p = problems(
            "status: reviewed\nby: ['@alice', Alice, alice, '-x']\npr: 0\ndate: 2026-13-01\n",
        );
        let paths: Vec<&str> = p.iter().map(|s| s.split(' ').next().unwrap()).collect();
        assert_eq!(
            paths,
            [
                "/review/by/0",
                "/review/by/2",
                "/review/by/3",
                "/review/pr",
                "/review/date"
            ],
            "{p:?}"
        );
    }

    #[test]
    fn logins() {
        assert!(is_github_login("jijinbei"));
        assert!(is_github_login("a-b-c"));
        assert!(!is_github_login(""));
        assert!(!is_github_login("a_b"));
        assert!(!is_github_login(&"a".repeat(40)));
    }
}
