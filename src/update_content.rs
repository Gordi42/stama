use crate::job::Job;
use crate::job::JobStatus;
use crate::user_options::UserOptions;
use std::collections::HashSet;
use std::process::Command;
use std::sync::mpsc;
use std::thread;

#[derive(Debug, Clone)]
pub struct Content {
    pub job: Option<Job>,
    pub job_list: Vec<Job>,
    pub details_text: String,
    pub log_text: String,
}

impl Content {
    pub fn new(
        job: Option<Job>,
        job_list: Vec<Job>,
        details_text: String,
        log_text: String,
    ) -> Self {
        Self {
            job,
            job_list,
            details_text,
            log_text,
        }
    }
}

pub struct MyProcess {
    pub receiver: mpsc::Receiver<Content>,
    pub handler: thread::JoinHandle<()>,
}

pub struct ContentUpdater {
    pub my_process: Option<MyProcess>,
}

impl Default for ContentUpdater {
    fn default() -> Self {
        Self::new()
    }
}

impl ContentUpdater {
    pub fn new() -> Self {
        Self { my_process: None }
    }

    pub fn tick(
        &mut self,
        job: Option<Job>,
        command: String,
        options: UserOptions,
    ) -> Option<Content> {
        // check if there is already a job queued
        let job_clone = job.clone();
        // if not send the new job
        match &self.my_process {
            Some(my_process) => {
                // try to receive the content
                match my_process.receiver.try_recv() {
                    Ok(mut content) => {
                        self.start_new_process(job, command, options);
                        update_job_content(job_clone, &mut content);
                        Some(content)
                    }
                    // the worker is still running, check again on the next tick
                    Err(mpsc::TryRecvError::Empty) => None,
                    // the worker died without sending (e.g. it panicked);
                    // drop the dead process so the next tick spawns a fresh one
                    // instead of freezing the job list forever
                    Err(mpsc::TryRecvError::Disconnected) => {
                        self.my_process = None;
                        None
                    }
                }
            }
            None => {
                self.start_new_process(job, command, options);
                None
            }
        }
    }

    fn start_new_process(&mut self, job: Option<Job>, command: String, options: UserOptions) {
        let (tx, rx) = mpsc::channel();
        let handler = thread::spawn(move || {
            tx.send(get_content(job, command, options)).unwrap_or(());
        });
        self.my_process = Some(MyProcess {
            receiver: rx,
            handler,
        });
    }
}

fn get_content(job: Option<Job>, command: String, options: UserOptions) -> Content {
    // setup a thread to get the joblist from squeue
    let command_clone = command.clone();
    let (tx_sq, rx_sq) = mpsc::channel();
    let handle_sq = thread::spawn(move || {
        tx_sq.send(get_squeue_joblist(&command_clone)).unwrap();
    });
    // setup a thread to get the joblist from sacct
    let command_clone = command.clone();
    let (tx_sa, rx_sa) = mpsc::channel();
    let handle_sa = match options.show_completed_jobs {
        true => thread::spawn(move || {
            tx_sa.send(get_acct_joblist(&command_clone)).unwrap();
        }),
        false => thread::spawn(|| {}),
    };
    // setup a thread to get the job details
    let (tx_jd, rx_jd) = mpsc::channel();
    let handle_jd = match job {
        Some(ref job) => {
            let job_id_clone = job.id.clone();
            thread::spawn(move || {
                tx_jd.send(get_job_details(&job_id_clone)).unwrap();
            })
        }
        None => thread::spawn(|| {}),
    };
    // setup a thread to get the log
    let (tx_log, rx_log) = mpsc::channel();
    let handle_log = match job {
        Some(ref job) => match job.get_stdout() {
            Some(ref output) => {
                let log_path = output.clone();
                thread::spawn(move || {
                    tx_log.send(get_log_tail(&log_path)).unwrap();
                })
            }
            None => thread::spawn(|| {}),
        },
        None => thread::spawn(|| {}),
    };

    // collect the joblist from squeue
    let mut joblist = rx_sq.recv().unwrap();
    handle_sq.join().unwrap();
    // collect the joblist from sacct
    if options.show_completed_jobs {
        joblist.extend(rx_sa.recv().unwrap());
        handle_sa.join().unwrap();
    }
    let mut details_text = "No job selected".to_string();
    let mut log_text = "No logfile available".to_string();
    // collect the job details
    if let Some(ref job) = job {
        details_text = rx_jd.recv().unwrap();
        handle_jd.join().unwrap();
        if job.output.is_some() {
            log_text = rx_log.recv().unwrap();
            handle_log.join().unwrap();
        }
    }
    // if a job is JobStatus::Completing (from squeue), sacct may still report
    // a JobStatus::Completed entry with the same id
    // remove the JobStatus::Completed duplicates
    remove_completed_duplicates(&mut joblist);

    Content::new(job, joblist, details_text, log_text)
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

fn get_squeue_joblist(command: &str) -> Vec<Job> {
    let format_entries = vec![
        "JobID:16",
        "Name:32",
        "StateCompact:2",
        "TimeUsed:16",
        "PendingTime:16",
        "Partition:16",
        "NumNodes:8",
        "WorkDir:256",
        "Command:256",
        "StdOut:256",
    ];
    let format = format_entries.join("|%|,");
    let command = format!("{} --Format=\",{},\"", command, format);
    let output = get_squeue_output(&command);
    format_squeue_output(&output)
}

pub fn get_squeue_output(command: &str) -> String {
    // split the command into first word and the rest
    let mut parts = command.split_whitespace();
    let program = parts.next().unwrap_or(" ");
    let args: Vec<&str> = parts.collect();

    let command_stat = Command::new(program).args(args).output();

    match command_stat {
        Ok(output) => {
            if !output.status.success() {
                // TODO(scheduler-refactor): return a Result instead of a
                // sentinel string; the parsers currently treat this as
                // output that yields zero jobs
                return "Error executing command".to_string();
            }
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        }
        // TODO(scheduler-refactor): return a Result instead of a sentinel string
        Err(_) => "Error executing squeue".to_string(),
    }
}

pub fn format_squeue_output(output: &str) -> Vec<Job> {
    let mut joblist = vec![];
    for line in output.lines().skip(1) {
        let parts = line.split("|%|").map(|s| s.trim()).collect::<Vec<&str>>();
        // A well-formed line has 10 fields separated by 9 "|%|" delimiters
        // (the --Format suffix is attached to every entry except the last),
        // so splitting yields exactly 10 parts. Skip anything shorter
        // (error messages, help text, truncated output) instead of panicking.
        if parts.len() < 10 {
            continue;
        }
        let id = parts[0].to_string();
        let name = parts[1].to_string();
        let status = match parts[2] {
            "R" => JobStatus::Running,
            "PD" => JobStatus::Pending,
            "CG" => JobStatus::Completing,
            _ => JobStatus::Unknown,
        };
        let time = match status {
            JobStatus::Pending => format_time_pending(parts[4]),
            _ => format_time_used(parts[3]),
        };
        let partition = parts[5].to_string();
        let nodes = parts[6].parse::<u32>().unwrap_or(0);
        let workdir = parts[7].to_string();
        let command = parts[8].to_string();
        let output = parts[9].to_string();

        joblist.push(Job::new(
            &id,
            &name,
            status,
            &time,
            &partition,
            nodes,
            &workdir,
            &command,
            Some(output),
        ));
    }
    joblist
}

fn get_acct_joblist(command: &str) -> Vec<Job> {
    let output = get_sacct_output(command);
    format_sacct_output(&output)
}

pub fn get_sacct_output(command: &str) -> String {
    let mut parts = command.split_whitespace();
    let _program = parts.next().unwrap_or(" ");
    let args: Vec<&str> = parts.collect();

    let entries = [
        "JobID",
        "JobName",
        "State",
        "Elapsed",
        "Partition",
        "NNodes",
        "WorkDir",
        "SubmitLine",
    ];
    let format = entries.join(",");
    let format_arg = format!("--format={}", format);

    let command_stat = Command::new("sacct")
        .args(args)
        .args(&[format_arg, "--parsable2".to_string(), "-n".to_string()])
        .output();
    match command_stat {
        Ok(output) => {
            if !output.status.success() {
                // TODO(scheduler-refactor): return a Result instead of a
                // sentinel string; the parsers currently treat this as
                // output that yields zero jobs
                return "Error executing sacct".to_string();
            }
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        }
        // TODO(scheduler-refactor): return a Result instead of a sentinel string
        Err(_) => "Error executing sacct".to_string(),
    }
}

pub fn format_sacct_output(output: &str) -> Vec<Job> {
    let mut joblist = vec![];
    for line in output.lines() {
        if line.trim().is_empty() {
            continue;
        }

        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() < 8 {
            continue;
        }

        let id = fields[0].trim();
        let name = fields[1].trim().to_string();
        let status_text = fields[2].trim();
        let status = match status_text {
            s if s.starts_with("COMPLETED") => JobStatus::Completed,
            s if s.starts_with("TIMEOUT") => JobStatus::Timeout,
            s if s.starts_with("CANCELLED") => JobStatus::Cancelled,
            s if s.starts_with("FAILED") => JobStatus::Failed,
            s if s.starts_with("RUNNING") => continue,
            s if s.starts_with("PENDING") => continue,
            _ => JobStatus::Unknown,
        };

        let time = fields[3].trim();
        let partition = fields[4].trim();
        if partition.is_empty() {
            continue;
        }

        let nodes = fields[5].trim().parse::<u32>().unwrap_or(0);
        let workdir = fields[6].trim().to_string();
        let command = fields[7].trim().to_string();

        joblist.push(Job::new(
            id, &name, status, time, partition, nodes, &workdir, &command, None,
        ));
    }
    joblist
}

pub fn get_job_details(job_id: &str) -> String {
    let args = vec!["show", "job", &job_id];
    let command_stat = Command::new("scontrol").args(args).output();
    match command_stat {
        Ok(output) => {
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        }
        Err(e) => e.to_string(),
    }
}

fn get_log_tail(log_path: &str) -> String {
    // check if the path exists
    if !std::path::Path::new(log_path).exists() {
        return format!("Logfile does not exist: {}", log_path);
    }
    // Option 1: use std::fs::read_to_string
    // Option 2: use Command::new("tail")
    // the first option seems to be slow for large files
    // so use the second option for now

    // // read the content of the file at the log file path
    // match std::fs::read_to_string(log_path) {
    //     Ok(content) => {
    //         let lines = content.lines().collect::<Vec<&str>>();
    //         lines.join("\n")
    //     },
    //     Err(e) => {
    //         e.to_string()
    //     },
    // }

    let command_stat = Command::new("tail")
        .arg("-n")
        .arg("100") // last 100 lines should be enough
        .arg(log_path)
        .output();
    match command_stat {
        Ok(output) => {
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        }
        Err(e) => e.to_string(),
    }
}

fn format_time_used(time_str: &str) -> String {
    // format the time string in D-HH:MM:SS
    let mut time_output = "0-00:00:00".to_string();
    if time_str.len() <= time_output.len() {
        let start_ind = time_output.len() - time_str.len();
        time_output.replace_range(start_ind.., time_str);
    } else {
        time_output = time_str.to_string();
    }
    time_output
}

fn format_time_pending(time_str: &str) -> String {
    let time_in_sec = time_str.parse::<u64>().unwrap_or(0);
    let days = time_in_sec / (24 * 3600);
    let hours = (time_in_sec % (24 * 3600)) / 3600;
    let minutes = (time_in_sec % 3600) / 60;
    let seconds = time_in_sec % 60;
    format!("{}-{:02}:{:02}:{:02}", days, hours, minutes, seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // format_sacct_output
    // ----------------------------------------------------------------
    // Field order must match the --format list in get_sacct_output:
    // JobID|JobName|State|Elapsed|Partition|NNodes|WorkDir|SubmitLine
    // (--parsable2: '|'-separated, no trailing delimiter; -n: no header)

    #[test]
    fn format_sacct_output_parses_completed_and_cancelled_jobs() {
        let output = "\
1001|myjob|COMPLETED|01:00:00|compute|2|/home/user/run|sbatch job.sh
1001.batch|batch|COMPLETED|01:00:00||2|/home/user/run|
1001.extern|extern|COMPLETED|01:00:00||2|/home/user/run|
1002|cancelled_job|CANCELLED by 4242|00:10:00|gpu|1|/home/user/other|sbatch cancel.sh
1003|running_job|RUNNING|00:05:00|compute|4|/home/user/run|sbatch run.sh
";
        let jobs = format_sacct_output(output);

        // step rows (.batch/.extern, empty Partition) are skipped,
        // RUNNING rows are filtered out
        assert_eq!(jobs.len(), 2);

        assert_eq!(jobs[0].id, "1001");
        assert_eq!(jobs[0].name, "myjob");
        assert_eq!(jobs[0].status, JobStatus::Completed);
        assert_eq!(jobs[0].time, "01:00:00");
        assert_eq!(jobs[0].partition, "compute");
        assert_eq!(jobs[0].nodes, 2);
        assert_eq!(jobs[0].workdir, "/home/user/run");
        assert_eq!(jobs[0].command, "sbatch job.sh");

        assert_eq!(jobs[1].id, "1002");
        assert_eq!(jobs[1].status, JobStatus::Cancelled);
        assert_eq!(jobs[1].nodes, 1);
    }

    // ----------------------------------------------------------------
    // format_squeue_output
    // ----------------------------------------------------------------
    // Field order must match the --Format list in get_squeue_joblist:
    // JobID, Name, StateCompact, TimeUsed, PendingTime, Partition,
    // NumNodes, WorkDir, Command, StdOut
    // The "|%|" suffix is attached to every field except the last, so a
    // line has 10 fields and 9 delimiters. The first line is the header.

    fn squeue_line(fields: [&str; 10]) -> String {
        fields.join("|%|")
    }

    #[test]
    fn format_squeue_output_parses_running_and_pending_jobs() {
        let header = "JOBID|%|NAME|%|ST|%|TIME|%|PENDING_TIME|%|PARTITION|%|NODES|%|WORK_DIR|%|COMMAND|%|STDOUT";
        let running = squeue_line([
            "1234 ",
            " job_running ",
            "R ",
            "12:34 ",
            "0 ",
            "main ",
            "1 ",
            "/work ",
            "/work/run.sh ",
            "/work/out-%j.log ",
        ]);
        let pending = squeue_line([
            "5678",
            "job_pending",
            "PD",
            "0:00",
            "3661",
            "gpu",
            "2",
            "/work2",
            "/work2/run.sh",
            "/work2/out.log",
        ]);
        let output = format!("{}\n{}\n{}\n", header, running, pending);

        let jobs = format_squeue_output(&output);
        assert_eq!(jobs.len(), 2);

        assert_eq!(jobs[0].id, "1234");
        assert_eq!(jobs[0].name, "job_running");
        assert_eq!(jobs[0].status, JobStatus::Running);
        // running jobs use TimeUsed, padded into D-HH:MM:SS
        assert_eq!(jobs[0].time, "0-00:12:34");
        assert_eq!(jobs[0].partition, "main");
        assert_eq!(jobs[0].nodes, 1);
        assert_eq!(jobs[0].workdir, "/work");
        assert_eq!(jobs[0].command, "/work/run.sh");
        assert_eq!(jobs[0].output.as_deref(), Some("/work/out-%j.log"));

        assert_eq!(jobs[1].id, "5678");
        assert_eq!(jobs[1].status, JobStatus::Pending);
        // pending jobs use PendingTime (seconds): 3661 s = 1 h 1 min 1 s
        assert_eq!(jobs[1].time, "0-01:01:01");
        assert_eq!(jobs[1].nodes, 2);
    }

    // ----------------------------------------------------------------
    // malformed input (regression test for the bounds guard)
    // ----------------------------------------------------------------

    #[test]
    fn malformed_input_does_not_panic() {
        let inputs = [
            "",
            "garbage",
            "a|%|b|%|c",
            "a|b|c",
            "Error executing command",
            "Error executing squeue",
            "Error executing sacct",
            "Usage: squeue [OPTIONS]\n  -A, --account=account(s)\n  -h, --noheader\nHelp options:\n  --help  show this help message\n",
            "JOBID|%|NAME\n1234|%|too_short\n",
            "\n\n\n",
        ];
        for input in inputs {
            // must not panic; garbage yields no (or only partial) jobs
            let squeue_jobs = format_squeue_output(input);
            assert!(
                squeue_jobs.is_empty(),
                "unexpected squeue jobs from {:?}",
                input
            );
            let sacct_jobs = format_sacct_output(input);
            assert!(
                sacct_jobs.is_empty(),
                "unexpected sacct jobs from {:?}",
                input
            );
        }
    }

    // ----------------------------------------------------------------
    // time formatting
    // ----------------------------------------------------------------

    #[test]
    fn format_time_used_edge_cases() {
        // shorter strings are padded into the D-HH:MM:SS template
        assert_eq!(format_time_used("15"), "0-00:00:15");
        assert_eq!(format_time_used("1:23"), "0-00:01:23");
        assert_eq!(format_time_used("12:34:56"), "0-12:34:56");
        assert_eq!(format_time_used("1-02:03:04"), "1-02:03:04");
        // longer strings are passed through unchanged
        assert_eq!(format_time_used("12-02:03:04"), "12-02:03:04");
        // empty input yields the zero template
        assert_eq!(format_time_used(""), "0-00:00:00");
        // garbage input must not panic
        let _ = format_time_used("garbage");
        let _ = format_time_used("N/A");
    }

    #[test]
    fn format_time_pending_edge_cases() {
        assert_eq!(format_time_pending("0"), "0-00:00:00");
        assert_eq!(format_time_pending("59"), "0-00:00:59");
        assert_eq!(format_time_pending("3661"), "0-01:01:01");
        assert_eq!(format_time_pending("90061"), "1-01:01:01");
        // non-numeric input falls back to zero and must not panic
        assert_eq!(format_time_pending(""), "0-00:00:00");
        assert_eq!(format_time_pending("garbage"), "0-00:00:00");
        assert_eq!(format_time_pending("-5"), "0-00:00:00");
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
