use regex::Regex;

#[derive(Debug, Clone, Default, PartialEq)]
pub enum JobStatus {
    #[default]
    Unknown,
    Running,
    Pending,
    Completing,
    Completed,
    Timeout,
    Cancelled,
    Failed,
}

impl JobStatus {
    /// The compact Slurm-style status code used in the aggregate
    /// status cell of a job-array group row (e.g. "3R 10PD").
    pub fn abbrev(&self) -> &'static str {
        match self {
            JobStatus::Unknown => "?",
            JobStatus::Running => "R",
            JobStatus::Pending => "PD",
            JobStatus::Completing => "CG",
            JobStatus::Completed => "CD",
            JobStatus::Timeout => "TO",
            JobStatus::Cancelled => "CA",
            JobStatus::Failed => "F",
        }
    }

    pub fn priority(&self) -> usize {
        match self {
            JobStatus::Unknown => 0,
            JobStatus::Pending => 1,
            JobStatus::Running => 2,
            JobStatus::Completing => 3,
            JobStatus::Failed => 4,
            JobStatus::Completed => 5,
            JobStatus::Timeout => 6,
            JobStatus::Cancelled => 7,
        }
    }
}
impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let status = match self {
            JobStatus::Unknown => "Unknown",
            JobStatus::Running => "Running",
            JobStatus::Pending => "Pending",
            JobStatus::Completing => "Completing",
            JobStatus::Completed => "Completed",
            JobStatus::Timeout => "Timeout",
            JobStatus::Cancelled => "Cancelled",
            JobStatus::Failed => "Failed",
        };
        write!(f, "{}", status)
    }
}

/// seff-style efficiency figures of a job, computed from sacct output
/// (see `Scheduler::job_stats`). Every component is `None` when the
/// underlying sacct fields are missing or unparsable (e.g. for jobs
/// that have not started yet).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct JobStats {
    /// TotalCPU / (Elapsed * AllocCPUS): the fraction of the allocated
    /// CPU time that was actually used (1.0 = 100 %).
    pub cpu_efficiency: Option<f64>,
    /// MaxRSS / total requested memory (1.0 = 100 %).
    pub mem_efficiency: Option<f64>,
    /// Elapsed / Timelimit: how much of the time limit is used up.
    pub elapsed_frac_of_limit: Option<f64>,
}

impl JobStats {
    /// Whether at least one component is available for display.
    pub fn has_any(&self) -> bool {
        self.cpu_efficiency.is_some()
            || self.mem_efficiency.is_some()
            || self.elapsed_frac_of_limit.is_some()
    }
}

#[derive(Debug, Clone, Default)]
pub struct Job {
    pub id: String,             // the job id
    pub name: String,           // the name of the job
    pub status: JobStatus,      // the status of the job
    pub time: String,           // time string
    pub partition: String,      // the partition the job is running on
    pub nodes: u32,             // the number of nodes the job is running on
    pub workdir: String,        // the working directory of the job
    pub command: String,        // the command the job is running
    pub output: Option<String>, // the output of the job
    /// The squeue "Reason" of the job (only meaningful for Pending).
    pub reason: Option<String>,
    /// The scheduling priority of the job (squeue PriorityLong).
    pub priority: u64,
    /// The account the job is charged to.
    pub account: String,
    /// The quality of service of the job.
    pub qos: String,
    /// The number of allocated CPUs.
    pub cpus: u32,
    /// The compressed node list of the job (e.g. "l[42314-42316]").
    pub nodelist: String,
    /// seff-style efficiency stats (filled in for the selected job).
    /// Boxed to keep `Job` (which is embedded in action enums) small.
    pub stats: Option<Box<JobStats>>,
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl Job {
    // A constructor mirroring the struct's fields; grouping them would be a refactor.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: &str,
        name: &str,
        status: JobStatus,
        time: &str,
        partition: &str,
        nodes: u32,
        workdir: &str,
        command: &str,
        output: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            status,
            time: time.to_string(),
            partition: partition.to_string(),
            nodes,
            workdir: workdir.to_string(),
            command: command.to_string(),
            output,
            reason: None,
            priority: 0,
            account: String::new(),
            qos: String::new(),
            cpus: 0,
            nodelist: String::new(),
            stats: None,
        }
    }

    /// Create a random job (for testing purposes)
    pub fn new_default() -> Self {
        Self {
            id: "123456".to_string(),
            name: "jobname".to_string(),
            status: JobStatus::Running,
            time: "00:00:00".to_string(),
            partition: "default".to_string(),
            nodes: 1,
            workdir: "/home/user".to_string(),
            command: "/path/to/script".to_string(),
            output: None,
            reason: None,
            priority: 0,
            account: String::new(),
            qos: String::new(),
            cpus: 0,
            nodelist: String::new(),
            stats: None,
        }
    }
}

// ====================================================================
//  JOB-ARRAY IDS
// ====================================================================

/// The task part of a Slurm job-array id.
#[derive(Debug, Clone, PartialEq)]
pub enum ArrayTask {
    /// A concrete task index, e.g. the "7" of "12345_7".
    Id(u64),
    /// The pending-range placeholder squeue shows for tasks that have
    /// not started yet, e.g. the "[8-99]" of "12345_[8-99]" (with the
    /// brackets included).
    Range(String),
}

/// Parses a Slurm job-array id of the form `<base>_<task>`.
///
/// "12345_7" parses to `(12345, ArrayTask::Id(7))` and the pending
/// placeholder "12345_[8-99]" to `(12345, ArrayTask::Range("[8-99]"))`.
/// Returns `None` for ids that are not array-task ids (e.g. the plain
/// "12345", or ids with a non-numeric base).
pub fn parse_array_id(id: &str) -> Option<(u64, ArrayTask)> {
    let (base, task) = id.split_once('_')?;
    let base: u64 = base.parse().ok()?;
    if let Ok(task_id) = task.parse::<u64>() {
        return Some((base, ArrayTask::Id(task_id)));
    }
    if task.starts_with('[') && task.ends_with(']') {
        return Some((base, ArrayTask::Range(task.to_string())));
    }
    None
}

/// The base job id of an array-task id ("12345_7" -> "12345"), or
/// `None` if the id is not an array-task id.
pub fn array_base_id(id: &str) -> Option<&str> {
    parse_array_id(id)?;
    id.split_once('_').map(|(base, _)| base)
}

/// The number of tasks a pending-range placeholder stands for, e.g.
/// 92 for "[8-99]", 3 for "[1,3,5]" and 16 for "[0-15%4]" (the "%n"
/// suffix only limits concurrency). Unparsable items count as one
/// task, so the result is always at least 1.
pub fn pending_range_count(range: &str) -> u64 {
    let inner = range.trim_start_matches('[').trim_end_matches(']');
    // a trailing "%<n>" limits how many tasks run at once; it does
    // not change how many tasks the range contains
    let inner = inner.split('%').next().unwrap_or(inner);
    inner
        .split(',')
        .map(|item| match item.trim().split_once('-') {
            Some((start, end)) => match (start.parse::<u64>(), end.parse::<u64>()) {
                (Ok(start), Ok(end)) if end >= start => end - start + 1,
                _ => 1,
            },
            None => 1,
        })
        .sum::<u64>()
        .max(1)
}

// ====================================================================
//  PENDING-REASON EXPLANATIONS
// ====================================================================

/// Extracts the bare reason code from a raw squeue "Reason" value.
///
/// squeue may append details after the code, e.g.
/// "ReqNodeNotAvail, UnavailableNodes:n[1-2]" or "(Priority)"; the code
/// is the leading run of alphanumeric characters and underscores.
pub fn reason_code(raw: &str) -> &str {
    let trimmed = raw.trim().trim_start_matches('(');
    let end = trimmed
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .unwrap_or(trimmed.len());
    &trimmed[..end]
}

/// Maps a Slurm pending-reason code to a one-line plain-English
/// explanation. Returns `None` for unknown codes (the caller then
/// shows the raw code instead).
pub fn explain_reason(code: &str) -> Option<&'static str> {
    let explanation = match code {
        "" | "None" => "waiting for the scheduler to evaluate the job",
        "Priority" => "waiting: higher-priority jobs are ahead in the queue",
        "Resources" => "waiting for the requested resources (nodes/CPUs/GPUs) to become free",
        "Dependency" => "waiting for a job dependency to finish",
        "DependencyNeverSatisfied" => {
            "a dependency failed and can never be satisfied; the job will never start"
        }
        "QOSMaxCpuPerUserLimit" => {
            "your per-user CPU limit of this QOS is reached; waits until your other jobs free CPUs"
        }
        "QOSMaxGRESPerUser" => "your per-user GPU/GRES limit of this QOS is reached",
        "QOSGrpCpuLimit" => "the group CPU limit of this QOS is reached",
        "QOSGrpGRES" => "the group GPU/GRES limit of this QOS is reached",
        "AssocGrpCpuLimit" => "your association/account has reached its CPU limit",
        "AssocGrpGRES" => "your association/account has reached its GPU/GRES limit",
        "AssocGrpGRESMinutes" => "your association/account has used up its GPU/GRES-minutes budget",
        "PartitionNodeLimit" => "the job requests more nodes than this partition allows",
        "PartitionTimeLimit" => "the job requests more time than this partition allows",
        "ReqNodeNotAvail" => {
            "a requested node is not available (down, drained or reserved, e.g. for maintenance)"
        }
        "BeginTime" => "the requested start time of the job has not been reached yet",
        "JobHeldUser" => "the job is held by the user; release it with 'scontrol release <jobid>'",
        "JobHeldAdmin" => "the job is held by an administrator",
        "NodeDown" => "a node required by the job is down",
        "BadConstraints" => "the constraints of the job cannot be satisfied by any node",
        "launch_failed_requeued_held" => {
            "the job launch failed (often a node problem); it was requeued and held"
        }
        _ => return None,
    };
    Some(explanation)
}

// ====================================================================
// METHODS
// ====================================================================

impl Job {
    pub fn get_jobname(&self) -> String {
        self.name.clone()
    }

    /// format the stdout of the command
    /// replace %j with the job id and %x with the job name
    pub fn get_stdout(&self) -> Option<String> {
        match &self.output {
            Some(output) => {
                let mut output = output.to_string();
                output = output.replace("%j", &self.id);
                output = output.replace("%x", &self.name);
                // we also want to cover the case when %<number>j is used
                // where <number> is the minimum number of digits to be used
                // leading zeros are added if the number of digits is less than <number>
                // Define a regular expression pattern to match %<n>j
                let re = Regex::new(r"%\d+j").unwrap();

                // Use the replace_all method to replace all occurrences of the pattern with the job ID
                Some(
                    re.replace_all(&output, |caps: &regex::Captures| {
                        // Extract the matched number from the pattern
                        let num_str = &caps[0][1..caps[0].len() - 1];
                        // Parse the number as usize
                        if let Ok(num) = num_str.parse::<usize>() {
                            // Generate the replacement string with the job ID
                            format!("{:0>width$}", &self.id, width = num)
                        } else {
                            // If parsing fails, return the original match
                            caps[0].to_string()
                        }
                    })
                    .to_string(),
                )
            }
            None => None,
        }
    }

    pub fn is_completed(&self) -> bool {
        matches!(
            self.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Timeout | JobStatus::Cancelled
        )
    }
}

// ====================================================================
// TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn job(id: &str, name: &str, output: Option<&str>) -> Job {
        Job::new(
            id,
            name,
            JobStatus::Running,
            "00:00:00",
            "compute",
            1,
            "/tmp",
            "run.sh",
            output.map(str::to_string),
        )
    }

    #[test]
    fn test_get_stdout_none_passthrough() {
        assert_eq!(job("123", "myjob", None).get_stdout(), None);
    }

    #[test]
    fn test_get_stdout_no_placeholders_unchanged() {
        assert_eq!(
            job("123", "myjob", Some("/logs/output.txt")).get_stdout(),
            Some("/logs/output.txt".to_string())
        );
    }

    #[test]
    fn test_get_stdout_job_id_expansion() {
        assert_eq!(
            job("123", "myjob", Some("slurm-%j.out")).get_stdout(),
            Some("slurm-123.out".to_string())
        );
    }

    #[test]
    fn test_get_stdout_job_name_expansion() {
        assert_eq!(
            job("123", "myjob", Some("%x.log")).get_stdout(),
            Some("myjob.log".to_string())
        );
    }

    #[test]
    fn test_get_stdout_combined_expansion() {
        assert_eq!(
            job("123", "myjob", Some("/logs/%x_%j.out")).get_stdout(),
            Some("/logs/myjob_123.out".to_string())
        );
    }

    #[test]
    fn test_get_stdout_zero_padded_id() {
        assert_eq!(
            job("123", "myjob", Some("slurm-%8j.out")).get_stdout(),
            Some("slurm-00000123.out".to_string())
        );
    }

    #[test]
    fn test_get_stdout_padding_width_smaller_than_id() {
        // the width is a minimum: an id longer than the width is not truncated
        assert_eq!(
            job("123456", "myjob", Some("%2j.out")).get_stdout(),
            Some("123456.out".to_string())
        );
    }

    // ----------------------------------------------------------------
    // pending-reason explanations
    // ----------------------------------------------------------------

    #[test]
    fn test_reason_code_extraction() {
        assert_eq!(reason_code("Priority"), "Priority");
        assert_eq!(reason_code("(Priority)"), "Priority");
        assert_eq!(reason_code("  Resources "), "Resources");
        // trailing details after the code are stripped
        assert_eq!(
            reason_code("ReqNodeNotAvail, UnavailableNodes:n[1-2]"),
            "ReqNodeNotAvail"
        );
        // underscores belong to the code
        assert_eq!(
            reason_code("launch_failed_requeued_held"),
            "launch_failed_requeued_held"
        );
        assert_eq!(reason_code(""), "");
    }

    #[test]
    fn test_explain_reason_known_codes() {
        // every code of the map yields an explanation
        for code in [
            "Priority",
            "Resources",
            "Dependency",
            "DependencyNeverSatisfied",
            "QOSMaxCpuPerUserLimit",
            "QOSMaxGRESPerUser",
            "QOSGrpCpuLimit",
            "QOSGrpGRES",
            "AssocGrpCpuLimit",
            "AssocGrpGRES",
            "AssocGrpGRESMinutes",
            "PartitionNodeLimit",
            "PartitionTimeLimit",
            "ReqNodeNotAvail",
            "BeginTime",
            "JobHeldUser",
            "JobHeldAdmin",
            "NodeDown",
            "BadConstraints",
            "launch_failed_requeued_held",
            "None",
            "",
        ] {
            assert!(
                explain_reason(code).is_some(),
                "no explanation for {:?}",
                code
            );
        }
        assert_eq!(
            explain_reason("Priority"),
            Some("waiting: higher-priority jobs are ahead in the queue")
        );
    }

    #[test]
    fn test_explain_reason_unknown_code_is_none() {
        assert_eq!(explain_reason("SomeNewSlurmReason"), None);
        assert_eq!(explain_reason("priority"), None); // case-sensitive
    }

    #[test]
    fn test_job_stats_has_any() {
        assert!(!JobStats::default().has_any());
        assert!(JobStats {
            cpu_efficiency: Some(0.5),
            ..JobStats::default()
        }
        .has_any());
        assert!(JobStats {
            elapsed_frac_of_limit: Some(0.1),
            ..JobStats::default()
        }
        .has_any());
    }

    // ----------------------------------------------------------------
    // job-array id parsing
    // ----------------------------------------------------------------

    #[test]
    fn test_parse_array_id_task() {
        assert_eq!(parse_array_id("12345_7"), Some((12345, ArrayTask::Id(7))));
        assert_eq!(array_base_id("12345_7"), Some("12345"));
    }

    #[test]
    fn test_parse_array_id_pending_range() {
        assert_eq!(
            parse_array_id("12345_[8-99]"),
            Some((12345, ArrayTask::Range("[8-99]".to_string())))
        );
        assert_eq!(array_base_id("12345_[8-99]"), Some("12345"));
    }

    #[test]
    fn test_parse_array_id_non_array_ids() {
        // a plain id is not an array-task id
        assert_eq!(parse_array_id("12345"), None);
        assert_eq!(array_base_id("12345"), None);
        // non-numeric bases and malformed task parts are rejected
        assert_eq!(parse_array_id("abc_1"), None);
        assert_eq!(parse_array_id("12345_abc"), None);
        assert_eq!(parse_array_id(""), None);
    }

    #[test]
    fn test_pending_range_count() {
        assert_eq!(pending_range_count("[8-99]"), 92);
        assert_eq!(pending_range_count("[0]"), 1);
        assert_eq!(pending_range_count("[1,3,5]"), 3);
        assert_eq!(pending_range_count("[1-3,7,10-11]"), 6);
        // a "%n" concurrency limit does not change the task count
        assert_eq!(pending_range_count("[0-15%4]"), 16);
        // unparsable input still counts as at least one task
        assert_eq!(pending_range_count("[garbage]"), 1);
        assert_eq!(pending_range_count(""), 1);
    }

    #[test]
    fn test_status_abbrev() {
        assert_eq!(JobStatus::Running.abbrev(), "R");
        assert_eq!(JobStatus::Pending.abbrev(), "PD");
        assert_eq!(JobStatus::Completing.abbrev(), "CG");
        assert_eq!(JobStatus::Completed.abbrev(), "CD");
        assert_eq!(JobStatus::Failed.abbrev(), "F");
        assert_eq!(JobStatus::Timeout.abbrev(), "TO");
        assert_eq!(JobStatus::Cancelled.abbrev(), "CA");
        assert_eq!(JobStatus::Unknown.abbrev(), "?");
    }

    #[test]
    fn test_get_stdout_array_job_id() {
        // an id with an array task suffix is substituted verbatim, and the
        // zero padding applies to the whole "id_task" string
        assert_eq!(
            job("123_4", "myjob", Some("%j.out")).get_stdout(),
            Some("123_4.out".to_string())
        );
        assert_eq!(
            job("123_4", "myjob", Some("%8j.out")).get_stdout(),
            Some("000123_4.out".to_string())
        );
    }
}
