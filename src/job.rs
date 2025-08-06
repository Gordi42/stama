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
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl Job {
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
            status: status,
            time: time.to_string(),
            partition: partition.to_string(),
            nodes: nodes,
            workdir: workdir.to_string(),
            command: command.to_string(),
            output: output,
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
        }
    }
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
        match self.status {
            JobStatus::Completed => true,
            JobStatus::Failed => true,
            JobStatus::Timeout => true,
            JobStatus::Cancelled => true,
            _ => false,
        }
    }
}
