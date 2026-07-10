//! The interactive display filter of the job list.
//!
//! A [`JobFilter`] narrows the *visible* job rows client-side; it is
//! purely a display concern (sorting, notifications and the squeue
//! command see the unfiltered job list). A job matches when any of the
//! configured table columns' cell texts matches the filter input:
//!
//! * plain input is matched as a case-insensitive substring,
//! * input that contains regex metacharacters AND compiles as a regex
//!   is matched as a case-insensitive regex (e.g. "gpu|cpu" or
//!   "train.*8"); input that only looks like a regex but does not
//!   compile (e.g. "c++") falls back to a literal substring match.
//!
//! For the status column the compact Slurm code ("PD", "R", ...) is
//! matched in addition to the long status text, so filtering for "PD"
//! finds pending jobs.

use regex::{Regex, RegexBuilder};

use crate::columns::JobColumn;
use crate::job::Job;

/// Characters that make an input "look like" a regex. Only input
/// containing at least one of them is tried as a regex; everything
/// else is a literal substring (so "c-c++" or "job.sh" style inputs
/// with no metacharacters never surprise the user).
const REGEX_METACHARACTERS: &[char] = &[
    '.', '*', '+', '?', '^', '$', '|', '(', ')', '[', ']', '{', '}', '\\',
];

/// How the filter input is matched against a cell text.
#[derive(Debug, Clone)]
enum Matcher {
    /// Case-insensitive substring (the needle is stored lowercased).
    Substring(String),
    /// A compiled case-insensitive regex.
    Regex(Regex),
}

/// An active display filter: the raw input, the columns to match
/// against and the compiled matcher.
#[derive(Debug, Clone)]
pub struct JobFilter {
    text: String,
    columns: Vec<JobColumn>,
    matcher: Matcher,
}

impl JobFilter {
    /// Builds a filter from the prompt input and the configured table
    /// columns. Returns `None` for empty input (= no filter).
    pub fn new(text: &str, columns: &[JobColumn]) -> Option<JobFilter> {
        if text.is_empty() {
            return None;
        }
        Some(JobFilter {
            text: text.to_string(),
            columns: columns.to_vec(),
            matcher: build_matcher(text),
        })
    }

    /// The raw filter input (shown in the filter indicator).
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Whether the job matches the filter in any configured column.
    pub fn matches(&self, job: &Job) -> bool {
        self.columns.iter().any(|column| {
            if self.matches_text(&column.cell(job)) {
                return true;
            }
            // let compact Slurm codes ("PD", "R", "CD", ...) match the
            // status column in addition to the long text
            *column == JobColumn::Status && self.matches_text(job.status.abbrev())
        })
    }

    /// Whether one cell text matches the filter input.
    fn matches_text(&self, text: &str) -> bool {
        match &self.matcher {
            Matcher::Substring(needle) => text.to_lowercase().contains(needle),
            Matcher::Regex(regex) => regex.is_match(text),
        }
    }
}

/// Compiles the matcher for the given input: a case-insensitive regex
/// when the input contains regex metacharacters and compiles, a
/// literal case-insensitive substring otherwise.
fn build_matcher(text: &str) -> Matcher {
    if text.contains(REGEX_METACHARACTERS) {
        if let Ok(regex) = RegexBuilder::new(text).case_insensitive(true).build() {
            return Matcher::Regex(regex);
        }
    }
    Matcher::Substring(text.to_lowercase())
}

// ====================================================================
//  TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::JobStatus;

    /// A running job on the "gpu" partition named "train_model".
    fn train_job() -> Job {
        Job::new(
            "424242",
            "train_model",
            JobStatus::Running,
            "12:34:56",
            "gpu",
            2,
            "/work",
            "run.sh",
            None,
        )
    }

    fn filter(text: &str) -> JobFilter {
        JobFilter::new(text, &JobColumn::defaults()).unwrap()
    }

    #[test]
    fn test_empty_input_is_no_filter() {
        assert!(JobFilter::new("", &JobColumn::defaults()).is_none());
    }

    #[test]
    fn test_substring_matches_across_configured_columns() {
        let job = train_job();
        // name, partition, id and status cells all match
        assert!(filter("train").matches(&job));
        assert!(filter("gpu").matches(&job));
        assert!(filter("4242").matches(&job));
        assert!(filter("Running").matches(&job));
        // no cell contains this
        assert!(!filter("cpu").matches(&job));
    }

    #[test]
    fn test_matching_is_case_insensitive() {
        let job = train_job();
        assert!(filter("TRAIN").matches(&job));
        assert!(filter("Gpu").matches(&job));
        assert!(filter("rUnNiNg").matches(&job));
    }

    #[test]
    fn test_status_abbreviation_matches_the_status_column() {
        let mut job = train_job();
        job.status = JobStatus::Pending;
        // both the long text and the compact Slurm code match
        assert!(filter("Pending").matches(&job));
        assert!(filter("pd").matches(&job));
        assert!(!filter("R ").matches(&job));
    }

    #[test]
    fn test_only_configured_columns_are_matched() {
        let mut job = train_job();
        job.account = "physics".to_string();
        // "account" is not in the default column set ...
        assert!(!filter("physics").matches(&job));
        // ... but matches once the column is configured
        let columns = vec![JobColumn::Id, JobColumn::Account];
        assert!(JobFilter::new("physics", &columns).unwrap().matches(&job));
    }

    #[test]
    fn test_input_with_metacharacters_is_matched_as_regex() {
        let job = train_job();
        // alternation: "gpu|cpu" matches the gpu partition
        assert!(filter("gpu|cpu").matches(&job));
        // anchors and wildcards work (case-insensitively)
        assert!(filter("^TRAIN.*model$").matches(&job));
        assert!(!filter("^model").matches(&job));
        // a regex that matches nothing filters the job out
        assert!(!filter("^cpu$").matches(&job));
    }

    #[test]
    fn test_valid_regex_input_is_not_matched_literally() {
        let mut job = train_job();
        // "a+b" compiles as a regex ("one or more 'a', then 'b'"), so
        // it matches "aab" but not the literal text "a+b"
        job.name = "a+b".to_string();
        assert!(!filter("a+b").matches(&job));
        job.name = "aab".to_string();
        assert!(filter("a+b").matches(&job));
    }

    #[test]
    fn test_invalid_regex_falls_back_to_literal_substring() {
        let mut job = train_job();
        job.name = "c++_build".to_string();
        // "c++" does not compile as a regex, so it matches literally
        assert!(filter("c++").matches(&job));
        assert!(!filter("c++x").matches(&job));
        // unbalanced brackets are literal, too
        job.name = "array[5".to_string();
        assert!(filter("array[5").matches(&job));
    }
}
