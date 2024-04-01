use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};
use crossterm::event::{
    KeyCode, KeyEvent, MouseEventKind, MouseButton,};
use tui_textarea::{TextArea, CursorMove};
use std::process::Command;
use std::thread;
use std::sync::mpsc;

use crate::app::Action;
use crate::job::{Job, JobStatus};
use crate::mouse_input::MouseInput;
use crate::user_options::UserOptions;


pub enum WindowFocus {
    JobDetails,
    Log,
}

#[derive(Default)]
pub struct MouseAreas {
    pub joblist_title: Rect,
    pub details_title: Rect,
    pub bottom_symbol: Rect,
    pub log_title: Rect,
    pub joblist: Rect,
    pub categories: Vec<Rect>,
}

#[derive(PartialEq)]
pub enum SortCategory {
    Id,
    Name,
    Status,
    Time,
    Partition,
    Nodes,
}

pub struct JobOverview {
    pub should_render: bool,  // if the window should render
    pub handle_input: bool,   // if the window should handle input
    pub sort_category: SortCategory, // the category to sort 
    pub reversed: bool,       // if the sorting is reversed
    pub collapsed_top: bool,  // if the job list is collapsed
    pub collapsed_bot: bool,  // if the job details are collapsed
    pub focus: WindowFocus,   // which part of the window is in focus
    pub joblist: Vec<Job>,    // the list of jobs
    pub index: usize,         // the index of the job in focus
    pub state: ListState,     // the state of the job list
    pub mouse_areas: MouseAreas, // the mouse areas of the window
    pub squeue_command: TextArea<'static>, // the squeue command
    pub edit_squeue: bool,    // if the squeue command is being edited
    pub refresh_rate: usize,    // the refresh rate of the window
    pub job_details: String,  // the job details
    pub log_height: u16,      // the height of the log section
    pub log: String,          // the log of the job
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl JobOverview {
    pub fn new(refresh_rate: usize) -> Self {
        let mut state = ListState::default();
        state.select(Some(0));
        // create mouse areas with 6 categories
        let mut mouse_areas = MouseAreas::default();
        for _ in 0..6 {
            mouse_areas.categories.push(Rect::default());
        }
        let command = format!("squeue -u {}", whoami());
        let mut textarea = TextArea::from([command]);
        textarea.move_cursor(CursorMove::End);
        Self {
            should_render: true,
            handle_input: true,
            sort_category: SortCategory::Id,
            reversed: false,
            collapsed_top: false,
            collapsed_bot: true,
            focus: WindowFocus::JobDetails,
            joblist: vec![],
            index: 0,
            state: state,
            mouse_areas: mouse_areas,
            squeue_command: textarea,
            edit_squeue: false,
            refresh_rate: refresh_rate,
            job_details: String::new(),
            log_height: 0,
            log: String::new(),
        }
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl JobOverview {
    pub fn sort(&mut self) {
        // only sort if there are jobs
        if self.joblist.is_empty() { return; }
        // get the id of the job in focus
        let id = self.joblist[self.index].id.clone();
        // sort the job list
        match self.sort_category {
            SortCategory::Id => {
                self.joblist.sort_by(|a, b| b.id.cmp(&a.id));
            },
            SortCategory::Name => {
                self.joblist.sort_by(|a, b| a.name.cmp(&b.name));
            },
            SortCategory::Status => {
                self.joblist.sort_by(|a, b| 
                                     a.status.priority().cmp(
                                     &b.status.priority()));
            },
            SortCategory::Time => {
                self.joblist.sort_by(|a, b| a.time.cmp(&b.time));
            },
            SortCategory::Partition => {
                self.joblist.sort_by(|a, b| a.partition.cmp(&b.partition));
            },
            SortCategory::Nodes => {
                self.joblist.sort_by(|a, b| a.nodes.cmp(&b.nodes));
            },
        }
        // reverse the list if needed
        if self.reversed {
            self.joblist.reverse();
        }
        // get the index of the job in focus
        let index = self.joblist.iter().position(|job| job.id == id);
        self.set_index(index.unwrap_or(0) as i32);
    }

    pub fn get_job(&self) -> Option<&Job> {
        self.joblist.get(self.index)
    }

    // pub fn get_jobname(&self) -> Option<String> {
    //     if self.joblist.is_empty() {
    //         return None;
    //     }
    //     Some(self.joblist[self.index].name.clone())
    // }

    pub fn get_squeue_command(&self) -> String {
        self.squeue_command.lines().join("\n")
    }

    pub fn update_joblist(&mut self, user_options: &UserOptions) {
        let id = match self.joblist.is_empty() {
            true => "no_job".to_string(),
            false => self.joblist[self.index].id.clone(),
        };
        if user_options.dummy_jobs {
            // self.joblist = vec![
            //     Job::new(1, "job1", JobStatus::Running, 10, "partition1", 1),
            //     Job::new(13, "job2", JobStatus::Pending, 235, "partition2", 2),
            //     Job::new(15, "job3", JobStatus::Completed, 5123, "partition3", 120),
            //     Job::new(12314, "job4", JobStatus::Failed, 123, "partition4", 1),
            //     Job::new(133, "job5", JobStatus::Running, 10, "partition1", 1),
            //     Job::new(1134, "job6", JobStatus::Pending, 235, "partition2", 2),
            //     Job::new(2, "job7", JobStatus::Completed, 5123, "partition3", 120),
            //     Job::new(194, "job8", JobStatus::Failed, 123, "partition4", 1),
            //     Job::new(139, "job9", JobStatus::Running, 10, "partition1", 1),
            //     Job::new(9577782, "job10", JobStatus::Pending, 9577782, "partition2", 2),];
        } else {
            let command = self.get_squeue_command();
            // setup a thread to get the joblist from squeue
            let command_clone = command.clone();
            let (tx_sq, rx_sq) = mpsc::channel();
            let handle_sq = thread::spawn(move || {
                tx_sq.send(get_squeue_joblist(&command_clone)).unwrap();
            });
            // setup a thread to get the joblist from sacct
            let command_clone = command.clone();
            let (tx_sa, rx_sa) = mpsc::channel();
            let handle_sa = match user_options.show_completed_jobs {
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
            let job_id_clone = id.clone();
            let handle_jd = match self.get_job() {
                Some(job) => {
                    get_job_det = true;
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
            let (tx_log, rx_log) = mpsc::channel();
            let handle_log = match get_job_det {
                true => {
                    let job_details = self.job_details.clone();
                    let log_len = self.log_height;
                    thread::spawn(move || {
                        let log_path = match get_log_path(&job_details) {
                            Some(path) => path,
                            None => {
                                return tx_log.send("No log file found.".to_string()).unwrap()
                            }
                        };
                        tx_log.send(get_log_tail(&log_path, log_len.into())).unwrap();
                    })
                },
                false => thread::spawn(|| {}),
            };

            // collect the joblist from squeue
            self.joblist = rx_sq.recv().unwrap();
            handle_sq.join().unwrap();
            // collect the joblist from sacct
            if user_options.show_completed_jobs {
                self.joblist.extend(rx_sa.recv().unwrap());
                handle_sa.join().unwrap();
            }
            // collect the job details
            if get_job_det {
                self.job_details = rx_jd.recv().unwrap();
                handle_jd.join().unwrap();
                self.log = rx_log.recv().unwrap();
                handle_log.join().unwrap();
            }
            // if a job is JobStatus::Completing, another job JobStatus::Completed exist
            // remove the JobStatus::Completed job
            for (i, job) in self.joblist.iter().enumerate() {
                if job.status == JobStatus::Completing {
                    // get all indexes of jobs with the same id
                    let indexes = self.joblist.iter()
                        .enumerate()
                        .filter(|(_, j)| j.id == job.id)
                        .map(|(i, _)| i)
                        .collect::<Vec<usize>>();
                    for index in indexes {
                        if index != i {
                            self.joblist.remove(index);
                        }
                    }
                    break;
                }
            }

        }
        // check if there is a job with the same id
        let index = self.joblist.iter().position(|job| job.id == id);
        self.set_index(index.unwrap_or(0) as i32);
        self.sort();
    }




        
}

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

fn get_log_path(job_details: &str) -> Option<String> {
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

fn get_log_tail(log_path: &str, lines: usize) -> String {
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


// ====================================================================
//  RENDERING
// ====================================================================

impl JobOverview {
    pub fn render(&mut self, f: &mut Frame, area: &Rect) {
        // only render if the window is active
        if !self.should_render { return; }

        let mut constraints = vec![Constraint::Length(1)];
        if self.collapsed_top && self.collapsed_bot {
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Length(1));
        } else if self.collapsed_top && !self.collapsed_bot {
            constraints.push(Constraint::Length(1));
            constraints.push(Constraint::Min(1));
        } else if !self.collapsed_top && self.collapsed_bot {
            constraints.push(Constraint::Min(1));
            constraints.push(Constraint::Length(1));
        } else {
            constraints.push(Constraint::Percentage(30));
            constraints.push(Constraint::Percentage(70));
        }

        // create a layout for the title
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints.as_slice())
            .split(*area);

        // render the title, job list, and job details
        self.render_title(f, &layout[0]);
        self.render_joblist(f, &layout[1]);
        self.render_bottom_section(f, &layout[2]);
    }

    fn render_title(&self, f: &mut Frame, area: &Rect) {
        f.render_widget(
            Paragraph::new("JOB OVERVIEW")
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center),
            *area,
        );
    }

    // ----------------------------------------------------------------------
    // RENDERING THE JOB LIST
    // ----------------------------------------------------------------------

    fn render_joblist(&mut self, f: &mut Frame, area: &Rect) {
        match self.collapsed_top {
            true => self.render_joblist_collapsed(f, area),
            false => self.render_joblist_extended(f, area),
        }
    }

    fn render_joblist_collapsed(&mut self, f: &mut Frame, area: &Rect) {
        // update the mouse areas
        self.mouse_areas.joblist_title = area.clone();
        self.mouse_areas.joblist = Rect::default();

        if self.joblist.is_empty() {
            let title = format!("▶ Job list (collapsed)");
            f.render_widget(Line::from(title), *area);
            return;
        }

        let job = &self.joblist[self.index];
        let col = get_job_color(job);

        let content_strings = vec![
            "▶ Job: ".to_string(),
            job.id.clone(),
            job.name.clone(),
            format_status(job),
            job.time.clone(),
            job.partition.clone(),
            job.nodes.to_string(),
        ];

        let constraints = content_strings.iter()
            .map(|s| Constraint::Min(s.len() as u16 + 2))
            .collect::<Vec<Constraint>>();

        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints::<Vec<Constraint>>(constraints)
            .split(*area);
        
        // update the mouse areas of the categories
        for category in self.mouse_areas.categories.iter_mut() {
            *category = Rect::default();
        }

        content_strings.iter().enumerate().for_each(|(i, s)| {
            let line = Line::from(s.clone()).
                style(Style::default().fg(col));
            f.render_widget(line, layout[i]);
        });
    }

    fn render_joblist_extended(&mut self, f: &mut Frame, area: &Rect) {
        let title = "▼ Job list: ";
        let title_len = title.len() as u16;

        let refresh_rate = format!(
            "{} ms", self.refresh_rate);
        
        let block = Block::default().title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title(block::Title::from(refresh_rate)
                   .alignment(Alignment::Right));

        // update the mouse areas
        let mut top_row = area.clone();
        top_row.height = 1;
        self.mouse_areas.joblist_title = top_row;
        let mut joblist_area = block.inner(*area).clone();
        joblist_area.y += 1;       // remove the header row
        joblist_area.height -= 1;
        self.mouse_areas.joblist = joblist_area;

        f.render_widget(block.clone(), *area);


        // render the squeue command
        let buffer = self.get_squeue_command();
        let mut squeue_rect = top_row.clone();
        squeue_rect.width = buffer.len() as u16 + 1;
        squeue_rect.x = title_len - 1;
        self.render_squeue_command(f, &squeue_rect);

        if self.joblist.is_empty() {
            self.render_empty_joblist(f, &joblist_area);
            return;
        }

        let id_list   = map2column(&self.joblist, |job| job.id.clone());
        let name_list = map2column(&self.joblist, |job| job.name.clone());
        let stat_list = map2column(&self.joblist, |job| format_status(job));
        let time_list = map2column(&self.joblist, |job| job.time.clone());
        let part_list = map2column(&self.joblist, |job| job.partition.clone());
        let node_list = map2column(&self.joblist, |job| job.nodes.to_string());

        let constraints = [
            Constraint::Min(get_column_width(&id_list,   8)),
            Constraint::Min(get_column_width(&name_list, 10)),
            Constraint::Min(get_column_width(&stat_list, 8)),
            Constraint::Min(get_column_width(&time_list, 6)),
            Constraint::Min(get_column_width(&part_list, 11)),
            Constraint::Min(get_column_width(&node_list, 7))
        ];

        // create a layout for the job list
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints.as_ref())
            .split(block.inner(*area));

        // update the mouse areas of the categories
        for (i, area) in layout.iter().enumerate() {
            let mut cat_area = area.clone();
            cat_area.height = 1;
            self.mouse_areas.categories[i] = cat_area;
        }

        let hc = get_job_color(&self.joblist[self.index]);
        let state = &mut self.state;
        
        let mut title_names = vec!["ID", "Name", "Status", 
                               "Time", "Partition", "Nodes"];
        // modify the title names if the category is selected
        let cat_ind = match self.sort_category {
            SortCategory::Id => 0,
            SortCategory::Name => 1,
            SortCategory::Status => 2,
            SortCategory::Time => 3,
            SortCategory::Partition => 4,
            SortCategory::Nodes => 5,
        };
        let new_title = format!("{} {}", 
                           title_names[cat_ind],
                           if self.reversed { "▲" } else { "▼" });
        title_names[cat_ind] = new_title.as_str();

        let row_lists = vec![id_list, name_list, stat_list, 
                             time_list, part_list, node_list];

        for i in 0..6 {
            render_row(state, f, &layout[i], title_names[i], 
                       row_lists[i].clone(), hc);
        }

    }

    fn render_squeue_command(&mut self, f: &mut Frame, area: &Rect) {
        let textarea = &mut self.squeue_command;
        if self.edit_squeue {
            textarea.set_cursor_style(Style::default().bg(Color::Red));
            textarea.set_cursor_line_style(
                Style::default().fg(Color::Red)
                .add_modifier(Modifier::BOLD));
        } else {
            textarea.set_cursor_line_style(Style::default());
            textarea.set_cursor_style(Style::default());
        }
        f.render_widget(textarea.widget(), *area);
    }

    fn render_empty_joblist(&self, f: &mut Frame, area: &Rect) {
        let text = "No jobs found";
        let text = Span::styled(text, Style::default().fg(Color::Gray));
        let paragraph = Paragraph::new(text)
            .alignment(Alignment::Center);
        f.render_widget(paragraph, *area);
    }

    // ----------------------------------------------------------------------
    // RENDERING THE JOB DETAILS AND LOG SECTION
    // ----------------------------------------------------------------------
    fn render_bottom_section(&mut self, f: &mut Frame, area: &Rect) {
        self.log_height = area.height.saturating_sub(2);
        match self.collapsed_bot {
            true => self.render_bottom_collapsed(f, area),
            false => self.render_bottom_extended(f, area),
        }
    }

    fn render_bottom_collapsed(&mut self, f: &mut Frame, area: &Rect) {
        let title = vec!{
            Span::raw("▶ "),
            Span::raw("1. Job details"), 
            Span::raw("  "),
            Span::raw("2. Log"),
        };

        // update the mouse areas
        self.update_bottom_mouse_positions(area, title.clone(), 0);

        let line = Line::from(title).
            style(Style::default().fg(Color::Gray));
        f.render_widget(line, *area);
    }

    fn render_bottom_extended(&mut self, f: &mut Frame, area: &Rect) {
        let mut title = vec!{
            Span::raw("▼ "),
            Span::raw("1. Job details"), 
            Span::raw("  "),
            Span::raw("2. Log"),
        };

        // update the mouse areas
        self.update_bottom_mouse_positions(area, title.clone(), 1);

        match self.focus {
            WindowFocus::JobDetails => {
                title[1] = Span::styled("1. Job details", 
                                        Style::default().fg(Color::Blue));
            },
            WindowFocus::Log => {
                title[3] = Span::styled("2. Log", 
                                        Style::default().fg(Color::Blue));
            },
        }
        
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        f.render_widget(block.clone(), *area);
        let rect = block.inner(*area);
        match self.focus {
            WindowFocus::JobDetails => {
                self.render_job_details(f, &rect);
            },
            WindowFocus::Log => {
                self.render_log(f, &rect);
            },
        }
    }

    fn render_job_details(&self, f: &mut Frame, area: &Rect) {

        let paragraph = Paragraph::new(self.job_details.clone())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, *area);
    }

    fn render_log(&self, f: &mut Frame, area: &Rect) {
        let paragraph = Paragraph::new(self.log.clone())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, *area);
    }

    fn update_bottom_mouse_positions(
        &mut self, area: &Rect, title: Vec<Span>, offset: u16) {
        if title.len() != 4 { return; }
        let mut top_row = area.clone();
        top_row.height = 1;
        let mut symbol = top_row.clone();
        symbol.width = title[0].width() as u16;
        symbol.x += offset;
        let mut details_title = top_row.clone();
        details_title.width = title[1].width() as u16;
        details_title.x += symbol.width + symbol.x;
        let mut log_title = top_row.clone();
        log_title.width = title[3].width() as u16;
        log_title.x += details_title.width + details_title.x + 2;
        self.mouse_areas.bottom_symbol = symbol;
        self.mouse_areas.details_title = details_title;
        self.mouse_areas.log_title = log_title;
    }

}

fn get_job_color(job: &Job) -> Color {
    match job.status {
        JobStatus::Running => Color::Green,
        JobStatus::Pending => Color::Yellow,
        JobStatus::Completing => Color::Yellow,
        JobStatus::Completed => Color::Gray,
        JobStatus::Failed => Color::Red,
        JobStatus::Timeout => Color::Red,
        JobStatus::Cancelled => Color::Red,
        JobStatus::Unknown => Color::Red,
    }
}

fn format_status(job: &Job) -> String {
    match job.status {
        JobStatus::Unknown => "Unknown",
        JobStatus::Running => "Running",
        JobStatus::Pending => "Pending",
        JobStatus::Completed => "Completed",
        JobStatus::Completing => "Completing",
        JobStatus::Failed => "Failed",
        JobStatus::Timeout => "Timeout",
        JobStatus::Cancelled => "Cancelled",
    }.to_string()
}

// fn format_time(job: &Job) -> String {
//     let time = job.time;
//     let hours = time / 3600;
//     let minutes = (time % 3600) / 60;
//     let seconds = time % 60;
//     format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
// }

fn get_column_width(column: &Vec<Line>, minimum: u16) -> u16 {
    column.iter()
        .map(|item| item.width()).max().unwrap_or(0)
        .max(minimum.into()) as u16
}

fn map2column<F>(joblist: &Vec<Job>, map_fn: F) -> Vec<Line>
where
F: Fn(&Job) -> String,
{
    joblist.iter()
        .map(|job| {
            Line::styled(
                map_fn(job), 
                Style::default().fg(get_job_color(job)))
                .alignment(Alignment::Right)
        }).collect()
}

fn render_row(state: &mut ListState, 
              f: &mut Frame, 
              area: &Rect, title: &str, 
              content: Vec<Line>,
              highlight_color: Color) {
    // create a bold styled centered title
    let title = Span::styled(title, 
                             Style::default().add_modifier(Modifier::BOLD));
    let title = Paragraph::new(title)
        .alignment(Alignment::Right);

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(*area);

    f.render_widget(title, layout[0]);

    let item_list = List::new(content)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD)
                         .bg(highlight_color).fg(Color::Black));

    // align the list to the right


    f.render_stateful_widget(item_list, layout[1], state);
}

// ====================================================================
//  USER INPUT
// ====================================================================

impl JobOverview {
    /// Handle user input for the job overview window
    /// Returns true if the input was handled
    /// Returns false if the input was not handled
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent)
        -> bool {
        if !self.handle_input { return false; }

        if self.edit_squeue {
            match key_event.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.edit_squeue = false;
                    return true;
                },
                _ => {
                    self.squeue_command.input(key_event);
                    return true;
                },
            }
        }

        match key_event.code {
            // Escaping the program
            KeyCode::Char('q') => {
                *action = Action::Quit;
            },
            // Next / Previous job
            KeyCode::Down | KeyCode::Char('j') => {
                self.next_job();
            },
            KeyCode::Up | KeyCode::Char('k') => {
                self.prev_job();
            },
            // Open job action menu
            KeyCode::Enter | KeyCode::Char('l') => {
                *action = Action::OpenJobAction;
            },
            // Change sorting category
            KeyCode::Tab => {
                self.next_sort_category();
                *action = Action::SortJobList;
            },
            KeyCode::Char('r') => {
                self.reversed = !self.reversed;
                *action = Action::SortJobList;
            },
            // Switching focus between job details and log
            KeyCode::Char('1') => {
                self.select_details();
            },
            KeyCode::Char('2') => {
                self.select_log();
            },
            KeyCode::Right => {
                self.next_focus();
            },
            KeyCode::Left => {
                self.prev_focus();
            },
            // Open job allocation menu
            KeyCode::Char('a') => {
                *action = Action::OpenJobAllocation;
            },
            KeyCode::Char('o') => {
                *action = Action::OpenUserOptions;
            },
            // Collapsing/Extending the joblist
            KeyCode::Char('m') => {
                self.collapsed_top = !self.collapsed_top;
            },
            KeyCode::Char('n') => {
                self.collapsed_bot = !self.collapsed_bot;
            },
            // Edit the squeue command
            KeyCode::Char('/') => {
                self.collapsed_top = false;
                self.edit_squeue = true;
            },
            _ => {return false;},
        };
        true
    }

    fn select_details(&mut self) {
        self.focus = WindowFocus::JobDetails;
        self.collapsed_bot = false;
    }

    fn select_log(&mut self) {
        self.focus = WindowFocus::Log;
        self.collapsed_bot = false;
    }

    fn next_focus(&mut self) {
        match self.focus {
            WindowFocus::JobDetails => {
                self.focus = WindowFocus::Log;
            },
            WindowFocus::Log => {
                self.focus = WindowFocus::JobDetails;
            },
        }
    }

    fn prev_focus(&mut self) {
        match self.focus {
            WindowFocus::JobDetails => {
                self.focus = WindowFocus::Log;
            },
            WindowFocus::Log => {
                self.focus = WindowFocus::JobDetails;
            },
        }
    }

    fn next_job(&mut self) {
        self.set_index(self.index as i32 + 1);
    }

    fn prev_job(&mut self) {
        self.set_index(self.index as i32 - 1);
    }

    pub fn set_index(&mut self, index: i32) {
        let job_len = self.joblist.len() as i32;
        let mut new_index = index;
        if index >= job_len {
            new_index = 0;
        } else if index < 0 {
            new_index = job_len - 1;
        } 
        self.index = new_index as usize;
        self.state.select(Some(self.index));
    }

    fn next_sort_category(&mut self) {
        match self.sort_category {
            SortCategory::Id => {
                self.sort_category = SortCategory::Name;
            },
            SortCategory::Name => {
                self.sort_category = SortCategory::Status;
            },
            SortCategory::Status => {
                self.sort_category = SortCategory::Time;
            },
            SortCategory::Time => {
                self.sort_category = SortCategory::Partition;
            },
            SortCategory::Partition => {
                self.sort_category = SortCategory::Nodes;
            },
            SortCategory::Nodes => {
                self.sort_category = SortCategory::Id;
            },
        }
    }
}

// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl JobOverview {
    pub fn mouse_input(
        &mut self, action: &mut Action, mouse_input: &mut MouseInput,) { 

        if !self.handle_input { return; }
        let mouse_pos = mouse_input.get_position();

        if let Some(event_kind) = mouse_input.kind() {
            if self.edit_squeue {
                self.edit_squeue = false;
                return;
            }

            match event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // joblist title
                    if self.mouse_areas.joblist_title.contains(mouse_pos) {
                        self.collapsed_top = !self.collapsed_top;
                        mouse_input.click();
                    }
                    // joblist categories
                    for (i, category) in self.mouse_areas
                                             .categories.iter().enumerate() {
                        if category.contains(mouse_pos) {
                            let new_cat = match i {
                                0 => SortCategory::Id,
                                1 => SortCategory::Name,
                                2 => SortCategory::Status,
                                3 => SortCategory::Time,
                                4 => SortCategory::Partition,
                                5 => SortCategory::Nodes,
                                _ => SortCategory::Id,
                            };
                            // reverse sorting if same category is selected
                            if new_cat == self.sort_category {
                                self.reversed = !self.reversed;
                            } else {
                                self.sort_category = new_cat;
                            };
                            *action = Action::SortJobList;
                            mouse_input.click();
                        }
                    }
                    // joblist entries
                    if self.mouse_areas.joblist.contains(mouse_pos) {
                        let rel_y = mouse_pos.y - self.mouse_areas.joblist.y;
                        let mut new_index = rel_y as i32 + self.state.offset() as i32;
                        new_index = new_index.min(self.joblist.len() as i32 - 1);
                        self.set_index(new_index);
                        if mouse_input.is_double_click() {
                            *action = Action::OpenJobAction;
                        }
                        mouse_input.click();
                    }
                    // collapse symbol
                    if self.mouse_areas.bottom_symbol.contains(mouse_pos) {
                        self.collapsed_bot = !self.collapsed_bot;
                        mouse_input.click();
                    }
                    // details title
                    if self.mouse_areas.details_title.contains(mouse_pos) {
                        self.focus = WindowFocus::JobDetails;
                        self.collapsed_bot = false;
                        mouse_input.click();
                    }
                    // log title
                    if self.mouse_areas.log_title.contains(mouse_pos) {
                        self.focus = WindowFocus::Log;
                        self.collapsed_bot = false;
                        mouse_input.click();
                    }
                },
                MouseEventKind::ScrollDown => {
                    self.next_job();
                },
                MouseEventKind::ScrollUp => {
                    self.prev_job();
                },
                _ => {},
            }
        }



    }
}

// ====================================================================
//  TESTS
// ====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_log_path() {
        let job_details = "
JobId=9638603 JobName=dummy_name
   NodeList=l[50321-50324,50327,50330,50333,50337-50338,50340-50341,50343]
   BatchHost=l50321
   Command=./path/to/command.run
   WorkDir=/work/dir
   StdErr=/std/LOG.err
   StdIn=/dev/null
   StdOut=/std/LOG.out";
        let log_path = get_log_path(job_details);
        assert_eq!(log_path, Some("/std/LOG.out".to_string()));
    }
}
