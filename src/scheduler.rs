//! Central abstraction over all Slurm / external command execution.
//!
//! Every non-interactive external command (squeue, sacct, scontrol,
//! scancel, reading log files) goes through the [`Scheduler`] trait.
//! The real implementation is [`SlurmScheduler`]; tests inject a
//! [`FakeScheduler`] with canned responses. Errors are returned as
//! [`SchedulerError`] values so callers can surface them to the user
//! instead of silently showing an empty job list.
//!
//! Interactive processes that take over the terminal (the external
//! editor and salloc) are intentionally *not* part of this trait; they
//! are spawned directly in `app.rs`.

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::process::Command;

use crate::job::{Job, JobStats, JobStatus};

// ====================================================================
//  ERROR TYPE
// ====================================================================

/// An error from executing an external command or reading a file.
#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerError {
    /// The command could not be started (e.g. binary not found).
    SpawnFailed { program: String, message: String },
    /// The command ran but exited with a non-zero status.
    CommandFailed { program: String, stderr: String },
    /// A file could not be read (e.g. a job log file).
    FileRead { path: String, message: String },
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchedulerError::SpawnFailed { program, message } => {
                write!(f, "failed to run '{}': {}", program, message)
            }
            SchedulerError::CommandFailed { program, stderr } => {
                if stderr.is_empty() {
                    write!(f, "'{}' exited with an error", program)
                } else {
                    write!(f, "'{}' exited with an error: {}", program, stderr)
                }
            }
            SchedulerError::FileRead { path, message } => {
                write!(f, "could not read '{}': {}", path, message)
            }
        }
    }
}

impl std::error::Error for SchedulerError {}

// ====================================================================
//  TRAIT
// ====================================================================

/// Interface for all non-interactive Slurm / external command calls.
pub trait Scheduler: Send + Sync {
    /// Runs the user-configured squeue command (e.g. "squeue -u alice")
    /// and parses its output into a job list.
    fn squeue_jobs(&self, command: &str) -> Result<Vec<Job>, SchedulerError>;

    /// Runs sacct with the given user/cluster filter (e.g.
    /// `["-u", "alice"]`, see [`sacct_user_filter`]) and parses the
    /// completed jobs from its output.
    fn sacct_jobs(&self, user_filter: &[String]) -> Result<Vec<Job>, SchedulerError>;

    /// Returns the detail text of a job ("scontrol show job <id>").
    fn job_details(&self, job_id: &str) -> Result<String, SchedulerError>;

    /// Returns seff-style efficiency stats of a job, computed from
    /// sacct output. `Ok(None)` means sacct has no record of the job
    /// (yet); individual [`JobStats`] components are `None` when the
    /// underlying fields are missing or unparsable.
    fn job_stats(&self, job_id: &str) -> Result<Option<JobStats>, SchedulerError>;

    /// Cancels a job ("scancel <id>"). A successful return only means
    /// the cancel request was accepted, not that the job is gone.
    fn cancel_job(&self, job_id: &str) -> Result<(), SchedulerError>;

    /// Returns the nodes a job runs on, with Slurm's compressed node
    /// list (e.g. "l[42314-42316],m01") expanded into the individual
    /// node names (see [`parse_node_list`]).
    fn job_nodes(&self, job_id: &str) -> Result<Vec<String>, SchedulerError>;

    /// Returns the last `lines` lines of the file at `path`.
    fn log_tail(&self, path: &str, lines: usize) -> Result<String, SchedulerError>;
}

// ====================================================================
//  REAL IMPLEMENTATION
// ====================================================================

/// The real [`Scheduler`] that shells out to the Slurm commands.
pub struct SlurmScheduler;

impl Scheduler for SlurmScheduler {
    fn squeue_jobs(&self, command: &str) -> Result<Vec<Job>, SchedulerError> {
        let mut parts = command.split_whitespace().map(str::to_string);
        let program = parts.next().ok_or_else(|| SchedulerError::SpawnFailed {
            program: command.to_string(),
            message: "empty squeue command".to_string(),
        })?;
        let mut args: Vec<String> = parts.collect();
        args.push(squeue_format_arg());
        let output = run_command(&program, &args)?;
        Ok(format_squeue_output(&output))
    }

    fn sacct_jobs(&self, user_filter: &[String]) -> Result<Vec<Job>, SchedulerError> {
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
        let mut args: Vec<String> = user_filter.to_vec();
        args.push(format!("--format={}", entries.join(",")));
        args.push("--parsable2".to_string());
        args.push("-n".to_string());
        let output = run_command("sacct", &args)?;
        Ok(format_sacct_output(&output))
    }

    fn job_details(&self, job_id: &str) -> Result<String, SchedulerError> {
        let args = ["show", "job", job_id].map(str::to_string);
        run_command("scontrol", &args)
    }

    fn job_stats(&self, job_id: &str) -> Result<Option<JobStats>, SchedulerError> {
        // the field order must match the parsing in parse_job_stats
        let args = [
            "-j",
            job_id,
            "--parsable2",
            "-n",
            "--format=JobID,State,TotalCPU,Elapsed,AllocCPUS,MaxRSS,ReqMem,Timelimit,NNodes",
        ]
        .map(str::to_string);
        let output = run_command("sacct", &args)?;
        Ok(parse_job_stats(&output, job_id))
    }

    fn cancel_job(&self, job_id: &str) -> Result<(), SchedulerError> {
        run_command("scancel", &[job_id.to_string()])?;
        Ok(())
    }

    fn job_nodes(&self, job_id: &str) -> Result<Vec<String>, SchedulerError> {
        let args = ["-j", job_id, "--Format=NodeList", "--noheader"].map(str::to_string);
        let output = run_command("squeue", &args)?;
        Ok(parse_node_list(&output))
    }

    fn log_tail(&self, path: &str, lines: usize) -> Result<String, SchedulerError> {
        read_last_lines(path, lines)
    }
}

/// Runs a command and returns its stdout (lossily converted to UTF-8).
///
/// Returns [`SchedulerError::SpawnFailed`] if the command could not be
/// started and [`SchedulerError::CommandFailed`] (with the trimmed
/// stderr text) if it exited with a non-zero status.
fn run_command(program: &str, args: &[String]) -> Result<String, SchedulerError> {
    let output =
        Command::new(program)
            .args(args)
            .output()
            .map_err(|e| SchedulerError::SpawnFailed {
                program: program.to_string(),
                message: e.to_string(),
            })?;
    if !output.status.success() {
        return Err(SchedulerError::CommandFailed {
            program: program.to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Builds the `--Format` argument for the squeue command. The field
/// order must match the parsing in [`format_squeue_output`].
fn squeue_format_arg() -> String {
    let format_entries = [
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
        "Reason:64",
        // PriorityLong yields the priority as a sortable integer
        // (the plain "Priority" field is a float in 0..1)
        "PriorityLong:16",
        "Account:32",
        "QOS:32",
        "NumCPUs:8",
        "NodeList:64",
    ];
    // "|%|" is used as the field delimiter; it is attached as a suffix
    // to every field except the last
    format!("--Format=\",{},\"", format_entries.join("|%|,"))
}

/// Derives the sacct filter arguments from the user's squeue command.
///
/// Only the user (`-u`, `--user`) and cluster (`-M`, `--clusters`)
/// filters carry over; all other squeue flags either do not apply to
/// sacct or mean something different there (e.g. `-p` selects a
/// partition in squeue but means "parsable output" in sacct).
pub fn sacct_user_filter(squeue_command: &str) -> Vec<String> {
    let mut filter = Vec::new();
    let mut parts = squeue_command.split_whitespace();
    while let Some(part) = parts.next() {
        match part {
            "-u" | "--user" => {
                if let Some(value) = parts.next() {
                    filter.push("-u".to_string());
                    filter.push(value.to_string());
                }
            }
            "-M" | "--clusters" => {
                if let Some(value) = parts.next() {
                    filter.push("-M".to_string());
                    filter.push(value.to_string());
                }
            }
            _ => {
                if let Some(value) = part.strip_prefix("--user=") {
                    filter.push("-u".to_string());
                    filter.push(value.to_string());
                } else if let Some(value) = part.strip_prefix("--clusters=") {
                    filter.push("-M".to_string());
                    filter.push(value.to_string());
                }
            }
        }
    }
    filter
}

/// The maximum number of nodes [`parse_node_list`] expands to. Guards
/// against pathological ranges; no popup or ssh target needs more.
const MAX_EXPANDED_NODES: usize = 4096;

/// Expands Slurm's compressed node list format into the individual
/// node names, e.g. "l[42314-42316],m01" yields
/// ["l42314", "l42315", "l42316", "m01"]. Zero-padded ranges keep
/// their padding ("nid[001-003]" yields "nid001", ...).
///
/// Malformed items are kept verbatim instead of being dropped (so the
/// caller still sees *something* to report), and the expansion is
/// capped at [`MAX_EXPANDED_NODES`] entries.
pub fn parse_node_list(raw: &str) -> Vec<String> {
    let mut nodes = Vec::new();
    for expression in split_outside_brackets(raw.trim()) {
        expand_hostlist_expression(expression, &mut nodes);
        if nodes.len() >= MAX_EXPANDED_NODES {
            nodes.truncate(MAX_EXPANDED_NODES);
            break;
        }
    }
    nodes
}

/// Splits a node list on the commas that separate hostlist
/// expressions, i.e. the commas outside of "[...]" groups.
fn split_outside_brackets(raw: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (index, character) in raw.char_indices() {
        match character {
            '[' => depth += 1,
            ']' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => {
                parts.push(&raw[start..index]);
                start = index + 1;
            }
            _ => {}
        }
    }
    parts.push(&raw[start..]);
    parts
        .into_iter()
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect()
}

/// Expands a single hostlist expression like "l[1-3,7]" or "node01"
/// into `nodes` (stopping at [`MAX_EXPANDED_NODES`]).
fn expand_hostlist_expression(expression: &str, nodes: &mut Vec<String>) {
    let (prefix, rest) = match expression.split_once('[') {
        Some(parts) => parts,
        // no bracket group: a plain node name
        None => {
            nodes.push(expression.to_string());
            return;
        }
    };
    let (ranges, suffix) = match rest.split_once(']') {
        Some(parts) => parts,
        // unbalanced bracket: keep the expression verbatim
        None => {
            nodes.push(expression.to_string());
            return;
        }
    };
    for item in ranges.split(',') {
        let item = item.trim();
        if item.is_empty() {
            continue;
        }
        match parse_range(item) {
            Some((start, end, width)) => {
                for number in start..=end {
                    nodes.push(format!(
                        "{}{:0width$}{}",
                        prefix,
                        number,
                        suffix,
                        width = width
                    ));
                    if nodes.len() >= MAX_EXPANDED_NODES {
                        return;
                    }
                }
            }
            // a non-numeric (or reversed) item is kept verbatim
            None => nodes.push(format!("{}{}{}", prefix, item, suffix)),
        }
    }
}

/// Parses a range item "a-b" (or a single number "a") into
/// (start, end, zero-padding width). Returns `None` for non-numeric
/// items and reversed ranges.
fn parse_range(item: &str) -> Option<(u64, u64, usize)> {
    let (start_str, end_str) = match item.split_once('-') {
        Some((start, end)) => (start, end),
        None => (item, item),
    };
    let start: u64 = start_str.parse().ok()?;
    let end: u64 = end_str.parse().ok()?;
    if end < start {
        return None;
    }
    Some((start, end, start_str.len()))
}

// ====================================================================
//  LOG FILE READING
// ====================================================================

fn file_error(path: &str, error: std::io::Error) -> SchedulerError {
    SchedulerError::FileRead {
        path: path.to_string(),
        message: error.to_string(),
    }
}

/// Reads the last `lines` lines of the file at `path`.
///
/// The file is read backwards in chunks so that only the tail of large
/// log files is loaded. Non-UTF-8 bytes are tolerated via a lossy
/// conversion. Returns an error if the file does not exist or cannot
/// be read.
pub fn read_last_lines(path: &str, lines: usize) -> Result<String, SchedulerError> {
    const CHUNK_SIZE: u64 = 8192;

    let mut file = File::open(path).map_err(|e| file_error(path, e))?;
    let len = file.metadata().map_err(|e| file_error(path, e))?.len();

    // read chunks from the end of the file until the buffer contains
    // more newlines than requested lines (or the whole file is read)
    let mut buffer: Vec<u8> = Vec::new();
    let mut pos = len;
    while pos > 0 {
        let read_size = CHUNK_SIZE.min(pos);
        pos -= read_size;
        file.seek(SeekFrom::Start(pos))
            .map_err(|e| file_error(path, e))?;
        let mut chunk = vec![0u8; read_size as usize];
        file.read_exact(&mut chunk)
            .map_err(|e| file_error(path, e))?;
        chunk.extend_from_slice(&buffer);
        buffer = chunk;
        let newlines = buffer.iter().filter(|&&byte| byte == b'\n').count();
        if newlines > lines {
            break;
        }
    }

    let text = String::from_utf8_lossy(&buffer);
    let all_lines: Vec<&str> = text.lines().collect();
    let start = all_lines.len().saturating_sub(lines);
    Ok(all_lines[start..].join("\n"))
}

// ====================================================================
//  OUTPUT PARSING
// ====================================================================

/// Parses the output of the squeue command into a job list.
///
/// The field order must match the `--Format` list built in
/// [`squeue_format_arg`]: JobID, Name, StateCompact, TimeUsed,
/// PendingTime, Partition, NumNodes, WorkDir, Command, StdOut, Reason,
/// PriorityLong, Account, QOS, NumCPUs, NodeList.
pub fn format_squeue_output(output: &str) -> Vec<Job> {
    let mut joblist = vec![];
    for line in output.lines().skip(1) {
        let parts = line.split("|%|").map(|s| s.trim()).collect::<Vec<&str>>();
        // A well-formed line has 16 fields separated by 15 "|%|" delimiters
        // (the --Format suffix is attached to every entry except the last),
        // so splitting yields exactly 16 parts. Skip anything shorter
        // (error messages, help text, truncated output) instead of panicking.
        if parts.len() < 16 {
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
        // the Reason is only meaningful for pending jobs; squeue
        // reports "None" for jobs that are not waiting on anything
        let reason = match parts[10] {
            "" => None,
            _ if status != JobStatus::Pending => None,
            r => Some(r.to_string()),
        };

        let mut job = Job::new(
            &id,
            &name,
            status,
            &time,
            &partition,
            nodes,
            &workdir,
            &command,
            Some(output),
        );
        job.reason = reason;
        job.priority = parts[11].parse::<u64>().unwrap_or(0);
        job.account = parts[12].to_string();
        job.qos = parts[13].to_string();
        job.cpus = parts[14].parse::<u32>().unwrap_or(0);
        job.nodelist = parts[15].to_string();
        joblist.push(job);
    }
    joblist
}

/// Parses the output of the sacct command into a job list.
///
/// The field order must match the `--format` list in
/// [`SlurmScheduler::sacct_jobs`]:
/// JobID|JobName|State|Elapsed|Partition|NNodes|WorkDir|SubmitLine
/// (--parsable2: '|'-separated, no trailing delimiter; -n: no header)
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

/// Formats a squeue TimeUsed string into "D-HH:MM:SS".
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

/// Formats a squeue PendingTime (seconds) string into "D-HH:MM:SS".
fn format_time_pending(time_str: &str) -> String {
    let time_in_sec = time_str.parse::<u64>().unwrap_or(0);
    let days = time_in_sec / (24 * 3600);
    let hours = (time_in_sec % (24 * 3600)) / 3600;
    let minutes = (time_in_sec % 3600) / 60;
    let seconds = time_in_sec % 60;
    format!("{}-{:02}:{:02}:{:02}", days, hours, minutes, seconds)
}

// ====================================================================
//  EFFICIENCY STATS (seff-style) PARSING
// ====================================================================

/// Parses a Slurm duration into seconds.
///
/// Handles the forms found in the sacct TotalCPU / Elapsed / Timelimit
/// columns: "D-HH:MM:SS", "HH:MM:SS", "MM:SS" and "MM:SS.mmm" (TotalCPU
/// carries fractional seconds). Returns `None` for non-durations such
/// as "UNLIMITED", "Partition_Limit" or an empty field.
pub fn parse_slurm_duration(text: &str) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    // an optional "D-" day prefix
    let (days, rest) = match text.split_once('-') {
        Some((day_str, rest)) => (day_str.parse::<u64>().ok()? as f64, rest),
        None => (0.0, text),
    };
    let parts: Vec<&str> = rest.split(':').collect();
    let (hours, minutes, seconds) = match parts.len() {
        3 => (
            parts[0].parse::<u64>().ok()? as f64,
            parts[1].parse::<u64>().ok()? as f64,
            parse_seconds(parts[2])?,
        ),
        2 => (
            0.0,
            parts[0].parse::<u64>().ok()? as f64,
            parse_seconds(parts[1])?,
        ),
        1 => (0.0, 0.0, parse_seconds(parts[0])?),
        _ => return None,
    };
    Some(days * 86400.0 + hours * 3600.0 + minutes * 60.0 + seconds)
}

/// Parses a (possibly fractional) non-negative seconds field.
fn parse_seconds(text: &str) -> Option<f64> {
    let value = text.parse::<f64>().ok()?;
    if value.is_finite() && value >= 0.0 {
        Some(value)
    } else {
        None
    }
}

/// Parses a memory size with an optional K/M/G/T suffix into bytes,
/// e.g. sacct MaxRSS values like "12345K", "4.5G" or "0". A plain
/// number is taken as bytes. Returns `None` for empty or garbage input.
pub fn parse_mem_size(text: &str) -> Option<f64> {
    let text = text.trim();
    let (number, multiplier) = match text.chars().last()?.to_ascii_uppercase() {
        'K' => (&text[..text.len() - 1], 1024.0),
        'M' => (&text[..text.len() - 1], 1024.0 * 1024.0),
        'G' => (&text[..text.len() - 1], 1024.0 * 1024.0 * 1024.0),
        'T' => (&text[..text.len() - 1], 1024.0 * 1024.0 * 1024.0 * 1024.0),
        c if c.is_ascii_digit() => (text, 1.0),
        _ => return None,
    };
    let value = number.parse::<f64>().ok()?;
    if value.is_finite() && value >= 0.0 {
        Some(value * multiplier)
    } else {
        None
    }
}

/// Parses a sacct ReqMem value into the *total* requested bytes of the
/// job.
///
/// Older Slurm versions append the unit "n" (per node) or "c" (per
/// CPU), e.g. "4000Mn" or "16Gc"; the per-unit value is multiplied by
/// the node/CPU count. Newer versions (>= 21.08) report the total
/// requested memory without a suffix, e.g. "16G".
pub fn parse_req_mem(text: &str, nodes: u32, cpus: u32) -> Option<f64> {
    let text = text.trim();
    let (size, count) = if let Some(size) = text.strip_suffix(['n', 'N']) {
        (size, nodes as f64)
    } else if let Some(size) = text.strip_suffix(['c', 'C']) {
        (size, cpus as f64)
    } else {
        (text, 1.0)
    };
    Some(parse_mem_size(size)? * count)
}

/// Parses the output of the sacct stats query into [`JobStats`].
///
/// The field order must match the `--format` list in
/// [`SlurmScheduler::job_stats`]:
/// JobID|State|TotalCPU|Elapsed|AllocCPUS|MaxRSS|ReqMem|Timelimit|NNodes
/// (--parsable2: '|'-separated, no trailing delimiter; -n: no header).
///
/// The job-level values come from the row whose JobID equals `job_id`;
/// MaxRSS is only recorded on the step rows (".batch" etc.), so the
/// maximum across all rows is used. Returns `None` when sacct has no
/// row for the job; components are `None` when their fields are
/// missing, unparsable or would divide by zero.
pub fn parse_job_stats(output: &str, job_id: &str) -> Option<JobStats> {
    let mut main_row: Option<Vec<String>> = None;
    let mut max_rss: Option<f64> = None;
    for line in output.lines() {
        let fields: Vec<&str> = line.split('|').collect();
        if fields.len() < 9 {
            continue;
        }
        if fields[0].trim() == job_id {
            main_row = Some(fields.iter().map(|f| f.trim().to_string()).collect());
        }
        // MaxRSS lives on the step rows; take the maximum across steps
        if let Some(rss) = parse_mem_size(fields[5]) {
            max_rss = Some(max_rss.map_or(rss, |current: f64| current.max(rss)));
        }
    }
    let main_row = main_row?;

    let total_cpu = parse_slurm_duration(&main_row[2]);
    let elapsed = parse_slurm_duration(&main_row[3]);
    let alloc_cpus = main_row[4].parse::<u32>().ok();
    let nodes = main_row[8].parse::<u32>().unwrap_or(0);
    let req_mem = parse_req_mem(&main_row[6], nodes, alloc_cpus.unwrap_or(0));
    let time_limit = parse_slurm_duration(&main_row[7]);

    let cpu_efficiency = match (total_cpu, elapsed, alloc_cpus) {
        (Some(used), Some(elapsed), Some(cpus)) if elapsed > 0.0 && cpus > 0 => {
            Some(used / (elapsed * cpus as f64))
        }
        _ => None,
    };
    let mem_efficiency = match (max_rss, req_mem) {
        (Some(rss), Some(requested)) if requested > 0.0 => Some(rss / requested),
        _ => None,
    };
    let elapsed_frac_of_limit = match (elapsed, time_limit) {
        (Some(elapsed), Some(limit)) if limit > 0.0 => Some(elapsed / limit),
        _ => None,
    };

    Some(JobStats {
        cpu_efficiency,
        mem_efficiency,
        elapsed_frac_of_limit,
    })
}

// ====================================================================
//  FAKE IMPLEMENTATION FOR TESTS
// ====================================================================

/// A [`Scheduler`] with canned responses that records the calls it
/// receives. Usable by tests in any module of the crate.
#[cfg(test)]
pub struct FakeScheduler {
    pub squeue_response: Result<Vec<Job>, SchedulerError>,
    pub sacct_response: Result<Vec<Job>, SchedulerError>,
    pub details_response: Result<String, SchedulerError>,
    pub stats_response: Result<Option<JobStats>, SchedulerError>,
    pub cancel_response: Result<(), SchedulerError>,
    pub nodes_response: Result<Vec<String>, SchedulerError>,
    pub log_response: Result<String, SchedulerError>,
    /// The squeue commands passed to `squeue_jobs`, in call order.
    pub squeue_commands: std::sync::Mutex<Vec<String>>,
    /// The filters passed to `sacct_jobs`, in call order.
    pub sacct_filters: std::sync::Mutex<Vec<Vec<String>>>,
    /// The job ids passed to `cancel_job`, in call order.
    pub cancelled_jobs: std::sync::Mutex<Vec<String>>,
}

#[cfg(test)]
impl Default for FakeScheduler {
    fn default() -> Self {
        Self {
            squeue_response: Ok(Vec::new()),
            sacct_response: Ok(Vec::new()),
            details_response: Ok(String::new()),
            stats_response: Ok(None),
            cancel_response: Ok(()),
            nodes_response: Ok(Vec::new()),
            log_response: Ok(String::new()),
            squeue_commands: std::sync::Mutex::new(Vec::new()),
            sacct_filters: std::sync::Mutex::new(Vec::new()),
            cancelled_jobs: std::sync::Mutex::new(Vec::new()),
        }
    }
}

#[cfg(test)]
impl Scheduler for FakeScheduler {
    fn squeue_jobs(&self, command: &str) -> Result<Vec<Job>, SchedulerError> {
        self.squeue_commands
            .lock()
            .unwrap()
            .push(command.to_string());
        self.squeue_response.clone()
    }

    fn sacct_jobs(&self, user_filter: &[String]) -> Result<Vec<Job>, SchedulerError> {
        self.sacct_filters
            .lock()
            .unwrap()
            .push(user_filter.to_vec());
        self.sacct_response.clone()
    }

    fn job_details(&self, _job_id: &str) -> Result<String, SchedulerError> {
        self.details_response.clone()
    }

    fn job_stats(&self, _job_id: &str) -> Result<Option<JobStats>, SchedulerError> {
        self.stats_response.clone()
    }

    fn cancel_job(&self, job_id: &str) -> Result<(), SchedulerError> {
        self.cancelled_jobs.lock().unwrap().push(job_id.to_string());
        self.cancel_response.clone()
    }

    fn job_nodes(&self, _job_id: &str) -> Result<Vec<String>, SchedulerError> {
        self.nodes_response.clone()
    }

    fn log_tail(&self, _path: &str, _lines: usize) -> Result<String, SchedulerError> {
        self.log_response.clone()
    }
}

// ====================================================================
//  TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ----------------------------------------------------------------
    // format_sacct_output
    // ----------------------------------------------------------------
    // Field order must match the --format list in sacct_jobs:
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
    // Field order must match the --Format list in squeue_format_arg:
    // JobID, Name, StateCompact, TimeUsed, PendingTime, Partition,
    // NumNodes, WorkDir, Command, StdOut, Reason, PriorityLong,
    // Account, QOS, NumCPUs, NodeList
    // The "|%|" suffix is attached to every field except the last, so a
    // line has 16 fields and 15 delimiters. The first line is the header.

    fn squeue_line(fields: [&str; 16]) -> String {
        fields.join("|%|")
    }

    #[test]
    fn format_squeue_output_parses_running_and_pending_jobs() {
        let header = "JOBID|%|NAME|%|ST|%|TIME|%|PENDING_TIME|%|PARTITION|%|NODES|%|WORK_DIR|%|COMMAND|%|STDOUT|%|REASON|%|PRIORITY|%|ACCOUNT|%|QOS|%|CPUS|%|NODELIST";
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
            "None ",
            "4294901760 ",
            "physics ",
            "normal ",
            "16 ",
            "l[42314-42315] ",
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
            "Priority",
            "1013",
            "chemistry",
            "high",
            "8",
            "",
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
        // the reason is only stored for pending jobs
        assert_eq!(jobs[0].reason, None);
        assert_eq!(jobs[0].priority, 4294901760);
        assert_eq!(jobs[0].account, "physics");
        assert_eq!(jobs[0].qos, "normal");
        assert_eq!(jobs[0].cpus, 16);
        assert_eq!(jobs[0].nodelist, "l[42314-42315]");

        assert_eq!(jobs[1].id, "5678");
        assert_eq!(jobs[1].status, JobStatus::Pending);
        // pending jobs use PendingTime (seconds): 3661 s = 1 h 1 min 1 s
        assert_eq!(jobs[1].time, "0-01:01:01");
        assert_eq!(jobs[1].nodes, 2);
        assert_eq!(jobs[1].reason.as_deref(), Some("Priority"));
        assert_eq!(jobs[1].priority, 1013);
        assert_eq!(jobs[1].account, "chemistry");
        assert_eq!(jobs[1].qos, "high");
        assert_eq!(jobs[1].cpus, 8);
        // a pending job has no allocated nodes yet
        assert_eq!(jobs[1].nodelist, "");
    }

    #[test]
    fn format_squeue_output_keeps_pending_reason_none_verbatim() {
        // a pending job whose reason squeue reports as "None" (freshly
        // submitted) keeps the raw value; the display layer maps it
        let header = "H";
        let pending = squeue_line([
            "1", "job", "PD", "0:00", "5", "gpu", "1", "/w", "/w/r.sh", "/w/o.log", "None", "12",
            "acc", "qos", "4", "",
        ]);
        let output = format!("{}\n{}\n", header, pending);
        let jobs = format_squeue_output(&output);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].reason.as_deref(), Some("None"));

        // an empty reason field stays None
        let pending = squeue_line([
            "1", "job", "PD", "0:00", "5", "gpu", "1", "/w", "/w/r.sh", "/w/o.log", "", "12",
            "acc", "qos", "4", "",
        ]);
        let output = format!("{}\n{}\n", header, pending);
        let jobs = format_squeue_output(&output);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].reason, None);
    }

    #[test]
    fn format_squeue_output_skips_lines_in_the_old_11_field_format() {
        // a line in the old 11-field format (before PriorityLong,
        // Account, QOS, NumCPUs and NodeList were added) is too short
        // for the 16-field bounds guard and is skipped, not mis-parsed
        let output = "H\n1|%|job|%|R|%|1:00|%|0|%|gpu|%|1|%|/w|%|/w/r.sh|%|/w/o.log|%|None\n";
        assert!(format_squeue_output(output).is_empty());
    }

    #[test]
    fn format_squeue_output_tolerates_non_numeric_priority_and_cpus() {
        let header = "H";
        let line = squeue_line([
            "1", "job", "R", "1:00", "0", "gpu", "1", "/w", "/w/r.sh", "/w/o.log", "None", "N/A",
            "acc", "qos", "N/A", "n01",
        ]);
        let output = format!("{}\n{}\n", header, line);
        let jobs = format_squeue_output(&output);
        assert_eq!(jobs.len(), 1);
        // unparsable numeric fields fall back to 0 instead of panicking
        assert_eq!(jobs[0].priority, 0);
        assert_eq!(jobs[0].cpus, 0);
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
    // duration parsing (sacct TotalCPU / Elapsed / Timelimit)
    // ----------------------------------------------------------------

    #[test]
    fn parse_slurm_duration_all_forms() {
        // D-HH:MM:SS
        assert_eq!(parse_slurm_duration("1-02:03:04"), Some(93784.0));
        // HH:MM:SS
        assert_eq!(parse_slurm_duration("02:03:04"), Some(7384.0));
        // MM:SS with fractional seconds (TotalCPU form)
        assert_eq!(parse_slurm_duration("03:04.567"), Some(184.567));
        assert_eq!(parse_slurm_duration("00:00.123"), Some(0.123));
        // MM:SS without fraction
        assert_eq!(parse_slurm_duration("03:04"), Some(184.0));
        // bare seconds
        assert_eq!(parse_slurm_duration("42"), Some(42.0));
        assert_eq!(parse_slurm_duration(" 00:00:00 "), Some(0.0));
    }

    #[test]
    fn parse_slurm_duration_rejects_non_durations() {
        assert_eq!(parse_slurm_duration(""), None);
        assert_eq!(parse_slurm_duration("UNLIMITED"), None);
        assert_eq!(parse_slurm_duration("Partition_Limit"), None);
        assert_eq!(parse_slurm_duration("INVALID"), None);
        assert_eq!(parse_slurm_duration("-5"), None);
        assert_eq!(parse_slurm_duration("1:2:3:4"), None);
        assert_eq!(parse_slurm_duration("NaN"), None);
    }

    // ----------------------------------------------------------------
    // memory size parsing (sacct MaxRSS / ReqMem)
    // ----------------------------------------------------------------

    #[test]
    fn parse_mem_size_suffixes() {
        assert_eq!(parse_mem_size("0"), Some(0.0));
        assert_eq!(parse_mem_size("1024"), Some(1024.0));
        assert_eq!(parse_mem_size("2K"), Some(2048.0));
        assert_eq!(parse_mem_size("12345K"), Some(12345.0 * 1024.0));
        assert_eq!(parse_mem_size("4M"), Some(4.0 * 1024.0 * 1024.0));
        assert_eq!(parse_mem_size("1.5G"), Some(1.5 * 1024.0 * 1024.0 * 1024.0));
        assert_eq!(
            parse_mem_size("2T"),
            Some(2.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0)
        );
        // lowercase suffixes appear in some Slurm versions
        assert_eq!(parse_mem_size("2k"), Some(2048.0));
        assert_eq!(parse_mem_size(" 16G "), Some(16.0 * 1024.0f64.powi(3)));
    }

    #[test]
    fn parse_mem_size_rejects_garbage() {
        assert_eq!(parse_mem_size(""), None);
        assert_eq!(parse_mem_size("K"), None);
        assert_eq!(parse_mem_size("garbage"), None);
        assert_eq!(parse_mem_size("-4K"), None);
        assert_eq!(parse_mem_size("16Q"), None);
    }

    #[test]
    fn parse_req_mem_per_node_per_cpu_and_total() {
        let gib = 1024.0f64.powi(3);
        let mib = 1024.0f64.powi(2);
        // "Mn": per node, multiplied by the node count
        assert_eq!(parse_req_mem("4000Mn", 2, 8), Some(2.0 * 4000.0 * mib));
        // "Gc": per CPU, multiplied by the CPU count
        assert_eq!(parse_req_mem("16Gc", 2, 8), Some(8.0 * 16.0 * gib));
        // plain form (Slurm >= 21.08): the total requested memory
        assert_eq!(parse_req_mem("16G", 2, 8), Some(16.0 * gib));
        // a zero node count yields 0 total; the caller guards div-by-zero
        assert_eq!(parse_req_mem("4000Mn", 0, 0), Some(0.0));
        // garbage and empty input
        assert_eq!(parse_req_mem("", 1, 1), None);
        assert_eq!(parse_req_mem("n", 1, 1), None);
        assert_eq!(parse_req_mem("garbagen", 1, 1), None);
    }

    // ----------------------------------------------------------------
    // parse_job_stats
    // ----------------------------------------------------------------
    // Field order must match the --format list in job_stats:
    // JobID|State|TotalCPU|Elapsed|AllocCPUS|MaxRSS|ReqMem|Timelimit|NNodes

    #[test]
    fn parse_job_stats_completed_job_with_batch_step() {
        // 4 CPUs, 1 h elapsed, 2 h of CPU time used -> 50 % CPU efficiency
        // MaxRSS (on the .batch step) 2 GiB of 8 GiB requested -> 25 %
        // 1 h elapsed of a 2 h limit -> 50 %
        let output = "\
1001|COMPLETED|02:00:00|01:00:00|4||8Gn|02:00:00|1
1001.batch|COMPLETED|01:59:58|01:00:00|4|2097152K|8Gn|02:00:00|1
1001.extern|COMPLETED|00:00:00|01:00:00|4|1024K|8Gn|02:00:00|1
";
        let stats = parse_job_stats(output, "1001").unwrap();
        assert!((stats.cpu_efficiency.unwrap() - 0.5).abs() < 1e-9);
        assert!((stats.mem_efficiency.unwrap() - 0.25).abs() < 1e-9);
        assert!((stats.elapsed_frac_of_limit.unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn parse_job_stats_per_cpu_req_mem_and_fractional_total_cpu() {
        // 2 CPUs at 1 Gc -> 2 GiB total; MaxRSS 1 GiB -> 50 % mem
        // TotalCPU "30:00.500" = 1800.5 s of 2 * 3600 s -> ~25 % CPU
        let output = "\
2002|RUNNING|30:00.500|01:00:00|2||1Gc|04:00:00|1
2002.batch|RUNNING|30:00.500|01:00:00|2|1048576K|1Gc|04:00:00|1
";
        let stats = parse_job_stats(output, "2002").unwrap();
        assert!((stats.cpu_efficiency.unwrap() - 1800.5 / 7200.0).abs() < 1e-9);
        assert!((stats.mem_efficiency.unwrap() - 0.5).abs() < 1e-9);
        assert!((stats.elapsed_frac_of_limit.unwrap() - 0.25).abs() < 1e-9);
    }

    #[test]
    fn parse_job_stats_pending_job_has_no_components() {
        // a pending job: zero elapsed time, no steps, no MaxRSS
        let output = "3003|PENDING|00:00:00|00:00:00|0||4000Mn|01:00:00|0\n";
        let stats = parse_job_stats(output, "3003").unwrap();
        // zero elapsed/CPUs must not divide by zero
        assert_eq!(stats.cpu_efficiency, None);
        // no MaxRSS row and 0 nodes * 4000M = 0 requested
        assert_eq!(stats.mem_efficiency, None);
        // 0 s elapsed of a 1 h limit is a valid 0 %
        assert_eq!(stats.elapsed_frac_of_limit, Some(0.0));
    }

    #[test]
    fn parse_job_stats_unlimited_time_limit_yields_no_time_frac() {
        let output = "\
4004|RUNNING|01:00:00|01:00:00|1||4G|UNLIMITED|1
4004.batch|RUNNING|01:00:00|01:00:00|1|1G|4G|UNLIMITED|1
";
        let stats = parse_job_stats(output, "4004").unwrap();
        assert!((stats.cpu_efficiency.unwrap() - 1.0).abs() < 1e-9);
        assert!((stats.mem_efficiency.unwrap() - 0.25).abs() < 1e-9);
        assert_eq!(stats.elapsed_frac_of_limit, None);
    }

    #[test]
    fn parse_job_stats_missing_job_or_garbage_is_none() {
        assert_eq!(parse_job_stats("", "1001"), None);
        assert_eq!(parse_job_stats("garbage\n\n", "1001"), None);
        // rows of a different job do not match
        let output = "9999|COMPLETED|01:00:00|01:00:00|1|1G|4G|02:00:00|1\n";
        assert_eq!(parse_job_stats(output, "1001"), None);
        // the step row alone (id "1001.batch") is not the main row
        let output = "1001.batch|COMPLETED|01:00:00|01:00:00|1|1G|4G|02:00:00|1\n";
        assert_eq!(parse_job_stats(output, "1001"), None);
    }

    // ----------------------------------------------------------------
    // sacct argument derivation
    // ----------------------------------------------------------------

    #[test]
    fn sacct_user_filter_extracts_only_user_and_cluster() {
        // the user filter carries over; squeue-only flags do not
        // (passing e.g. --state=PD or -p to sacct means something
        // different there or is invalid)
        assert_eq!(
            sacct_user_filter("squeue -u alice --state=PD -p gpu"),
            vec!["-u", "alice"]
        );
        // long forms with and without '='
        assert_eq!(
            sacct_user_filter("squeue --user=bob --clusters=c1 --sort=+i"),
            vec!["-u", "bob", "-M", "c1"]
        );
        assert_eq!(
            sacct_user_filter("squeue --clusters c1 --user carol"),
            vec!["-M", "c1", "-u", "carol"]
        );
    }

    #[test]
    fn sacct_user_filter_without_filters_is_empty() {
        assert!(sacct_user_filter("squeue").is_empty());
        assert!(sacct_user_filter("squeue --state=PD").is_empty());
        // a trailing flag without a value is ignored
        assert!(sacct_user_filter("squeue -u").is_empty());
        assert!(sacct_user_filter("").is_empty());
    }

    // ----------------------------------------------------------------
    // node list parsing
    // ----------------------------------------------------------------

    #[test]
    fn parse_node_list_expands_compressed_lists() {
        // plain node names
        assert_eq!(parse_node_list("node01\n"), vec!["node01"]);
        assert_eq!(parse_node_list("l[42314]"), vec!["l42314"]);
        // ranges and enumerations
        assert_eq!(
            parse_node_list("l[42314-42316]"),
            vec!["l42314", "l42315", "l42316"]
        );
        assert_eq!(
            parse_node_list("l[42314,42316,42319]"),
            vec!["l42314", "l42316", "l42319"]
        );
        // mixed enumerations/ranges and multiple expressions
        assert_eq!(
            parse_node_list("gpu[1,3-5],mem1"),
            vec!["gpu1", "gpu3", "gpu4", "gpu5", "mem1"]
        );
        // zero padding is preserved
        assert_eq!(
            parse_node_list("nid[001-003]"),
            vec!["nid001", "nid002", "nid003"]
        );
        // empty input
        assert!(parse_node_list("").is_empty());
        assert!(parse_node_list("   \n").is_empty());
    }

    #[test]
    fn parse_node_list_handles_malformed_input() {
        // unbalanced brackets are kept verbatim instead of dropped
        assert_eq!(parse_node_list("l[42314"), vec!["l[42314"]);
        // non-numeric and reversed range items are kept verbatim
        assert_eq!(parse_node_list("n[a-b]"), vec!["na-b"]);
        assert_eq!(parse_node_list("n[5-3]"), vec!["n5-3"]);
        // pathological ranges are capped, not expanded endlessly
        let nodes = parse_node_list("n[0-99999999]");
        assert_eq!(nodes.len(), 4096);
        assert_eq!(nodes[0], "n0");
        assert_eq!(nodes[4095], "n4095");
    }

    // ----------------------------------------------------------------
    // read_last_lines
    // ----------------------------------------------------------------

    #[test]
    fn read_last_lines_file_shorter_than_requested() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("short.log");
        std::fs::write(&path, "line1\nline2\nline3\n").unwrap();

        let text = read_last_lines(path.to_str().unwrap(), 100).unwrap();
        assert_eq!(text, "line1\nline2\nline3");

        // a file without a trailing newline keeps its last line
        std::fs::write(&path, "line1\nline2").unwrap();
        let text = read_last_lines(path.to_str().unwrap(), 1).unwrap();
        assert_eq!(text, "line2");

        // an empty file yields an empty tail
        std::fs::write(&path, "").unwrap();
        assert_eq!(read_last_lines(path.to_str().unwrap(), 100).unwrap(), "");
    }

    #[test]
    fn read_last_lines_file_longer_than_requested() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("long.log");
        // ~30 bytes per line * 3000 lines > the 8192-byte chunk size,
        // so the backwards chunked reading path is exercised
        let content: String = (0..3000)
            .map(|i| format!("this is log line number {:06}\n", i))
            .collect();
        std::fs::write(&path, content).unwrap();

        let text = read_last_lines(path.to_str().unwrap(), 100).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines.len(), 100);
        assert_eq!(lines[0], "this is log line number 002900");
        assert_eq!(lines[99], "this is log line number 002999");
    }

    #[test]
    fn read_last_lines_missing_file_is_an_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("does_not_exist.log");

        let result = read_last_lines(path.to_str().unwrap(), 100);
        match result {
            Err(SchedulerError::FileRead { path: p, .. }) => {
                assert!(p.contains("does_not_exist.log"));
            }
            other => panic!("expected FileRead error, got {:?}", other),
        }
    }

    #[test]
    fn read_last_lines_tolerates_non_utf8_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binary.log");
        let mut bytes = b"valid line\n".to_vec();
        bytes.extend_from_slice(&[0xff, 0xfe, 0x80]);
        bytes.extend_from_slice(b" partly valid\nlast line\n");
        std::fs::write(&path, bytes).unwrap();

        let text = read_last_lines(path.to_str().unwrap(), 100).unwrap();
        // invalid bytes are replaced, valid content is preserved
        assert!(text.contains("valid line"));
        assert!(text.contains("last line"));
        assert!(text.contains('\u{FFFD}'));
    }
}
