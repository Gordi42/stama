//! Notifications for job state changes.
//!
//! [`detect_transitions`] diffs two job lists (old vs. new refresh) and
//! reports every job whose status changed. [`notify`] then emits the
//! user-enabled notifications for the *notable* transitions -- a job
//! starting (Pending -> Running) or finishing (Running/Completing ->
//! Completed/Failed/Timeout/Cancelled):
//!
//! * a terminal bell (BEL, `\x07`), rung once per batch so several
//!   simultaneous transitions do not cause a bell storm, and
//! * a desktop notification per transition via the OSC 777 escape
//!   sequence (supported by kitty, foot, WezTerm and Ghostty; other
//!   terminals ignore it).
//!
//! Both are written to the same stream the TUI renders to (the caller
//! passes it in), so they reach the user's terminal even while the
//! alternate screen is active.

use std::collections::HashMap;
use std::io::Write;

use crate::job::{Job, JobStatus};
use crate::user_options::UserOptions;

/// A job whose status changed between two refreshes.
#[derive(Debug, Clone, PartialEq)]
pub struct JobTransition {
    pub id: String,
    pub name: String,
    pub from: JobStatus,
    pub to: JobStatus,
}

/// The transition kinds the user is notified about.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TransitionKind {
    /// Pending -> Running
    Started,
    /// Running/Completing -> a terminal state
    Finished,
}

impl JobTransition {
    /// Classifies the transition; `None` for transitions that are not
    /// worth a notification (e.g. Running -> Completing).
    fn kind(&self) -> Option<TransitionKind> {
        match (&self.from, &self.to) {
            (JobStatus::Pending, JobStatus::Running) => Some(TransitionKind::Started),
            (
                JobStatus::Running | JobStatus::Completing,
                JobStatus::Completed
                | JobStatus::Failed
                | JobStatus::Timeout
                | JobStatus::Cancelled,
            ) => Some(TransitionKind::Finished),
            _ => None,
        }
    }

    /// The human-readable notification text for a notable transition,
    /// `None` for transitions that are not notified.
    fn message(&self) -> Option<String> {
        // strip control characters from the job name so it cannot
        // terminate or corrupt the OSC escape sequence
        let name: String = self.name.chars().filter(|c| !c.is_control()).collect();
        match self.kind()? {
            TransitionKind::Started => Some(format!("Job {} ({}) started", self.id, name)),
            TransitionKind::Finished => Some(format!(
                "Job {} ({}) finished: {}",
                self.id,
                name,
                self.to.to_string().to_uppercase()
            )),
        }
    }
}

/// Diffs the old job list against the new one and returns a transition
/// for every job that is present in both lists with a changed status.
///
/// Jobs seen for the first time produce no transition (this also keeps
/// the very first refresh after startup silent), and jobs that simply
/// disappeared from the list produce no transition either -- a running
/// job may fall out of squeue on completion, which is ambiguous; with
/// `show_completed_jobs` enabled the sacct merge keeps finished jobs in
/// the list, so they show up as real status transitions instead.
pub fn detect_transitions(old_jobs: &[Job], new_jobs: &[Job]) -> Vec<JobTransition> {
    let old_status: HashMap<&str, &JobStatus> = old_jobs
        .iter()
        .map(|job| (job.id.as_str(), &job.status))
        .collect();
    new_jobs
        .iter()
        .filter_map(|job| {
            let from = *old_status.get(job.id.as_str())?;
            if *from == job.status {
                return None;
            }
            Some(JobTransition {
                id: job.id.clone(),
                name: job.name.clone(),
                from: from.clone(),
                to: job.status.clone(),
            })
        })
        .collect()
}

/// Emits the enabled notifications for the notable transitions of a
/// batch to the given writer (the stream the TUI renders to).
///
/// With `notify_bell` the terminal bell is rung once per batch; with
/// `notify_desktop` one OSC 777 desktop notification is emitted per
/// notable transition. Transitions that are not notable (see
/// [`JobTransition::kind`]) are ignored.
pub fn notify(
    writer: &mut impl Write,
    transitions: &[JobTransition],
    options: &UserOptions,
) -> std::io::Result<()> {
    if !options.notify_bell && !options.notify_desktop {
        return Ok(());
    }
    let messages: Vec<String> = transitions
        .iter()
        .filter_map(JobTransition::message)
        .collect();
    if messages.is_empty() {
        return Ok(());
    }
    if options.notify_bell {
        // once per batch, not per transition: several jobs changing
        // state in the same refresh must not cause a bell storm
        writer.write_all(b"\x07")?;
    }
    if options.notify_desktop {
        for message in &messages {
            write!(writer, "\x1b]777;notify;stama;{}\x1b\\", message)?;
        }
    }
    writer.flush()
}

// ====================================================================
// TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Creates a job with the given id, name and status.
    fn job(id: &str, name: &str, status: JobStatus) -> Job {
        Job::new(
            id, name, status, "00:00:00", "main", 1, "/work", "cmd", None,
        )
    }

    /// Creates a transition of the given statuses for a default job.
    fn transition(from: JobStatus, to: JobStatus) -> JobTransition {
        JobTransition {
            id: "12345".to_string(),
            name: "train_model".to_string(),
            from,
            to,
        }
    }

    /// Options with both notification channels enabled.
    fn all_on() -> UserOptions {
        UserOptions {
            notify_bell: true,
            notify_desktop: true,
            ..UserOptions::default()
        }
    }

    // ----------------------------------------------------------------
    // transition detection
    // ----------------------------------------------------------------

    #[test]
    fn detects_started_and_finished_jobs() {
        let old = vec![
            job("1", "a", JobStatus::Pending),
            job("2", "b", JobStatus::Running),
        ];
        let new = vec![
            job("1", "a", JobStatus::Running),
            job("2", "b", JobStatus::Completed),
        ];
        assert_eq!(
            detect_transitions(&old, &new),
            vec![
                JobTransition {
                    id: "1".to_string(),
                    name: "a".to_string(),
                    from: JobStatus::Pending,
                    to: JobStatus::Running,
                },
                JobTransition {
                    id: "2".to_string(),
                    name: "b".to_string(),
                    from: JobStatus::Running,
                    to: JobStatus::Completed,
                },
            ]
        );
    }

    #[test]
    fn unchanged_jobs_produce_no_transition() {
        let old = vec![job("1", "a", JobStatus::Running)];
        let new = vec![job("1", "a", JobStatus::Running)];
        assert_eq!(detect_transitions(&old, &new), vec![]);
    }

    #[test]
    fn first_seen_jobs_produce_no_transition() {
        // a job that was not in the old list (this also covers the very
        // first refresh after startup, where the old list is empty)
        let old = vec![];
        let new = vec![job("1", "a", JobStatus::Running)];
        assert_eq!(detect_transitions(&old, &new), vec![]);

        let old = vec![job("1", "a", JobStatus::Running)];
        let new = vec![
            job("1", "a", JobStatus::Running),
            job("2", "b", JobStatus::Pending),
        ];
        assert_eq!(detect_transitions(&old, &new), vec![]);
    }

    #[test]
    fn disappeared_jobs_produce_no_transition() {
        let old = vec![
            job("1", "a", JobStatus::Running),
            job("2", "b", JobStatus::Pending),
        ];
        let new = vec![];
        assert_eq!(detect_transitions(&old, &new), vec![]);
    }

    // ----------------------------------------------------------------
    // notification emission
    // ----------------------------------------------------------------

    #[test]
    fn bell_rings_once_per_batch() {
        let options = UserOptions {
            notify_bell: true,
            ..UserOptions::default()
        };
        // two notable transitions in one batch -> exactly one bell
        let transitions = vec![
            transition(JobStatus::Pending, JobStatus::Running),
            transition(JobStatus::Running, JobStatus::Failed),
        ];
        let mut out: Vec<u8> = Vec::new();
        notify(&mut out, &transitions, &options).unwrap();
        assert_eq!(out, b"\x07");
    }

    #[test]
    fn desktop_notification_emits_osc_777_per_transition() {
        let options = UserOptions {
            notify_desktop: true,
            ..UserOptions::default()
        };
        let transitions = vec![
            transition(JobStatus::Pending, JobStatus::Running),
            transition(JobStatus::Running, JobStatus::Completed),
        ];
        let mut out: Vec<u8> = Vec::new();
        notify(&mut out, &transitions, &options).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "\x1b]777;notify;stama;Job 12345 (train_model) started\x1b\\\
             \x1b]777;notify;stama;Job 12345 (train_model) finished: COMPLETED\x1b\\"
        );
    }

    #[test]
    fn both_channels_emit_bell_then_escapes() {
        let transitions = vec![transition(JobStatus::Running, JobStatus::Timeout)];
        let mut out: Vec<u8> = Vec::new();
        notify(&mut out, &transitions, &all_on()).unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "\x07\x1b]777;notify;stama;Job 12345 (train_model) finished: TIMEOUT\x1b\\"
        );
    }

    #[test]
    fn disabled_options_emit_nothing() {
        let transitions = vec![transition(JobStatus::Pending, JobStatus::Running)];
        let mut out: Vec<u8> = Vec::new();
        notify(&mut out, &transitions, &UserOptions::default()).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn non_notable_transitions_emit_nothing() {
        // status changes that are neither "started" nor "finished"
        let transitions = vec![
            transition(JobStatus::Running, JobStatus::Completing),
            transition(JobStatus::Unknown, JobStatus::Running),
            transition(JobStatus::Pending, JobStatus::Cancelled),
        ];
        let mut out: Vec<u8> = Vec::new();
        notify(&mut out, &transitions, &all_on()).unwrap();
        assert!(out.is_empty());
    }

    #[test]
    fn completing_to_terminal_state_counts_as_finished() {
        let transitions = vec![transition(JobStatus::Completing, JobStatus::Cancelled)];
        let mut out: Vec<u8> = Vec::new();
        notify(&mut out, &transitions, &all_on()).unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("Job 12345 (train_model) finished: CANCELLED"));
    }

    #[test]
    fn control_characters_are_stripped_from_the_job_name() {
        // a BEL or ESC in the job name must not terminate or corrupt
        // the OSC escape sequence
        let mut transition = transition(JobStatus::Pending, JobStatus::Running);
        transition.name = "evil\x07\x1b]name".to_string();
        let mut out: Vec<u8> = Vec::new();
        notify(
            &mut out,
            &[transition],
            &UserOptions {
                notify_desktop: true,
                ..UserOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "\x1b]777;notify;stama;Job 12345 (evil]name) started\x1b\\"
        );
    }
}
