use crate::job::Job;
use std::sync::mpsc;
use std::thread;
use std::process::Command;
use crate::job::JobStatus;
use crate::user_options::UserOptions;


#[derive(Debug, Clone)]
pub struct Content {
    pub job: Option<Job>,
    pub job_list: Vec<Job>,
    pub details_text: String,
    pub log_text: String,
}

impl Content {
    pub fn new(job: Option<Job>, job_list: Vec<Job>, 
               details_text: String, log_text: String) -> Self {
        Self {
            job: job,
            job_list: job_list,
            details_text: details_text,
            log_text: log_text,
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

impl ContentUpdater {
    pub fn new() -> Self {
        Self {
            my_process: None,
        }
    }
   
    pub fn tick(&mut self, job: Option<Job>, command: String, 
                options: UserOptions) -> Option<Content> {
        // check if there is already a job queued
        // if not send the new job
        match &self.my_process {
            Some(my_process) => {
                // try to receive the content
                match my_process.receiver.try_recv() {
                    Ok(content) => {
                        self.start_new_process(job, command, options);
                        Some(content)
                    }
                    Err(_) => {
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

    fn start_new_process(
        &mut self, job: Option<Job>, command: String, options: UserOptions) {
        let (tx, rx) = mpsc::channel();
        let handler = thread::spawn(move || {
            tx.send(get_content(job, command, options)).unwrap_or(());
        });
        self.my_process = Some(MyProcess {
            receiver: rx,
            handler: handler,
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
        true => {
            thread::spawn(move || {
                tx_sa.send(get_acct_joblist(&command_clone)).unwrap();
            })
        },
        false => thread::spawn(|| {}),
    };
    // setup a thread to get the job details
    let (tx_jd, rx_jd) = mpsc::channel();
    let get_job_det: bool;
    let handle_jd = match job {
        Some(ref job) => {
            get_job_det = true;
            let job_id_clone = job.id.clone();
            thread::spawn(move || {
                tx_jd.send(get_job_details(&job_id_clone)).unwrap();
            })
        },
        None => {
            get_job_det = false;
            thread::spawn(|| {})
        }
    };
    // setup a thread to get the log
    // let (tx_log, rx_log) = mpsc::channel();
    // let handle_log = match get_job_det {
    //     true => {
    //         let job_details = self.job_details.clone();
    //         let log_len = self.log_height;
    //         thread::spawn(move || {
    //             let log_path = match get_log_path(&job_details) {
    //                 Some(path) => path,
    //                 None => {
    //                     return tx_log.send("No log file found.".to_string()).unwrap()
    //                 }
    //             };
    //             tx_log.send(get_log_tail(&log_path, log_len.into())).unwrap();
    //         })
    //     },
    //     false => thread::spawn(|| {}),
    // };

    // collect the joblist from squeue
    let mut joblist = rx_sq.recv().unwrap();
    handle_sq.join().unwrap();
    // collect the joblist from sacct
    if options.show_completed_jobs {
        joblist.extend(rx_sa.recv().unwrap());
        handle_sa.join().unwrap();
    }
    let mut details_text = "No job selected".to_string();
    // collect the job details
    if get_job_det {
        details_text = rx_jd.recv().unwrap();
        handle_jd.join().unwrap();
        // self.log = rx_log.recv().unwrap();
        // handle_log.join().unwrap();
    }
    // if a job is JobStatus::Completing, another job JobStatus::Completed exist
    // remove the JobStatus::Completed job
    for (i, job) in joblist.iter().enumerate() {
        if job.status == JobStatus::Completing {
            // get all indexes of jobs with the same id
            let indexes = joblist.iter()
                .enumerate()
                .filter(|(_, j)| j.id == job.id)
                .map(|(i, _)| i)
                .collect::<Vec<usize>>();
            for index in indexes {
                if index != i {
                    joblist.remove(index);
                }
            }
            break;
        }
    }

    let log_text = String::new();
    Content::new(job, joblist, details_text, log_text)
}


fn get_squeue_joblist(command: &str) -> Vec<Job> {
    let format_entries = vec![
        "JobID:16", "Name:16", "StateCompact:2", "TimeUsed:16", 
        "PendingTime:16", "Partition:16", "NumNodes:8",];
        // "WorkDir:256", "Command:256", "StdOut:256"];
    let format = format_entries.join("|%|,");
    let command = format!("{} --Format=\",{},\"", command, format);
    let output = get_squeue_output(&command);
    format_squeue_output(&output)
}

pub fn get_squeue_output(command: &str) -> String {
    // split the command into first word and the rest
    let mut parts = command.trim().split_whitespace();
    let program = parts.next().unwrap_or(" ");
    let args: Vec<&str> = parts.collect();

    let command_stat = Command::new(program)
        .args(args)
        .output();

    match command_stat {
        Ok(output) => {
            if !output.status.success() {
                return "Error executing command".to_string();
            }
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        },
        Err(_) => {
            "Error executing squeue".to_string()
        },
    }
}

pub fn format_squeue_output(output: &str) -> Vec<Job> {
    let mut joblist = vec![];
    for line in output.lines().skip(1) {
        let parts = line.split("|%|").map(|s| s.trim()).collect::<Vec<&str>>();
        // if parts.len() < 11 { continue; }
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
        joblist.push(Job::new(&id, &name, status, &time, &partition, nodes));
    }
    joblist
}

fn get_acct_joblist(command: &str) -> Vec<Job> {
    let output = get_sacct_output(command);
    format_sacct_output(&output)
}


pub fn get_sacct_output(command: &str) -> String {
    let mut parts = command.trim().split_whitespace();
    let _program = parts.next().unwrap_or(" ");
    let args: Vec<&str> = parts.collect();

    let entries = vec![
        "JobID%16", "JobName%16", "State%16", 
        "Elapsed%16", "Partition%16", "NNodes%16",];
    let format = entries.join(",");
    let format_arg = format!("--format={}", format);

    let command_stat = Command::new("sacct")
        .args(args)
        .args(&[format_arg, "-n".to_string()])
        .output();
    match command_stat {
        Ok(output) => {
            if !output.status.success() {
                return "Error executing sacct".to_string();
            }
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        },
        Err(_) => {
            "Error executing sacct".to_string()
        },
    }
}

pub fn format_sacct_output(output: &str) -> Vec<Job> {
    let mut joblist = vec![];
    for line in output.lines().skip(2) {

        let partition = line[4*17..5*17].trim();
        if partition.is_empty() { continue; }
        let id = line[0..17].trim();
        let name = line[17..2*17].trim().to_string();
        let status_text = line[2*17..3*17].trim();
        let status = if status_text.starts_with("COMPLETED") {
            JobStatus::Completed
        } else if status_text.starts_with("TIMEOUT") {
            JobStatus::Timeout
        } else if status_text.starts_with("CANCELLED") {
            JobStatus::Cancelled
        } else if status_text.starts_with("RUNNING") {
            continue;
        } else if status_text.starts_with("PENDING") {
            continue;
        }
        else {
            JobStatus::Unknown
        };
        let time = line[3*17..4*17].trim().to_string();
        let nodes = line[5*17..6*17].trim().parse::<u32>().unwrap_or(0);
        joblist.push(Job::new(&id, &name, status, &time, partition, nodes));
    }
    joblist
}


pub fn get_job_details(job_id: &str) -> String {
    let args = vec!["show", "job", &job_id];
    let command_stat = Command::new("scontrol")
        .args(args)
        .output();
    match command_stat {
        Ok(output) => {
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        },
        Err(e) => {
            e.to_string()
        },
    }
}

fn _get_log_path(job_details: &str) -> Option<String> {
    let mut log_path = String::new();
    for line in job_details.lines() {
        if line.replace(" ", "").starts_with("StdOut=") {
            log_path = line.split('=').collect::<Vec<&str>>()[1].to_string();
            break;
        }
    }
    if log_path.is_empty() {
        return None;
    }
    Some(log_path)
}

fn _get_log_tail(log_path: &str, lines: usize) -> String {
    let command_stat = Command::new("tail")
        .arg("-n")
        .arg(lines.to_string())
        .arg(log_path)
        .output();
    match command_stat {
        Ok(output) => {
            let output = String::from_utf8_lossy(&output.stdout);
            output.to_string()
        },
        Err(e) => {
            e.to_string()
        },
    }
}

fn format_time_used(time_str: &str) -> String {
    // format the time string in D-HH:MM:SS
    let mut time_output = "0-00:00:00".to_string();
    if time_str.len() <= time_output.len() {
        let start_ind = time_output.len() - time_str.len();
        time_output.replace_range(start_ind.., &time_str);
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
