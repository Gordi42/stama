use color_eyre::{Result, eyre::eyre};
use std::process::Command;

use crate::job::Job;
use crate::update_content::ContentUpdater;
use crate::user_options::UserOptions;

#[derive(PartialEq, Clone, Debug)]
pub enum SortCategory {
    Id,
    Name,
    Status,
    Time,
    Partition,
    Nodes,
}

impl SortCategory {
    /// Returns the next sort category.
    pub fn next(&self) -> SortCategory {
        match self {
            SortCategory::Id => SortCategory::Name,
            SortCategory::Name => SortCategory::Status,
            SortCategory::Status => SortCategory::Time,
            SortCategory::Time => SortCategory::Partition,
            SortCategory::Partition => SortCategory::Nodes,
            SortCategory::Nodes => SortCategory::Id,
        }
    }
}

/// An enum to handle actions that change the selected job.
#[derive(Debug, Clone)]
pub enum JobListAction {
    Next,
    Previous,
    Select(usize),
    SelectSortCategory(SortCategory),
    NextSortCategory,
    ReverseSortDirection,
    UpdateSqueueCommand(String),
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
    // The category by which the jobs are sorted.
    sort_category: SortCategory,
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
    /// Creates a new JobList.
    pub fn new() -> JobList {
        JobList {
            jobs: Vec::new(),
            selected: 0,
            job_details: String::new(),
            log_tail: String::new(),
            sort_category: SortCategory::Id,
            reverse: false,
            content_updater: ContentUpdater::new(),
            squeue_command: format!("squeue -u {}", whoami())
        }
    }
}

/// Returns the username of the current user.
fn whoami() -> String {
    let command = Command::new("whoami")
        .output();
    match command {
        Ok(output) => {
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        },
        Err(_) => {
            "Error executing whoami".to_string()
        },
    }
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

    /// Returns the category by which the jobs are sorted.
    pub fn get_sort_category(&self) -> &SortCategory {
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
    pub fn handle_joblist_action(&mut self, action: JobListAction) {
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
                self.set_sort_category(self.sort_category.next());
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

    /// Sets the category by which the jobs are sorted.
    pub fn set_sort_category(&mut self, category: SortCategory) {
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
    pub fn update_jobs(&mut self, user_options: &UserOptions) {
        // get the currently selected job to keep it selected after update
        let job: Option<Job> = self.get_job().cloned();
        let command = self.squeue_command.clone();
        // check if the content updater returns a new job list
        match self.content_updater.tick(
            job.clone(), command, user_options.clone()) {
            Some(content) => {
                self.jobs = content.job_list;
                self.job_details = content.details_text;
                self.log_tail = content.log_text;
            }
            None => { }
        }
        // sort the job list
        self.sort_raw();
        // try to select the job that was selected before the update
        if let Some(job) = job {
            self.select_job_by_id(job.id).unwrap_or_else(|_| {
                // if the job was not found:
                //  - check if the index is out of bounds
                //  - if so, set the index to 0
                if self.selected >= self.len() {
                    self.set_index(0).unwrap();
                }
            });
        }
    }

    /// Select the next job in the list.
    pub fn next(&mut self) {
        // check if the job list is empty
        if self.jobs.is_empty() { return; }
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
        if self.jobs.is_empty() { return; }
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
        if self.jobs.is_empty() { return; }
        // sort the job list based on the sort_category
        // secondary sort is based on the id
        match self.sort_category {
            SortCategory::Id => {
                self.jobs.sort_by(|a, b| b.id.cmp(&a.id));
            },
            SortCategory::Name => {
                self.jobs.sort_by(|a, b| {
                    a.name.cmp(&b.name).then_with(|| a.id.cmp(&b.id))
                });
            },
            SortCategory::Status => {
                self.jobs.sort_by(|a, b| {
                    a.status.priority().cmp(&b.status.priority())
                        .then_with(|| a.id.cmp(&b.id))
                });
            },
            SortCategory::Time => {
                self.jobs.sort_by(|a, b| {
                    a.time.cmp(&b.time).then_with(|| a.id.cmp(&b.id))
                });
            },
            SortCategory::Partition => {
                self.jobs.sort_by(|a, b| {
                    a.partition.cmp(&b.partition)
                        .then_with(|| a.id.cmp(&b.id))
                });
            },
            SortCategory::Nodes => {
                self.jobs.sort_by(|a, b| {
                    b.nodes.cmp(&a.nodes).then_with(|| a.id.cmp(&b.id))
                });
            },
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
        if self.jobs.is_empty() { return; }
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
                "1", "job1", JobStatus::Running, 
                "00:00:00", "partition1", 1,
                "workdir1", "command1", None));
        job_list.jobs.push(Job::new(
                "2", "job2", JobStatus::Pending,
                "00:00:00", "partition1", 2,
                "workdir2", "command2", None));
        job_list.jobs.push(Job::new(
                "3", "job3", JobStatus::Completing,
                "00:00:00", "partition1", 3,
                "workdir3", "command3", None));
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
        assert_eq!(job_list.reverse, false);
        job_list.negate_reverse();
        assert_eq!(job_list.reverse, true);
        // check if the selected index is set back to 0
        assert_eq!(job_list.selected, 0);
        job_list.negate_reverse();
        assert_eq!(job_list.reverse, false);
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
}
