//! Background refreshing of the job list and job details.
//!
//! [`ContentUpdater`] runs a single background worker at a time. Each
//! worker fetches the job list (squeue, optionally sacct), the details
//! of the selected job (scontrol) and the tail of its log file through
//! the injected [`Scheduler`] and sends the result back as a
//! [`Content`]. Command errors are carried in [`Content::error`] so the
//! app can surface them to the user.

use crate::job::{Job, JobStatus};
use crate::scheduler::{sacct_user_filter, LogChunk, Scheduler, SlurmScheduler};
use crate::user_options::UserOptions;
use std::collections::HashSet;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

/// How long a refresh worker may run before it is considered hung,
/// abandoned, and replaced by a fresh worker.
const WORKER_TIMEOUT: Duration = Duration::from_secs(30);

/// The error message surfaced when a worker hits [`WORKER_TIMEOUT`].
pub const TIMEOUT_ERROR: &str =
    "squeue is not responding (no job update for 30 seconds). Retrying...";

/// The number of lines shown from the end of a job's log file.
const LOG_TAIL_LINES: usize = 100;

/// What the live log view wants read on the next worker run: the log
/// path it follows and the file offset it has consumed so far
/// (`None` = the initial read, see [`Scheduler::log_since`]).
#[derive(Debug, Clone, PartialEq)]
pub struct LogFollowRequest {
    pub path: String,
    pub offset: Option<u64>,
}

/// The worker's answer to a [`LogFollowRequest`]: the path it read
/// (so a stale answer for a previously followed file can be ignored)
/// and the chunk, or `None` when the file is missing/unreadable (the
/// log view then keeps waiting and the next tick polls again).
#[derive(Debug, Clone, PartialEq)]
pub struct LogFollowUpdate {
    pub path: String,
    pub chunk: Option<LogChunk>,
}

#[derive(Debug, Clone)]
pub struct Content {
    pub job: Option<Job>,
    pub job_list: Vec<Job>,
    pub details_text: String,
    pub log_text: String,
    /// The incremental read for the live log view; `None` when no log
    /// view was open when the worker started.
    pub log_follow: Option<LogFollowUpdate>,
    /// The error text when fetching the job list failed (squeue/sacct);
    /// `None` when the update succeeded.
    pub error: Option<String>,
}

/// The result of a [`ContentUpdater::tick`] call.
pub enum ContentTick {
    /// The worker finished and delivered fresh content.
    New(Box<Content>),
    /// The worker is still running (or was just started); nothing new.
    Pending,
    /// The worker did not deliver within [`WORKER_TIMEOUT`] and was
    /// abandoned; a fresh worker is spawned on the next tick.
    TimedOut,
}

/// A running background worker: the channel it reports back on and the
/// time it was spawned (for the hang detection). The worker thread is
/// detached; a hung worker is simply abandoned by dropping the slot.
struct Worker {
    receiver: mpsc::Receiver<Content>,
    started: Instant,
}

pub struct ContentUpdater {
    worker: Option<Worker>,
    scheduler: Arc<dyn Scheduler>,
    timeout: Duration,
}

impl Default for ContentUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentUpdater {
    pub fn new() -> Self {
        Self::with_scheduler(Arc::new(SlurmScheduler))
    }

    pub fn with_scheduler(scheduler: Arc<dyn Scheduler>) -> Self {
        Self {
            worker: None,
            scheduler,
            timeout: WORKER_TIMEOUT,
        }
    }

    /// Overrides the worker timeout (only used to test the hang
    /// detection without waiting for the real timeout).
    #[cfg(test)]
    fn set_worker_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    pub fn tick(
        &mut self,
        job: Option<Job>,
        command: String,
        options: UserOptions,
        log_request: Option<LogFollowRequest>,
    ) -> ContentTick {
        match &self.worker {
            Some(worker) => match worker.receiver.try_recv() {
                // the worker finished: hand out its content and start
                // the next refresh right away
                Ok(mut content) => {
                    let job_clone = job.clone();
                    // the caller has not seen this content yet, so its
                    // request still carries the offset from *before*
                    // this chunk; advance it so the next worker does
                    // not read (and deliver) the same bytes twice
                    let log_request = advance_log_request(log_request, &content);
                    self.start_worker(job, command, options, log_request);
                    update_job_content(job_clone, &mut content);
                    ContentTick::New(Box::new(content))
                }
                // the worker is still running; if it has been running
                // for too long, consider it hung and abandon it so the
                // next tick spawns a fresh one
                Err(mpsc::TryRecvError::Empty) => {
                    if worker.started.elapsed() >= self.timeout {
                        self.worker = None;
                        ContentTick::TimedOut
                    } else {
                        ContentTick::Pending
                    }
                }
                // the worker died without sending (e.g. it panicked);
                // drop the dead worker so the next tick spawns a fresh
                // one instead of freezing the job list forever
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.worker = None;
                    ContentTick::Pending
                }
            },
            None => {
                self.start_worker(job, command, options, log_request);
                ContentTick::Pending
            }
        }
    }

    fn start_worker(
        &mut self,
        job: Option<Job>,
        command: String,
        options: UserOptions,
        log_request: Option<LogFollowRequest>,
    ) {
        let (tx, rx) = mpsc::channel();
        let scheduler = Arc::clone(&self.scheduler);
        thread::spawn(move || {
            tx.send(get_content(job, command, options, log_request, scheduler))
                .unwrap_or(());
        });
        self.worker = Some(Worker {
            receiver: rx,
            started: Instant::now(),
        });
    }
}

/// Advances a log-follow request past the chunk delivered in `content`
/// (only when it answers the same path), so the next worker continues
/// where that chunk ended instead of rereading it.
fn advance_log_request(
    request: Option<LogFollowRequest>,
    content: &Content,
) -> Option<LogFollowRequest> {
    let mut request = request?;
    if let Some(update) = &content.log_follow {
        if update.path == request.path {
            if let Some(chunk) = &update.chunk {
                request.offset = Some(chunk.offset);
            }
        }
    }
    Some(request)
}

/// Fetches the job list, job details and log tail. Runs on the worker
/// thread, so the individual commands simply run one after another.
fn get_content(
    job: Option<Job>,
    command: String,
    options: UserOptions,
    log_request: Option<LogFollowRequest>,
    scheduler: Arc<dyn Scheduler>,
) -> Content {
    let mut errors: Vec<String> = Vec::new();

    // the joblist from squeue
    let mut joblist = match scheduler.squeue_jobs(&command) {
        Ok(jobs) => jobs,
        Err(e) => {
            errors.push(e.to_string());
            Vec::new()
        }
    };

    // the completed jobs from sacct; only the user/cluster filter of
    // the squeue command carries over, the other flags do not apply
    if options.show_completed_jobs {
        match scheduler.sacct_jobs(&sacct_user_filter(&command)) {
            Ok(jobs) => joblist.extend(jobs),
            Err(e) => errors.push(e.to_string()),
        }
    }

    // the details and log tail of the selected job; errors here are
    // job-specific and shown inline in the respective pane instead of
    // in an error popup
    let mut details_text = "No job selected".to_string();
    let mut log_text = "No logfile available".to_string();
    if let Some(ref job) = job {
        details_text = scheduler
            .job_details(&job.id)
            .unwrap_or_else(|e| e.to_string());
        if let Some(log_path) = job.get_stdout() {
            log_text = scheduler
                .log_tail(&log_path, LOG_TAIL_LINES)
                .unwrap_or_else(|e| e.to_string());
        }
        // seff-style efficiency stats of the selected job, attached to
        // its entry in the fresh job list so the details pane can render
        // them; sacct has nothing useful for jobs that have not started,
        // and the stats are auxiliary, so errors are silently dropped
        if job.status != JobStatus::Pending {
            if let Ok(Some(stats)) = scheduler.job_stats(&job.id) {
                for entry in joblist.iter_mut().filter(|entry| entry.id == job.id) {
                    entry.stats = Some(Box::new(stats.clone()));
                }
            }
        }
    }

    // the incremental read for the live log view; a missing/unreadable
    // file is not an error but a "keep waiting" signal (chunk: None),
    // so a log path that does not exist yet never spams error popups
    let log_follow = log_request.map(|request| {
        let chunk = scheduler.log_since(&request.path, request.offset).ok();
        LogFollowUpdate {
            path: request.path,
            chunk,
        }
    });

    // if a job is JobStatus::Completing (from squeue), sacct may still
    // report a JobStatus::Completed entry with the same id
    // remove the JobStatus::Completed duplicates
    remove_completed_duplicates(&mut joblist);

    Content {
        job,
        job_list: joblist,
        details_text,
        log_text,
        log_follow,
        error: if errors.is_empty() {
            None
        } else {
            Some(errors.join("\n"))
        },
    }
}

/// Remove `Completed` entries (from sacct) whose job id also appears as a
/// `Completing` entry (from squeue), keeping the `Completing` one.
fn remove_completed_duplicates(joblist: &mut Vec<Job>) {
    let completing_ids: HashSet<String> = joblist
        .iter()
        .filter(|j| j.status == JobStatus::Completing)
        .map(|j| j.id.clone())
        .collect();
    if completing_ids.is_empty() {
        return;
    }
    joblist.retain(|j| !(j.status == JobStatus::Completed && completing_ids.contains(&j.id)));
}

fn update_job_content(job: Option<Job>, content: &mut Content) {
    let new_job = match job {
        Some(job) => job,
        None => return,
    };
    let old_job = match &content.job {
        Some(job) => job.clone(),
        None => return,
    };
    if new_job.id != old_job.id {
        set_content_loading(content);
    } else {
        if new_job.is_completed() {
            set_content_no_info(content);
        }
    }
}

fn set_content_loading(content: &mut Content) {
    content.details_text = "loading...".to_string();
    content.log_text = "loading...".to_string();
}

fn set_content_no_info(content: &mut Content) {
    let mut text = "Job id: ".to_string() + &content.job.as_ref().unwrap().id;
    text = text + "\nJob name: " + &content.job.as_ref().unwrap().name;
    text = text + "\nJob status: " + &content.job.as_ref().unwrap().status.to_string();
    text = text + "\nTime used: " + &content.job.as_ref().unwrap().time;
    text = text + "\nPartition: " + &content.job.as_ref().unwrap().partition;
    text = text + "\nNodes: " + &content.job.as_ref().unwrap().nodes.to_string();
    text = text + "\nWorkdir: " + &content.job.as_ref().unwrap().workdir;
    text = text + "\nCommand: " + &content.job.as_ref().unwrap().command;
    content.details_text = text;
    content.log_text =
        "Slurm has no database entry of the output file for completed jobs.".to_string();
}

// ====================================================================
//  TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scheduler::{FakeScheduler, SchedulerError};

    // ----------------------------------------------------------------
    // content pipeline with an injected FakeScheduler
    // ----------------------------------------------------------------

    /// Ticks the updater until the background worker delivers content
    /// (or panics after ~2 seconds).
    fn tick_until_content(
        updater: &mut ContentUpdater,
        command: &str,
        options: &UserOptions,
    ) -> Content {
        tick_until_content_with_job(updater, None, command, options)
    }

    /// Like [`tick_until_content`], but with a selected job.
    fn tick_until_content_with_job(
        updater: &mut ContentUpdater,
        job: Option<Job>,
        command: &str,
        options: &UserOptions,
    ) -> Content {
        tick_until_content_with(updater, job, command, options, None)
    }

    /// Like [`tick_until_content`], with a selected job and/or a
    /// log-follow request passed on every tick.
    fn tick_until_content_with(
        updater: &mut ContentUpdater,
        job: Option<Job>,
        command: &str,
        options: &UserOptions,
        log_request: Option<LogFollowRequest>,
    ) -> Content {
        for _ in 0..400 {
            match updater.tick(
                job.clone(),
                command.to_string(),
                options.clone(),
                log_request.clone(),
            ) {
                ContentTick::New(content) => return *content,
                _ => thread::sleep(Duration::from_millis(5)),
            }
        }
        panic!("the worker did not deliver content in time");
    }

    #[test]
    fn content_pipeline_merges_squeue_and_sacct_jobs() {
        let fake = Arc::new(FakeScheduler {
            squeue_response: Ok(vec![
                job("1", JobStatus::Running),
                job("2", JobStatus::Completing),
            ]),
            sacct_response: Ok(vec![
                // duplicate of the Completing job above; must be removed
                job("2", JobStatus::Completed),
                job("3", JobStatus::Completed),
            ]),
            ..FakeScheduler::default()
        });
        let mut updater = ContentUpdater::with_scheduler(fake.clone());
        let options = UserOptions {
            show_completed_jobs: true,
            ..UserOptions::default()
        };

        let content = tick_until_content(&mut updater, "squeue -u alice --state=PD", &options);

        assert!(content.error.is_none());
        // squeue and sacct joblists are merged, the Completed duplicate
        // of the Completing job is removed
        let ids: Vec<&str> = content.job_list.iter().map(|j| j.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "2", "3"]);
        assert_eq!(content.job_list[1].status, JobStatus::Completing);
        assert_eq!(content.job_list[2].status, JobStatus::Completed);

        // squeue received the unmodified user command
        assert_eq!(
            fake.squeue_commands.lock().unwrap().first().unwrap(),
            "squeue -u alice --state=PD"
        );
        // sacct received only the derived user filter, not squeue's
        // full argument list (--state=PD means nothing to sacct)
        assert_eq!(
            fake.sacct_filters.lock().unwrap().first().unwrap(),
            &vec!["-u".to_string(), "alice".to_string()]
        );
    }

    #[test]
    fn stats_of_selected_job_are_attached_to_its_job_list_entry() {
        use crate::job::JobStats;
        let stats = JobStats {
            cpu_efficiency: Some(0.85),
            mem_efficiency: Some(0.42),
            elapsed_frac_of_limit: Some(0.61),
        };
        let fake = Arc::new(FakeScheduler {
            squeue_response: Ok(vec![
                job("1", JobStatus::Running),
                job("2", JobStatus::Running),
            ]),
            stats_response: Ok(Some(stats.clone())),
            ..FakeScheduler::default()
        });
        let mut updater = ContentUpdater::with_scheduler(fake);
        let options = UserOptions::default();

        let selected = job("1", JobStatus::Running);
        let content = tick_until_content_with_job(&mut updater, Some(selected), "squeue", &options);

        // the stats are attached to the selected job's entry only
        assert_eq!(content.job_list[0].stats, Some(Box::new(stats)));
        assert_eq!(content.job_list[1].stats, None);
    }

    #[test]
    fn stats_are_not_fetched_for_pending_jobs() {
        use crate::job::JobStats;
        let fake = Arc::new(FakeScheduler {
            squeue_response: Ok(vec![job("1", JobStatus::Pending)]),
            stats_response: Ok(Some(JobStats {
                cpu_efficiency: Some(1.0),
                ..JobStats::default()
            })),
            ..FakeScheduler::default()
        });
        let mut updater = ContentUpdater::with_scheduler(fake);
        let options = UserOptions::default();

        let selected = job("1", JobStatus::Pending);
        let content = tick_until_content_with_job(&mut updater, Some(selected), "squeue", &options);

        // pending jobs never started, so no stats are requested/attached
        assert_eq!(content.job_list[0].stats, None);
    }

    #[test]
    fn failing_squeue_surfaces_error_in_content() {
        let fake = Arc::new(FakeScheduler {
            squeue_response: Err(SchedulerError::CommandFailed {
                program: "squeue".to_string(),
                stderr: "squeue: error: Invalid user: alice".to_string(),
            }),
            ..FakeScheduler::default()
        });
        let mut updater = ContentUpdater::with_scheduler(fake);
        let options = UserOptions {
            show_completed_jobs: false,
            ..UserOptions::default()
        };

        let content = tick_until_content(&mut updater, "squeue -u alice", &options);

        // the failure must surface as an error instead of silently
        // producing an empty "No jobs found" list
        let error = content.error.expect("the squeue error must surface");
        assert!(error.contains("squeue"));
        assert!(error.contains("Invalid user"));
    }

    // ----------------------------------------------------------------
    // log following (live log view)
    // ----------------------------------------------------------------
    //
    // FakeScheduler inherits the trait's default log_since (real file
    // reads), so these tests run against local temp files.

    #[test]
    fn log_follow_chunks_flow_through_the_content_pipeline() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("job.log");
        std::fs::write(&path, "one\ntwo\n").unwrap();
        let path = path.to_str().unwrap().to_string();
        let options = UserOptions::default();

        // initial read: offset None yields the scrollback tail
        let mut updater = ContentUpdater::with_scheduler(Arc::new(FakeScheduler::default()));
        let request = LogFollowRequest {
            path: path.clone(),
            offset: None,
        };
        let content =
            tick_until_content_with(&mut updater, None, "squeue", &options, Some(request));
        let update = content.log_follow.expect("a log update must arrive");
        assert_eq!(update.path, path);
        let chunk = update.chunk.expect("the file exists");
        assert_eq!(chunk.content, "one\ntwo\n");
        assert!(!chunk.truncated);

        // append and follow up with the returned offset: only the new
        // bytes arrive (a fresh updater makes the timing deterministic)
        std::fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .and_then(|mut f| std::io::Write::write_all(&mut f, b"three\n"))
            .unwrap();
        let mut updater = ContentUpdater::with_scheduler(Arc::new(FakeScheduler::default()));
        let request = LogFollowRequest {
            path: path.clone(),
            offset: Some(chunk.offset),
        };
        let content =
            tick_until_content_with(&mut updater, None, "squeue", &options, Some(request));
        let chunk = content.log_follow.unwrap().chunk.unwrap();
        assert_eq!(chunk.content, "three\n");
    }

    #[test]
    fn log_follow_missing_file_yields_waiting_update_not_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not_written_yet.log");
        let path = path.to_str().unwrap().to_string();
        let options = UserOptions::default();

        let mut updater = ContentUpdater::with_scheduler(Arc::new(FakeScheduler::default()));
        let request = LogFollowRequest {
            path: path.clone(),
            offset: None,
        };
        let content =
            tick_until_content_with(&mut updater, None, "squeue", &options, Some(request));

        // the update arrives with chunk: None ("keep waiting") and the
        // missing file does not surface in the error popup channel
        let update = content.log_follow.unwrap();
        assert_eq!(update.path, path);
        assert_eq!(update.chunk, None);
        assert!(content.error.is_none());
    }

    #[test]
    fn without_open_log_view_no_log_update_is_computed() {
        let mut updater = ContentUpdater::with_scheduler(Arc::new(FakeScheduler::default()));
        let content = tick_until_content(&mut updater, "squeue", &UserOptions::default());
        assert_eq!(content.log_follow, None);
    }

    #[test]
    fn advance_log_request_moves_past_the_delivered_chunk() {
        let request = LogFollowRequest {
            path: "/a.log".to_string(),
            offset: None,
        };
        let mut content = Content {
            job: None,
            job_list: Vec::new(),
            details_text: String::new(),
            log_text: String::new(),
            log_follow: Some(LogFollowUpdate {
                path: "/a.log".to_string(),
                chunk: Some(LogChunk {
                    content: "x\n".to_string(),
                    offset: 42,
                    truncated: false,
                }),
            }),
            error: None,
        };

        // the same path: the next worker continues at the chunk's end
        let advanced = advance_log_request(Some(request.clone()), &content).unwrap();
        assert_eq!(advanced.offset, Some(42));

        // a stale update for a different file leaves the request alone
        content.log_follow.as_mut().unwrap().path = "/other.log".to_string();
        let advanced = advance_log_request(Some(request.clone()), &content).unwrap();
        assert_eq!(advanced.offset, None);

        // an unreadable file ("keep waiting") leaves the offset alone
        content.log_follow = Some(LogFollowUpdate {
            path: "/a.log".to_string(),
            chunk: None,
        });
        let advanced = advance_log_request(Some(request), &content).unwrap();
        assert_eq!(advanced.offset, None);

        // no request in, no request out
        assert_eq!(advance_log_request(None, &content), None);
    }

    // ----------------------------------------------------------------
    // hang detection
    // ----------------------------------------------------------------

    /// A scheduler whose squeue call blocks longer than any test runs.
    struct BlockingScheduler;

    impl Scheduler for BlockingScheduler {
        fn squeue_jobs(&self, _command: &str) -> Result<Vec<Job>, SchedulerError> {
            thread::sleep(Duration::from_secs(60));
            Ok(Vec::new())
        }
        fn sacct_jobs(&self, _user_filter: &[String]) -> Result<Vec<Job>, SchedulerError> {
            Ok(Vec::new())
        }
        fn job_details(&self, _job_id: &str) -> Result<String, SchedulerError> {
            Ok(String::new())
        }
        fn job_stats(&self, _job_id: &str) -> Result<Option<crate::job::JobStats>, SchedulerError> {
            Ok(None)
        }
        fn cancel_job(&self, _job_id: &str) -> Result<(), SchedulerError> {
            Ok(())
        }
        fn job_nodes(&self, _job_id: &str) -> Result<Vec<String>, SchedulerError> {
            Ok(Vec::new())
        }
        fn log_tail(&self, _path: &str, _lines: usize) -> Result<String, SchedulerError> {
            Ok(String::new())
        }
    }

    #[test]
    fn hung_worker_times_out_and_is_replaced() {
        let mut updater = ContentUpdater::with_scheduler(Arc::new(BlockingScheduler));
        updater.set_worker_timeout(Duration::ZERO);
        let options = UserOptions::default();

        // the first tick spawns the (hanging) worker
        assert!(matches!(
            updater.tick(None, "squeue".to_string(), options.clone(), None),
            ContentTick::Pending
        ));
        // the worker has not delivered within the timeout: it is
        // abandoned and the timeout is reported exactly once
        assert!(matches!(
            updater.tick(None, "squeue".to_string(), options.clone(), None),
            ContentTick::TimedOut
        ));
        // the slot is free again, so the next tick starts a fresh worker
        assert!(matches!(
            updater.tick(None, "squeue".to_string(), options, None),
            ContentTick::Pending
        ));
    }

    // ----------------------------------------------------------------
    // duplicate removal (regression test for the index-shift bug)
    // ----------------------------------------------------------------

    fn job(id: &str, status: JobStatus) -> Job {
        Job::new(
            id,
            &format!("job_{}", id),
            status,
            "0-00:01:00",
            "main",
            1,
            "/work",
            "cmd",
            None,
        )
    }

    #[test]
    fn remove_completed_duplicates_keeps_completing_job() {
        // a Completing job with two Completed duplicates (the old
        // remove-by-index loop removed the wrong element or panicked when
        // the last duplicate was the final element) plus unrelated jobs
        let mut joblist = vec![
            job("1", JobStatus::Running),
            job("2", JobStatus::Completing),
            job("3", JobStatus::Completed),
            job("2", JobStatus::Completed),
            job("4", JobStatus::Pending),
            job("2", JobStatus::Completed),
        ];
        remove_completed_duplicates(&mut joblist);

        let ids: Vec<&str> = joblist.iter().map(|j| j.id.as_str()).collect();
        assert_eq!(ids, vec!["1", "2", "3", "4"]);
        assert_eq!(joblist[1].status, JobStatus::Completing);
        // the unrelated Completed job is untouched
        assert_eq!(joblist[2].status, JobStatus::Completed);
    }

    #[test]
    fn remove_completed_duplicates_no_completing_jobs_is_noop() {
        let mut joblist = vec![
            job("1", JobStatus::Completed),
            job("1", JobStatus::Completed),
            job("2", JobStatus::Running),
        ];
        remove_completed_duplicates(&mut joblist);
        assert_eq!(joblist.len(), 3);
    }
}
