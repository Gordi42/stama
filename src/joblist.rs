use color_eyre::{eyre::eyre, Result};
use std::process::Command;
use std::sync::Arc;

use crate::columns::JobColumn;
use crate::job::Job;
use crate::notify::{detect_transitions, JobTransition};
use crate::scheduler::{Scheduler, SlurmScheduler};
use crate::update_content::{ContentTick, ContentUpdater, TIMEOUT_ERROR};
use crate::user_options::UserOptions;

/// An enum to handle actions that change the selected job.
#[derive(Debug, Clone)]
pub enum JobListAction {
    Next,
    Previous,
    Select(usize),
    SelectSortCategory(JobColumn),
    NextSortCategory,
    ReverseSortDirection,
    UpdateSqueueCommand(String),
}

/// The outcome of a [`JobList::update_jobs`] tick, used by the app to
/// decide whether an error popup has to be opened.
#[derive(Debug, Clone, PartialEq)]
pub enum UpdateStatus {
    /// The background worker has not delivered new content yet.
    Pending,
    /// New content was applied without errors.
    Success,
    /// New content was applied (or the worker timed out) and there is
    /// an error to surface to the user.
    Error(String),
}

/// The full outcome of a [`JobList::update_jobs`] tick: the status the
/// app uses for error popups plus the job status transitions observed
/// between the old and the new job list (used for notifications).
#[derive(Debug, Clone, PartialEq)]
pub struct UpdateOutcome {
    pub status: UpdateStatus,
    pub transitions: Vec<JobTransition>,
}

/// A struct that contains all the informations about running jobs.
pub struct JobList {
    // The list of jobs.
    pub jobs: Vec<Job>,
    // The index of the selected job.
    selected: usize,
    // A string that contains the details of the selected job.
    // This string is displayed in the job details view.
    job_details: String,
    // A string that contains the log tail of the selected job.
    // This string is displayed in the log view.
    log_tail: String,
    // The column by which the jobs are sorted.
    sort_category: JobColumn,
    // A boolean that indicates whether the jobs are sorted in reverse order.
    reverse: bool,
    // A module that contains the logic for updating the job list.
    content_updater: ContentUpdater,
    // The squeue command to get the job list.
    pub squeue_command: String,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl JobList {
    /// Creates a new JobList using the real Slurm scheduler.
    pub fn new() -> JobList {
        Self::with_scheduler(Arc::new(SlurmScheduler))
    }

    /// Creates a new JobList whose background updater executes all
    /// commands through the given scheduler.
    pub fn with_scheduler(scheduler: Arc<dyn Scheduler>) -> JobList {
        JobList {
            jobs: Vec::new(),
            selected: 0,
            job_details: String::new(),
            log_tail: String::new(),
            sort_category: JobColumn::Id,
            reverse: false,
            content_updater: ContentUpdater::with_scheduler(scheduler),
            squeue_command: match get_user() {
                Some(user) => format!("squeue -u {}", user),
                // if the username cannot be determined, show all jobs
                // instead of filtering by a bogus user name
                None => "squeue".to_string(),
            },
        }
    }
}

impl Default for JobList {
    fn default() -> Self {
        Self::new()
    }
}

/// Returns the username of the current user.
///
/// Tries the `USER` environment variable first and falls back to the
/// `whoami` command (with trailing whitespace trimmed). Returns `None`
/// if neither yields a non-empty name; in that case the caller builds
/// the squeue command without a `-u` filter, which shows all jobs --
/// the least surprising behavior when the user is unknown.
fn get_user() -> Option<String> {
    if let Ok(user) = std::env::var("USER") {
        let user = user.trim();
        if !user.is_empty() {
            return Some(user.to_string());
        }
    }
    if let Ok(output) = Command::new("whoami").output() {
        if output.status.success() {
            let user = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !user.is_empty() {
                return Some(user);
            }
        }
    }
    None
}

// ====================================================================
// GETTERS
// ====================================================================

impl JobList {
    /// Returns the selected job.
    pub fn get_job(&self) -> Option<&Job> {
        self.jobs.get(self.selected)
    }

    /// Returns the details of the selected job.
    pub fn get_job_details(&self) -> &str {
        &self.job_details
    }

    /// Returns the log tail of the selected job.
    pub fn get_log_tail(&self) -> &str {
        &self.log_tail
    }

    /// Returns the index of the selected job.
    pub fn get_index(&self) -> usize {
        self.selected
    }

    /// Returns the column by which the jobs are sorted.
    pub fn get_sort_category(&self) -> &JobColumn {
        &self.sort_category
    }

    /// Returns a boolean that indicates whether the jobs are sorted
    /// in reverse order.
    pub fn is_reverse(&self) -> bool {
        self.reverse
    }

    /// Returns the length of the job list.
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    /// Returns whether the job list is empty.
    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

// ====================================================================
// SETTERS
// ====================================================================

impl JobList {
    /// Sets the index of the selected job.
    /// Returns an error if the index is out of bounds.
    pub fn set_index(&mut self, index: usize) -> Result<()> {
        // first handle the case of an empty job list
        if self.jobs.is_empty() {
            // set the selected index to 0
            self.selected = 0;
            // set the job details and log tail to "No job selected"
            self.job_details = "No job selected".to_string();
            self.log_tail = "No job selected".to_string();
            return Ok(());
        }
        // now handle the case of a non-empty job list
        // check if the index is out of bounds
        if index >= self.jobs.len() {
            return Err(eyre!("Index out of bounds"));
        }
        self.selected = index;

        Ok(())
    }

    /// Sets the job details and log tail to "loading...".
    fn set_loading_text(&mut self) {
        self.job_details = "loading...".to_string();
        self.log_tail = "loading...".to_string();
    }

    /// Selects the job with the given id.
    /// Returns an error if the job with the given id does not exist.
    pub fn select_job_by_id(&mut self, id: String) -> Result<()> {
        // find the index of the job with the given id
        let index = self.jobs.iter().position(|job| job.id == id);
        match index {
            // if the job with the given id exists, set the index
            Some(index) => {
                self.set_index(index)?;
                Ok(())
            }
            // Otherwise, return an error
            None => Err(eyre!("Job with id {} not found", id)),
        }
    }

    /// Handles an action that changes the selected job.
    /// Or changes the sort category or the reverse boolean.
    /// `columns` are the displayed job table columns; Tab
    /// (`NextSortCategory`) cycles through them.
    pub fn handle_joblist_action(&mut self, action: JobListAction, columns: &[JobColumn]) {
        match action {
            JobListAction::Next => self.next(),
            JobListAction::Previous => self.previous(),
            JobListAction::Select(index) => {
                // check if the index is out of bounds
                if index < self.len() {
                    self.set_index(index).unwrap();
                    self.set_loading_text();
                }
            }
            JobListAction::NextSortCategory => {
                self.set_sort_category(self.sort_category.next_in(columns));
            }
            JobListAction::ReverseSortDirection => {
                self.negate_reverse();
            }
            JobListAction::SelectSortCategory(category) => {
                // check if the category is different from the current one
                if category != self.sort_category {
                    self.set_sort_category(category);
                } else {
                    // if the category is the same, negate the reverse boolean
                    self.negate_reverse();
                }
            }
            JobListAction::UpdateSqueueCommand(command) => {
                self.squeue_command = command;
            }
        }
    }

    /// Sets the column by which the jobs are sorted.
    pub fn set_sort_category(&mut self, category: JobColumn) {
        self.sort_category = category;
        // sort the jobs
        self.sort_raw();
        // set the index to the first job
        // unwrap is safe here because set_index(0) is guaranteed to succeed
        self.set_index(0).unwrap();
    }

    /// Negates the reverse boolean.
    pub fn negate_reverse(&mut self) {
        self.reverse = !self.reverse;
        // sort the jobs
        self.sort_raw();
        // set the index to the first job
        // unwrap is safe here because set_index(0) is guaranteed to succeed
        self.set_index(0).unwrap();
    }
}

// ====================================================================
// METHODS
// ====================================================================

impl JobList {
    /// Updates the job list.
    /// Returns whether new content arrived and, if so, whether an error
    /// has to be surfaced to the user (see [`UpdateStatus`]), plus the
    /// job status transitions observed against the previous job list
    /// (see [`UpdateOutcome`]).
    pub fn update_jobs(&mut self, user_options: &UserOptions) -> UpdateOutcome {
        // get the currently selected job to keep it selected after update
        let job: Option<Job> = self.get_job().cloned();
        let command = self.squeue_command.clone();
        let mut transitions = Vec::new();
        // check if the content updater returns a new job list
        let status = match self
            .content_updater
            .tick(job.clone(), command, user_options.clone())
        {
            ContentTick::New(content) => {
                let content = *content;
                // diff the old job list against the fresh one before
                // replacing it, so job status changes can be notified
                transitions = detect_transitions(&self.jobs, &content.job_list);
                self.jobs = content.job_list;
                self.job_details = content.details_text;
                self.log_tail = content.log_text;
                match content.error {
                    Some(error) => UpdateStatus::Error(error),
                    None => UpdateStatus::Success,
                }
            }
            ContentTick::Pending => UpdateStatus::Pending,
            ContentTick::TimedOut => UpdateStatus::Error(TIMEOUT_ERROR.to_string()),
        };
        // sort the job list
        self.sort_raw();
        // try to select the job that was selected before the update
        if let Some(job) = job {
            self.reselect_job(job.id);
        }
        UpdateOutcome {
            status,
            transitions,
        }
    }

    /// Re-selects the job with the given id after the job list changed.
    /// If no job with that id exists anymore, the selection is reset to
    /// the first job. Keeping the old index would silently select a
    /// different job while still showing the stale details of the old
    /// one; resetting makes the id-miss explicit and the details are
    /// refreshed naturally on the next update.
    fn reselect_job(&mut self, id: String) {
        if self.select_job_by_id(id).is_err() {
            // unwrap is safe: set_index(0) always succeeds
            self.set_index(0).unwrap();
        }
    }

    /// Select the next job in the list.
    pub fn next(&mut self) {
        // check if the job list is empty
        if self.jobs.is_empty() {
            return;
        }
        // if the selected job is the last job, select the first job
        let new_index = (self.selected + 1) % self.len();
        // unwrap is safe here because new_index is always in bounds
        // this is guaranteed by the modulo operation and tested below
        self.set_index(new_index).unwrap();
        self.set_loading_text();
    }

    /// Select the previous job in the list.
    pub fn previous(&mut self) {
        // check if the job list is empty
        if self.jobs.is_empty() {
            return;
        }
        let job_count = self.len();
        // if the selected job is the first job, select the last job
        let new_index = (self.selected + job_count - 1) % job_count;
        // unwrap is safe here because new_index is always in bounds
        // this is guaranteed by the modulo operation and tested below
        self.set_index(new_index).unwrap();
        self.set_loading_text();
    }

    /// Raw sorting function that does not update the selected job index.
    fn sort_raw(&mut self) {
        // only sort if there are jobs
        if self.jobs.is_empty() {
            return;
        }
        // sort the job list based on the sort_category
        // secondary sort is based on the id
        match self.sort_category {
            JobColumn::Id => {
                self.jobs.sort_by(|a, b| compare_job_ids(&b.id, &a.id));
            }
            JobColumn::Name => {
                self.jobs.sort_by(|a, b| {
                    a.name
                        .cmp(&b.name)
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            JobColumn::Status => {
                self.jobs.sort_by(|a, b| {
                    a.status
                        .priority()
                        .cmp(&b.status.priority())
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            JobColumn::Time => {
                self.jobs.sort_by(|a, b| {
                    a.time
                        .cmp(&b.time)
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            JobColumn::Partition => {
                self.jobs.sort_by(|a, b| {
                    a.partition
                        .cmp(&b.partition)
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            JobColumn::Nodes => {
                self.jobs.sort_by(|a, b| {
                    b.nodes
                        .cmp(&a.nodes)
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            // like Nodes, the numeric columns sort descending
            // (highest priority / most CPUs first)
            JobColumn::Priority => {
                self.jobs.sort_by(|a, b| {
                    b.priority
                        .cmp(&a.priority)
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            JobColumn::Cpus => {
                self.jobs.sort_by(|a, b| {
                    b.cpus
                        .cmp(&a.cpus)
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            JobColumn::Reason => {
                self.jobs.sort_by(|a, b| {
                    a.reason
                        .as_deref()
                        .unwrap_or("")
                        .cmp(b.reason.as_deref().unwrap_or(""))
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            JobColumn::Account => {
                self.jobs.sort_by(|a, b| {
                    a.account
                        .cmp(&b.account)
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            JobColumn::Qos => {
                self.jobs.sort_by(|a, b| {
                    a.qos
                        .cmp(&b.qos)
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
            JobColumn::NodeList => {
                self.jobs.sort_by(|a, b| {
                    a.nodelist
                        .cmp(&b.nodelist)
                        .then_with(|| compare_job_ids(&a.id, &b.id))
                });
            }
        }
        // reverse the list if needed
        if self.reverse {
            self.jobs.reverse();
        }
    }

    /// Sorts the job list.
    /// Update the selected job index such that the selected job
    /// remains the same.
    pub fn sort(&mut self) {
        // only sort if there are jobs
        if self.jobs.is_empty() {
            return;
        }
        // get the id of the job in focus
        if let Some(job) = self.get_job() {
            let id = job.id.clone();
            self.sort_raw();
            // unwrap is safe here because the job in focus is
            // guaranteed to exist
            self.select_job_by_id(id).unwrap();
        } else {
            self.sort_raw();
        }
    }
}

/// Compares two slurm job ids numerically (ascending).
///
/// Plain ids ("9", "10") are compared by their integer value, so "9"
/// sorts before "10" instead of after it (as a lexicographic comparison
/// would). Array ids ("123_4") are compared by their numeric
/// (job, task) pair. Ids that cannot be parsed numerically are compared
/// as plain strings and ordered after all numeric ids.
fn compare_job_ids(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (parse_job_id(a), parse_job_id(b)) {
        (Some(key_a), Some(key_b)) => key_a.cmp(&key_b).then_with(|| a.cmp(b)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => a.cmp(b),
    }
}

/// Parses the numeric components of a slurm job id.
///
/// "123" parses to `(123, None)` and the array id "123_4" parses to
/// `(123, Some(4))`. For array ids with a non-numeric task part (e.g.
/// the pending range "123_[4-7]") the task component is `None`, which
/// groups them with their base job id before the individual tasks.
/// Returns `None` if the (base) job id is not a number.
fn parse_job_id(id: &str) -> Option<(u64, Option<u64>)> {
    let (job_part, task_part) = match id.split_once('_') {
        Some((job, task)) => (job, Some(task)),
        None => (id, None),
    };
    let job: u64 = job_part.parse().ok()?;
    let task: Option<u64> = task_part.and_then(|task| task.parse().ok());
    Some((job, task))
}

// ====================================================================
// TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::job::JobStatus;

    /// Creates a JobList with three jobs for testing.
    fn create_job_list() -> JobList {
        let mut job_list = JobList::new();
        job_list.jobs.push(Job::new(
            "1",
            "job1",
            JobStatus::Running,
            "00:00:00",
            "partition1",
            1,
            "workdir1",
            "command1",
            None,
        ));
        job_list.jobs.push(Job::new(
            "2",
            "job2",
            JobStatus::Pending,
            "00:00:00",
            "partition1",
            2,
            "workdir2",
            "command2",
            None,
        ));
        job_list.jobs.push(Job::new(
            "3",
            "job3",
            JobStatus::Completing,
            "00:00:00",
            "partition1",
            3,
            "workdir3",
            "command3",
            None,
        ));
        job_list
    }

    #[test]
    fn test_set_index() {
        let mut job_list = create_job_list();

        // Test setting a valid index.
        assert!(job_list.set_index(1).is_ok());
        assert_eq!(job_list.selected, 1);

        // Test setting an invalid index.
        assert!(job_list.set_index(3).is_err());

        // Test setting an index to an empty job list.
        let mut job_list = JobList::new();
        assert!(job_list.set_index(0).is_ok());
        assert_eq!(job_list.selected, 0);
        // Test if the job details and log tail are set to "No job selected".
        assert_eq!(job_list.job_details, "No job selected");
        assert_eq!(job_list.log_tail, "No job selected");
    }

    #[test]
    fn test_select_job_by_id() {
        let mut job_list = create_job_list();

        // Test selecting a job by id.
        assert!(job_list.select_job_by_id("2".to_string()).is_ok());
        assert_eq!(job_list.selected, 1);

        // Test selecting a job by an invalid id.
        assert!(job_list.select_job_by_id("4".to_string()).is_err());
    }

    #[test]
    fn test_next() {
        let mut job_list = create_job_list();

        // Check if the joblist has length 3.
        assert_eq!(job_list.len(), 3);

        // Test selecting the next job.
        job_list.next();
        assert_eq!(job_list.selected, 1);

        // Test if the job details and log tail are set to "loading...".
        assert_eq!(job_list.job_details, "loading...");
        assert_eq!(job_list.log_tail, "loading...");

        // Set the selected job to the last job.
        job_list.selected = 2;
        job_list.next();
        assert_eq!(job_list.selected, 0);
    }

    #[test]
    fn test_previous() {
        let mut job_list = create_job_list();
        // Check if the joblist has length 3.
        assert_eq!(job_list.len(), 3);

        // Test selecting the previous job.
        job_list.previous();
        assert_eq!(job_list.selected, 2);

        // Test if the job details and log tail are set to "loading...".
        assert_eq!(job_list.job_details, "loading...");
        assert_eq!(job_list.log_tail, "loading...");

        job_list.previous();
        assert_eq!(job_list.selected, 1);
    }

    #[test]
    fn test_reverse() {
        let mut job_list = create_job_list();

        // set the selected index to something other than 0
        job_list.selected = 1;

        // Test negating the reverse boolean.
        assert!(!job_list.reverse);
        job_list.negate_reverse();
        assert!(job_list.reverse);
        // check if the selected index is set back to 0
        assert_eq!(job_list.selected, 0);
        job_list.negate_reverse();
        assert!(!job_list.reverse);
    }

    #[test]
    fn test_sort() {
        let mut job_list = create_job_list();

        // Test sorting the job list.
        let job = job_list.get_job().unwrap().clone();
        job_list.sort();
        assert_eq!(job_list.jobs[0].id, "3");
        assert_eq!(job_list.jobs[1].id, "2");
        assert_eq!(job_list.jobs[2].id, "1");
        // check if the selected job remains the same
        assert_eq!(job_list.get_job().unwrap().id, job.id);

        // Test sorting the job list in reverse order.
        job_list.reverse = true;
        job_list.sort();
        assert_eq!(job_list.jobs[0].id, "1");
        assert_eq!(job_list.jobs[1].id, "2");
        assert_eq!(job_list.jobs[2].id, "3");
        // check if the selected job remains the same
        assert_eq!(job_list.get_job().unwrap().id, job.id);

        // There are many more tests that could be added here.
        // For example, tests for sorting by different categories.
        // However, this is sufficient for now.
    }

    /// Creates a job with the given id, status, time and node count.
    fn create_job(id: &str, status: JobStatus, time: &str, nodes: u32) -> Job {
        Job::new(
            id,
            "job",
            status,
            time,
            "partition1",
            nodes,
            "workdir",
            "command",
            None,
        )
    }

    /// Returns the ids of the jobs in the job list in their current order.
    fn job_ids(job_list: &JobList) -> Vec<&str> {
        job_list.jobs.iter().map(|job| job.id.as_str()).collect()
    }

    #[test]
    fn test_compare_job_ids() {
        use std::cmp::Ordering;

        // numeric ids are compared by value, not lexicographically
        assert_eq!(compare_job_ids("9", "10"), Ordering::Less);
        assert_eq!(compare_job_ids("10", "9"), Ordering::Greater);
        assert_eq!(compare_job_ids("10", "10"), Ordering::Equal);

        // array ids are compared by (job, task)
        assert_eq!(compare_job_ids("123_4", "123_10"), Ordering::Less);
        assert_eq!(compare_job_ids("123_4", "124_1"), Ordering::Less);
        // a plain id sorts before its array tasks
        assert_eq!(compare_job_ids("123", "123_1"), Ordering::Less);

        // non-numeric ids sort after all numeric ids, as strings
        assert_eq!(compare_job_ids("abc", "999999"), Ordering::Greater);
        assert_eq!(compare_job_ids("abc", "abd"), Ordering::Less);
    }

    #[test]
    fn test_sort_by_id_numeric() {
        let mut job_list = JobList::new();
        for id in ["9", "10", "123_10", "2", "123_4", "abc"] {
            job_list
                .jobs
                .push(create_job(id, JobStatus::Running, "00:00:00", 1));
        }

        // default id sort is descending (newest job first),
        // with non-numeric ids sorted in front of the numeric ones
        job_list.sort_raw();
        assert_eq!(
            job_ids(&job_list),
            ["abc", "123_10", "123_4", "10", "9", "2"]
        );

        // reversed: ascending numeric order, non-numeric ids last
        job_list.reverse = true;
        job_list.sort_raw();
        assert_eq!(
            job_ids(&job_list),
            ["2", "9", "10", "123_4", "123_10", "abc"]
        );
    }

    #[test]
    fn test_sort_by_status() {
        // create_job_list: id "1" Running, id "2" Pending, id "3" Completing
        let mut job_list = create_job_list();
        job_list.set_sort_category(JobColumn::Status);

        // ascending by status priority:
        // Pending (1) < Running (2) < Completing (3)
        assert_eq!(job_ids(&job_list), ["2", "1", "3"]);
        // the selection is reset to the first job
        assert_eq!(job_list.selected, 0);
    }

    #[test]
    fn test_sort_by_nodes() {
        let mut job_list = JobList::new();
        job_list
            .jobs
            .push(create_job("9", JobStatus::Running, "00:00:00", 2));
        job_list
            .jobs
            .push(create_job("10", JobStatus::Running, "00:00:00", 4));
        job_list
            .jobs
            .push(create_job("11", JobStatus::Running, "00:00:00", 2));
        job_list.set_sort_category(JobColumn::Nodes);

        // descending by node count; ties broken by ascending numeric id
        assert_eq!(job_ids(&job_list), ["10", "9", "11"]);
    }

    #[test]
    fn test_sort_by_priority() {
        let mut job_list = JobList::new();
        // squeue PriorityLong values are large integers; a numeric sort
        // must not compare them lexicographically
        for (id, priority) in [("9", 900u64), ("10", 4294901760), ("11", 900)] {
            let mut job = create_job(id, JobStatus::Running, "00:00:00", 1);
            job.priority = priority;
            job_list.jobs.push(job);
        }
        job_list.set_sort_category(JobColumn::Priority);

        // descending by priority; ties broken by ascending numeric id
        assert_eq!(job_ids(&job_list), ["10", "9", "11"]);

        // reversed: ascending by priority
        job_list.negate_reverse();
        assert_eq!(job_ids(&job_list), ["11", "9", "10"]);
    }

    #[test]
    fn test_next_sort_category_cycles_displayed_columns() {
        let mut job_list = create_job_list();
        // with the default columns, Tab cycles Id -> Name
        job_list.handle_joblist_action(JobListAction::NextSortCategory, &JobColumn::defaults());
        assert_eq!(*job_list.get_sort_category(), JobColumn::Name);

        // with a custom column set, Tab only visits the displayed columns
        let columns = vec![JobColumn::Id, JobColumn::Priority];
        job_list.handle_joblist_action(JobListAction::NextSortCategory, &columns);
        // "Name" is not displayed, so the cycle restarts at the first column
        assert_eq!(*job_list.get_sort_category(), JobColumn::Id);
        job_list.handle_joblist_action(JobListAction::NextSortCategory, &columns);
        assert_eq!(*job_list.get_sort_category(), JobColumn::Priority);
        job_list.handle_joblist_action(JobListAction::NextSortCategory, &columns);
        assert_eq!(*job_list.get_sort_category(), JobColumn::Id);
    }

    #[test]
    fn test_sort_by_time() {
        let mut job_list = JobList::new();
        job_list
            .jobs
            .push(create_job("9", JobStatus::Running, "00:30:00", 1));
        job_list
            .jobs
            .push(create_job("10", JobStatus::Running, "00:05:00", 1));
        job_list
            .jobs
            .push(create_job("11", JobStatus::Running, "00:05:00", 1));
        job_list.set_sort_category(JobColumn::Time);

        // ascending by time string; ties broken by ascending numeric id
        assert_eq!(job_ids(&job_list), ["10", "11", "9"]);
    }

    #[test]
    fn test_reselect_job_resets_on_id_miss() {
        let mut job_list = create_job_list();

        // select the job with id "2" (index 1)
        job_list.select_job_by_id("2".to_string()).unwrap();
        assert_eq!(job_list.selected, 1);

        // simulate a refresh where job "2" disappeared but the old
        // index is still in bounds
        job_list.jobs.remove(1);
        assert!(job_list.selected < job_list.len());

        // the id miss must reset the selection to the first job instead
        // of silently keeping the stale index (which would now point at
        // a different job)
        job_list.reselect_job("2".to_string());
        assert_eq!(job_list.selected, 0);

        // if the job still exists, it stays selected
        job_list.reselect_job("3".to_string());
        assert_eq!(job_list.get_job().unwrap().id, "3");
    }

    #[test]
    fn test_get_user_has_no_trailing_whitespace() {
        // in a normal test environment either $USER or `whoami` yields
        // a username; whatever is returned must be trimmed and non-empty
        if let Some(user) = get_user() {
            assert_eq!(user, user.trim());
            assert!(!user.is_empty());
        }
    }
}
