use crossterm::event::{KeyCode, KeyEvent, MouseButton, MouseEventKind};
use ratatui::{
    layout::Flex,
    prelude::*,
    style::{Color, Style},
    widgets::*,
};
use ratatui_textarea::{CursorMove, TextArea};

use crate::app::Action;
use crate::columns::JobColumn;
use crate::job::{explain_reason, reason_code, Job, JobStatus};
use crate::job_rows::JobRow;
use crate::joblist::{JobList, JobListAction};
use crate::menus::help::HelpContext;
use crate::menus::OpenMenu;
use crate::mouse_input::MouseInput;

#[derive(Debug, Clone, PartialEq)]
pub enum WindowFocus {
    JobDetails,
    Log,
}

#[derive(Default)]
pub struct MouseAreas {
    pub joblist_title: Rect,
    pub squeue_command: Rect,
    pub details_title: Rect,
    pub bottom_symbol: Rect,
    pub log_title: Rect,
    pub joblist: Rect,
    pub categories: Vec<Rect>,
}

pub struct JobOverview {
    pub collapsed_top: bool,               // if the job list is collapsed
    pub collapsed_bot: bool,               // if the job details are collapsed
    pub focus: WindowFocus,                // which part of the window is in focus
    pub state: TableState,                 // the state of the job list
    pub mouse_areas: MouseAreas,           // the mouse areas of the window
    pub squeue_command: TextArea<'static>, // the squeue command
    pub edit_squeue: bool,                 // if the squeue command is being edited
    pub refresh_rate: usize,               // the refresh rate of the window
    pub log_height: u16,                   // the height of the log section
    pub columns: Vec<JobColumn>,           // the configured job table columns
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl JobOverview {
    pub fn new(refresh_rate: usize, squeue_command: &str, columns: Vec<JobColumn>) -> Self {
        let mut state = TableState::default();
        state.select(Some(0));
        // create one mouse area per configured column
        let mut mouse_areas = MouseAreas::default();
        for _ in 0..columns.len() {
            mouse_areas.categories.push(Rect::default());
        }
        let command = squeue_command.to_string();
        let mut textarea = TextArea::from([command]);
        textarea.move_cursor(CursorMove::End);
        Self {
            collapsed_top: false,
            collapsed_bot: true,
            focus: WindowFocus::JobDetails,
            state,
            mouse_areas,
            squeue_command: textarea,
            edit_squeue: false,
            refresh_rate,
            log_height: 0,
            columns,
        }
    }
}

// ====================================================================
//  METHODS
// ====================================================================

impl JobOverview {
    fn get_squeue_command(&self) -> String {
        self.squeue_command.lines().join("\n")
    }

    fn exit_squeue_edit(&mut self, action: &mut Action) {
        let new_command = self.get_squeue_command();
        *action = Action::UpdateJobList(JobListAction::UpdateSqueueCommand(new_command));
        self.edit_squeue = false;
    }
}

// ====================================================================
//  RENDERING
// ====================================================================

impl JobOverview {
    pub fn render(&mut self, f: &mut Frame, area: &Rect, jobs: &JobList) {
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
        self.render_joblist(f, &layout[1], jobs);
        self.render_bottom_section(f, &layout[2], jobs);
    }

    fn render_title(&self, f: &mut Frame, area: &Rect) {
        f.render_widget(
            Paragraph::new("SLURM TASK MANAGER")
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center),
            *area,
        );
    }

    // ----------------------------------------------------------------------
    // RENDERING THE JOB LIST
    // ----------------------------------------------------------------------

    fn render_joblist(&mut self, f: &mut Frame, area: &Rect, jobs: &JobList) {
        // set the state of the table
        self.state.select(Some(jobs.get_index()));
        match self.collapsed_top {
            true => self.render_joblist_collapsed(f, area, jobs),
            false => self.render_joblist_extended(f, area, jobs),
        }
    }

    fn render_joblist_collapsed(&mut self, f: &mut Frame, area: &Rect, jobs: &JobList) {
        // update the mouse areas
        self.mouse_areas.joblist_title = *area;
        self.mouse_areas.joblist = Rect::default();

        let job = match jobs.get_job() {
            Some(job) => job,
            None => {
                let title = "▶ Job list (collapsed)".to_string();
                f.render_widget(Line::from(title), *area);
                return;
            }
        };

        let col = get_job_color(job);

        // the collapsed one-line row shows the configured columns
        let mut content_strings = vec!["▶ Job: ".to_string()];
        content_strings.extend(self.columns.iter().map(|column| column.cell(job)));

        let constraints = content_strings
            .iter()
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
            let line = Line::from(s.clone()).style(Style::default().fg(col));
            f.render_widget(line, layout[i]);
        });
    }

    fn render_joblist_extended(&mut self, f: &mut Frame, area: &Rect, jobs: &JobList) {
        let title = "▼ Job list: ";
        let title_len = title.len() as u16;

        let refresh_rate = format!("{} ms", self.refresh_rate);

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .title_top(Line::from(refresh_rate).alignment(Alignment::Right));

        // update the mouse areas
        // clip manually constructed rects to the frame area: rendering a
        // widget into a rect that extends beyond the buffer panics in
        // ratatui (Buffer::index_of) on narrow terminals
        let mut top_row = *area;
        top_row.height = 1;
        top_row.width = title_len - 2;
        let top_row = top_row.intersection(f.area());
        self.mouse_areas.joblist_title = top_row;
        let mut joblist_area = block.inner(*area);

        f.render_widget(block.clone(), *area);

        // render the squeue command
        let buffer = self.get_squeue_command();
        let mut squeue_rect = *area;
        squeue_rect.height = 1;
        squeue_rect.width = buffer.len() as u16 + 1;
        squeue_rect.x = title_len - 1;
        let squeue_rect = squeue_rect.intersection(f.area());
        self.mouse_areas.squeue_command = squeue_rect;
        if !squeue_rect.is_empty() {
            self.render_squeue_command(f, &squeue_rect);
        }

        if jobs.is_empty() {
            self.render_empty_joblist(f, &joblist_area);
            return;
        }

        // ----------------------------------------------
        //  CREATE THE JOB LIST
        // ----------------------------------------------

        // Create the titles for the configured columns
        let mut title_names = self
            .columns
            .iter()
            .map(|column| Span::raw(column.header()))
            .collect::<Vec<Span>>();
        // mark the sort category with a direction arrow (the sort
        // category may not be displayed; then no title is marked)
        let cat_ind = self
            .columns
            .iter()
            .position(|column| column == jobs.get_sort_category());
        if let Some(cat_ind) = cat_ind {
            let new_title = format!(
                "{} {}",
                self.columns[cat_ind].header(),
                if jobs.is_reverse() { "▲" } else { "▼" }
            );
            title_names[cat_ind] = Span::styled(new_title, Style::default().fg(Color::Blue));
        }

        // Create the rows for the job list: single jobs, array-group
        // headers and (for expanded groups) indented task rows
        let rows = jobs
            .rows()
            .iter()
            .map(|row| self.render_row(row, jobs))
            .collect::<Vec<Row>>();

        // Create the widths for the columns
        let widths = self
            .columns
            .iter()
            .map(|column| Constraint::Min(column.min_width()))
            .collect::<Vec<Constraint>>();

        // set the flex and spacing for the columns

        let flex = Flex::SpaceBetween;
        let column_spacing = 1;

        // get the rects for the columnss and update the mouse areas
        let mut rects = Layout::horizontal(widths.clone())
            .flex(flex)
            .spacing(column_spacing)
            .split(joblist_area);
        // set height of each rect to 1
        rects = rects
            .iter()
            .map(|rect| {
                let mut r = *rect;
                r.height = 1;
                r
            })
            .collect();
        self.mouse_areas.categories = rects.to_vec();

        // create the table

        let table = Table::new(rows, widths)
            .column_spacing(column_spacing)
            .header(Row::new(title_names).style(Style::new().bold()))
            .flex(flex)
            .row_highlight_style(Style::new().reversed());

        // render the table
        f.render_stateful_widget(table, joblist_area, &mut self.state);

        // update the mouse areas
        joblist_area.y += 1; // remove the header row
        joblist_area.height = joblist_area.height.saturating_sub(1);
        self.mouse_areas.joblist = joblist_area;
    }

    /// Builds one table row for a display row of the job list.
    fn render_row<'a>(&self, row: &JobRow, jobs: &'a JobList) -> Row<'a> {
        match row {
            JobRow::Single { job_index } => {
                let job = &jobs.jobs[*job_index];
                Row::new(
                    self.columns
                        .iter()
                        .map(|column| column.cell(job))
                        .collect::<Vec<String>>(),
                )
                .style(Style::default().fg(get_job_color(job)))
            }
            JobRow::Group {
                base_id,
                task_indices,
                expanded,
            } => {
                let tasks: Vec<&Job> = task_indices.iter().map(|&i| &jobs.jobs[i]).collect();
                Row::new(
                    self.columns
                        .iter()
                        .map(|column| column.group_cell(base_id, &tasks, *expanded))
                        .collect::<Vec<String>>(),
                )
                .style(Style::default().fg(get_group_color(&tasks)))
            }
            JobRow::Task { job_index, last } => {
                let job = &jobs.jobs[*job_index];
                Row::new(
                    self.columns
                        .iter()
                        .map(|column| {
                            // indent the id with a tree glyph to show the
                            // task belongs to the group header above
                            if *column == JobColumn::Id {
                                let glyph = if *last { "└" } else { "├" };
                                format!("{} {}", glyph, job.id)
                            } else {
                                column.cell(job)
                            }
                        })
                        .collect::<Vec<String>>(),
                )
                .style(Style::default().fg(get_job_color(job)))
            }
        }
    }

    fn render_squeue_command(&mut self, f: &mut Frame, area: &Rect) {
        let textarea = &mut self.squeue_command;
        if self.edit_squeue {
            textarea.set_cursor_style(Style::default().bg(Color::Red));
            textarea.set_cursor_line_style(
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            );
        } else {
            textarea.set_cursor_line_style(Style::default());
            textarea.set_cursor_style(Style::default());
        }
        f.render_widget(&*textarea, *area);
    }

    fn render_empty_joblist(&self, f: &mut Frame, area: &Rect) {
        let text = "No jobs found";
        let text = Span::styled(text, Style::default().fg(Color::Gray));
        let paragraph = Paragraph::new(text).alignment(Alignment::Center);
        f.render_widget(paragraph, *area);
    }

    // ----------------------------------------------------------------------
    // RENDERING THE JOB DETAILS AND LOG SECTION
    // ----------------------------------------------------------------------
    fn render_bottom_section(&mut self, f: &mut Frame, area: &Rect, jobs: &JobList) {
        self.log_height = area.height.saturating_sub(2);
        match self.collapsed_bot {
            true => self.render_bottom_collapsed(f, area),
            false => self.render_bottom_extended(f, area, jobs),
        }
    }

    fn render_bottom_collapsed(&mut self, f: &mut Frame, area: &Rect) {
        let title = vec![
            Span::raw("▶ "),
            Span::raw("1. Job details"),
            Span::raw("  "),
            Span::raw("2. Log"),
        ];

        // update the mouse areas
        self.update_bottom_mouse_positions(area, title.clone(), 0);

        let line = Line::from(title).style(Style::default().fg(Color::Gray));
        f.render_widget(line, *area);
    }

    fn render_bottom_extended(&mut self, f: &mut Frame, area: &Rect, jobs: &JobList) {
        let mut title = vec![
            Span::raw("▼ "),
            Span::raw("1. Job details"),
            Span::raw("  "),
            Span::raw("2. Log"),
        ];

        // update the mouse areas
        self.update_bottom_mouse_positions(area, title.clone(), 1);

        match self.focus {
            WindowFocus::JobDetails => {
                title[1] = Span::styled("1. Job details", Style::default().fg(Color::Blue));
            }
            WindowFocus::Log => {
                title[3] = Span::styled("2. Log", Style::default().fg(Color::Blue));
            }
        }

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        f.render_widget(block.clone(), *area);
        let rect = block.inner(*area);
        match self.focus {
            WindowFocus::JobDetails => {
                self.render_job_details(f, &rect, jobs);
            }
            WindowFocus::Log => {
                self.render_log(f, &rect, jobs);
            }
        }
    }

    fn render_job_details(&self, f: &mut Frame, area: &Rect, jobs: &JobList) {
        // insight lines about the selected job (pending reason or
        // seff-style efficiency stats) are shown above the scontrol text
        let mut lines: Vec<Line> = Vec::new();
        if let Some(job) = jobs.get_job() {
            append_pending_reason_lines(&mut lines, job);
            append_efficiency_lines(&mut lines, job);
        }
        for line in jobs.get_job_details().lines() {
            lines.push(Line::from(line.to_string()));
        }

        let paragraph = Paragraph::new(Text::from(lines))
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        f.render_widget(paragraph, *area);
    }

    fn render_log(&self, f: &mut Frame, area: &Rect, jobs: &JobList) {
        let mut paragraph = Paragraph::new(jobs.get_log_tail())
            .alignment(Alignment::Left)
            .wrap(Wrap { trim: true });

        // calculate the scroll offset such that the last line is visible
        let lines = paragraph.line_count(area.width);
        let offset = (lines as u16).saturating_sub(self.log_height);
        paragraph = paragraph.scroll((offset, 0));

        f.render_widget(paragraph, *area);
    }

    fn update_bottom_mouse_positions(&mut self, area: &Rect, title: Vec<Span>, offset: u16) {
        if title.len() != 4 {
            return;
        }
        let mut top_row = *area;
        top_row.height = 1;
        let mut symbol = top_row;
        symbol.width = title[0].width() as u16;
        symbol.x += offset;
        let mut details_title = top_row;
        details_title.width = title[1].width() as u16;
        details_title.x += symbol.width + symbol.x;
        let mut log_title = top_row;
        log_title.width = title[3].width() as u16;
        log_title.x += details_title.width + details_title.x + 2;
        // clip the manually constructed rects to the containing area so the
        // mouse areas never extend beyond the rendered region
        self.mouse_areas.bottom_symbol = symbol.intersection(*area);
        self.mouse_areas.details_title = details_title.intersection(*area);
        self.mouse_areas.log_title = log_title.intersection(*area);
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

/// The color of an array-group header row: the "most active" status of
/// its tasks wins (running > pending/completing > failed > completed).
fn get_group_color(tasks: &[&Job]) -> Color {
    if tasks.iter().any(|job| job.status == JobStatus::Running) {
        Color::Green
    } else if tasks
        .iter()
        .any(|job| matches!(job.status, JobStatus::Pending | JobStatus::Completing))
    {
        Color::Yellow
    } else if tasks.iter().any(|job| {
        matches!(
            job.status,
            JobStatus::Failed | JobStatus::Timeout | JobStatus::Cancelled | JobStatus::Unknown
        )
    }) {
        Color::Red
    } else {
        Color::Gray
    }
}

// ====================================================================
//  JOB-DETAILS INSIGHT LINES (pending reason, efficiency stats)
// ====================================================================

/// For a pending job, prepends a one-line explanation of why it is
/// still waiting (from the squeue "Reason" field).
fn append_pending_reason_lines(lines: &mut Vec<Line>, job: &Job) {
    if job.status != JobStatus::Pending {
        return;
    }
    let raw = job.reason.as_deref().unwrap_or("None");
    let code = reason_code(raw);
    let text = match explain_reason(code) {
        // "None"/empty carry no information, so no code is shown
        Some(explanation) if code.is_empty() || code == "None" => {
            format!("⏳ Pending — {}", explanation)
        }
        Some(explanation) => format!("⏳ Pending — {}: {}", code, explanation),
        // unknown codes: show the raw reason reported by squeue
        None => format!("⏳ Pending — {}", raw),
    };
    lines.push(Line::from(Span::styled(
        text,
        Style::default().fg(Color::Yellow).bold(),
    )));
    lines.push(Line::default());
}

/// The width (in cells) of the efficiency bars.
const EFFICIENCY_BAR_WIDTH: usize = 20;

/// For a started job with fetched stats, prepends seff-style
/// efficiency bars (CPU, memory, share of the time limit used).
fn append_efficiency_lines(lines: &mut Vec<Line>, job: &Job) {
    let stats = match &job.stats {
        Some(stats) if stats.has_any() => stats,
        // degrade gracefully: no stats, no extra lines
        _ => return,
    };
    lines.push(Line::from(Span::styled(
        "Efficiency  (CPU/Mem: red = underused · Time: red = near limit)",
        Style::default().fg(Color::Gray),
    )));
    if let Some(cpu) = stats.cpu_efficiency {
        lines.push(efficiency_line("CPU ", cpu, utilization_color(cpu)));
    }
    if let Some(mem) = stats.mem_efficiency {
        lines.push(efficiency_line("Mem ", mem, utilization_color(mem)));
    }
    if let Some(time) = stats.elapsed_frac_of_limit {
        lines.push(efficiency_line("Time", time, time_limit_color(time)));
    }
    lines.push(Line::default());
}

/// Builds one efficiency line, e.g. "CPU   85% ████████████████▌░░░".
fn efficiency_line(label: &str, fraction: f64, color: Color) -> Line<'static> {
    Line::from(vec![
        Span::raw(format!("{} ", label)),
        Span::styled(
            format!(
                "{:>4.0}% {}",
                fraction * 100.0,
                bar_string(fraction, EFFICIENCY_BAR_WIDTH)
            ),
            Style::default().fg(color),
        ),
    ])
}

/// Renders a fraction (clamped to 0..=1) as a bar of block characters
/// with eighth-block resolution, padded to `width` cells.
fn bar_string(fraction: f64, width: usize) -> String {
    const PARTIAL_BLOCKS: [char; 8] = [' ', '▏', '▎', '▍', '▌', '▋', '▊', '▉'];
    let clamped = fraction.clamp(0.0, 1.0);
    let eighths = (clamped * (width * 8) as f64).round() as usize;
    let mut bar = "█".repeat(eighths / 8);
    if !eighths.is_multiple_of(8) {
        bar.push(PARTIAL_BLOCKS[eighths % 8]);
    }
    let filled = bar.chars().count();
    bar.push_str(&"░".repeat(width.saturating_sub(filled)));
    bar
}

/// Color for CPU/memory utilization: *low* utilization is the warning
/// (allocated resources sit idle).
fn utilization_color(fraction: f64) -> Color {
    if fraction < 0.3 {
        Color::Red
    } else if fraction < 0.6 {
        Color::Yellow
    } else {
        Color::Green
    }
}

/// Color for the used share of the time limit: *high* usage is the
/// warning (the job is close to being killed by the limit).
fn time_limit_color(fraction: f64) -> Color {
    if fraction < 0.7 {
        Color::Green
    } else if fraction <= 0.9 {
        Color::Yellow
    } else {
        Color::Red
    }
}

// ====================================================================
//  USER INPUT
// ====================================================================

impl JobOverview {
    /// Handle user input for the job overview window
    /// Returns true if the input was handled
    /// Returns false if the input was not handled
    pub fn input(&mut self, action: &mut Action, key_event: KeyEvent) -> bool {
        if self.edit_squeue {
            match key_event.code {
                KeyCode::Esc | KeyCode::Enter => {
                    self.exit_squeue_edit(action);
                    return true;
                }
                _ => {
                    self.squeue_command.input(key_event);
                    return true;
                }
            }
        }

        match key_event.code {
            // Escaping the program
            KeyCode::Char('q') => {
                *action = Action::Quit;
            }
            // Next / Previous job
            KeyCode::Down | KeyCode::Char('j') => {
                self.next_job(action);
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.prev_job(action);
            }
            // Open job action menu
            KeyCode::Enter | KeyCode::Char('l') => {
                *action = Action::OpenMenu(OpenMenu::JobActions);
            }
            // Expand/collapse the selected job-array group
            KeyCode::Char(' ') => {
                *action = Action::UpdateJobList(JobListAction::ToggleGroup);
            }
            // Change sorting category
            KeyCode::Tab => {
                *action = Action::UpdateJobList(JobListAction::NextSortCategory);
            }
            KeyCode::Char('r') => {
                *action = Action::UpdateJobList(JobListAction::ReverseSortDirection);
            }
            // Switching focus between job details and log
            KeyCode::Char('1') => {
                self.select_details();
            }
            KeyCode::Char('2') => {
                self.select_log();
            }
            KeyCode::Right | KeyCode::Left => {
                self.toggle_focus();
            }
            // Open job allocation menu
            KeyCode::Char('a') => {
                *action = Action::OpenMenu(OpenMenu::Salloc);
            }
            KeyCode::Char('o') => {
                *action = Action::OpenMenu(OpenMenu::UserOptions);
            }
            KeyCode::Char('?') => {
                *action = Action::OpenMenu(OpenMenu::Help(HelpContext::JobOverview));
            }
            // Collapsing/Extending the joblist
            KeyCode::Char('m') => {
                self.collapsed_top = !self.collapsed_top;
            }
            KeyCode::Char('n') => {
                self.collapsed_bot = !self.collapsed_bot;
            }
            // Edit the squeue command
            KeyCode::Char('/') => {
                self.collapsed_top = false;
                self.edit_squeue = true;
            }
            _ => {
                return false;
            }
        };
        true
    }

    fn select_details(&mut self) {
        // if the job details are already in focus, toggle collapse
        if self.focus == WindowFocus::JobDetails {
            self.collapsed_bot = !self.collapsed_bot;
        } else {
            self.focus = WindowFocus::JobDetails;
            self.collapsed_bot = false;
        }
    }

    fn select_log(&mut self) {
        // if the log is already in focus, toggle collapse
        if self.focus == WindowFocus::Log {
            self.collapsed_bot = !self.collapsed_bot;
        } else {
            self.focus = WindowFocus::Log;
            self.collapsed_bot = false;
        }
    }

    /// Toggle the focus between the job details and the log section
    /// (there are only two sections, so next and previous coincide)
    fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            WindowFocus::JobDetails => WindowFocus::Log,
            WindowFocus::Log => WindowFocus::JobDetails,
        };
    }

    fn next_job(&mut self, action: &mut Action) {
        *action = Action::UpdateJobList(JobListAction::Next);
    }

    fn prev_job(&mut self, action: &mut Action) {
        *action = Action::UpdateJobList(JobListAction::Previous);
    }

    pub fn set_index(&mut self, index: i32) {
        self.state.select(Some(index as usize));
    }
}

// ====================================================================
//  MOUSE INPUT
// ====================================================================

impl JobOverview {
    pub fn mouse_input(&mut self, action: &mut Action, mouse_input: &mut MouseInput) {
        let mouse_pos = mouse_input.get_position();

        if let Some(event_kind) = mouse_input.kind() {
            match event_kind {
                MouseEventKind::Down(MouseButton::Left) => {
                    // if the squeue command is being edited, go back
                    // to normal mode
                    if self.edit_squeue {
                        self.exit_squeue_edit(action);
                        return;
                    }
                    // joblist title
                    if self.mouse_areas.joblist_title.contains(mouse_pos) {
                        self.collapsed_top = !self.collapsed_top;
                        mouse_input.click();
                    }
                    // squeue Command
                    if self.mouse_areas.squeue_command.contains(mouse_pos) {
                        self.edit_squeue = true;
                        mouse_input.click();
                    }
                    // joblist categories (one mouse area per column)
                    for (i, category) in self.mouse_areas.categories.iter().enumerate() {
                        if category.contains(mouse_pos) {
                            if let Some(new_cat) = self.columns.get(i).copied() {
                                *action = Action::UpdateJobList(JobListAction::SelectSortCategory(
                                    new_cat,
                                ));
                                mouse_input.click();
                            }
                        }
                    }
                    // joblist entries
                    if self.mouse_areas.joblist.contains(mouse_pos) {
                        let rel_y = mouse_pos.y - self.mouse_areas.joblist.y;
                        let new_index = rel_y as usize + self.state.offset();
                        *action = Action::UpdateJobList(JobListAction::Select(new_index));
                        if mouse_input.is_double_click() {
                            *action = Action::OpenMenu(OpenMenu::JobActions);
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
                        self.select_details();
                        mouse_input.click();
                    }
                    // log title
                    if self.mouse_areas.log_title.contains(mouse_pos) {
                        self.select_log();
                        mouse_input.click();
                    }
                }
                MouseEventKind::ScrollDown => {
                    self.next_job(action);
                }
                MouseEventKind::ScrollUp => {
                    self.prev_job(action);
                }
                _ => {}
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
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn make_running_job() -> Job {
        Job::new(
            "424242",
            "train_model",
            JobStatus::Running,
            "12:34:56",
            "gpu",
            2,
            "/home/user/project",
            "/home/user/project/run.sh",
            None,
        )
    }

    /// Render the job overview into a test terminal of the given size and
    /// return the resulting buffer content as a single string.
    fn render_to_string(width: u16, height: u16, jobs: &JobList) -> String {
        render_overview(width, height, jobs, false)
    }

    /// Like [`render_to_string`], with an expanded job-details pane.
    fn render_with_details(width: u16, height: u16, jobs: &JobList) -> String {
        render_overview(width, height, jobs, true)
    }

    fn render_overview(width: u16, height: u16, jobs: &JobList, expand_details: bool) -> String {
        render_overview_with_columns(width, height, jobs, expand_details, JobColumn::defaults())
    }

    fn render_overview_with_columns(
        width: u16,
        height: u16,
        jobs: &JobList,
        expand_details: bool,
        columns: Vec<JobColumn>,
    ) -> String {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();
        let mut overview = JobOverview::new(250, "squeue -u user", columns);
        overview.collapsed_bot = !expand_details;
        terminal
            .draw(|f| {
                let area = f.area();
                overview.render(f, &area, jobs);
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect()
    }

    #[test]
    fn test_render_job_overview_with_running_job() {
        let mut jobs = JobList::new();
        jobs.jobs.push(make_running_job());
        jobs.set_index(0).unwrap();

        let content = render_to_string(80, 20, &jobs);
        assert!(content.contains("SLURM TASK MANAGER"));
        assert!(content.contains("424242"));
        assert!(content.contains("train_model"));
        assert!(content.contains("Running"));
    }

    /// Regression test: with the default configuration the job table
    /// shows exactly the historical six column headers.
    #[test]
    fn test_render_default_columns_shows_historical_headers() {
        let mut jobs = JobList::new();
        jobs.jobs.push(make_running_job());
        jobs.set_index(0).unwrap();

        let content = render_to_string(100, 20, &jobs);
        for header in ["ID", "Name", "Status", "Time", "Partition", "Nodes"] {
            assert!(content.contains(header), "missing header {:?}", header);
        }
        // headers of non-default columns are not shown
        assert!(!content.contains("Priority"));
        assert!(!content.contains("Account"));
    }

    #[test]
    fn test_render_custom_columns_with_priority() {
        let mut job = make_running_job();
        job.priority = 4294901760;
        let mut jobs = JobList::new();
        jobs.jobs.push(job);
        jobs.set_index(0).unwrap();

        // the user replaced the Nodes column with Priority
        let columns = vec![
            JobColumn::Id,
            JobColumn::Name,
            JobColumn::Status,
            JobColumn::Time,
            JobColumn::Partition,
            JobColumn::Priority,
        ];
        let content = render_overview_with_columns(100, 20, &jobs, false, columns);

        // the Priority header and value are shown ...
        assert!(content.contains("Priority"));
        assert!(content.contains("4294901760"));
        // ... and the Nodes column is gone
        assert!(!content.contains("Nodes"));
    }

    // ----------------------------------------------------------------
    // job-array group rows
    // ----------------------------------------------------------------

    /// A job list with two tasks of the array 12345 and one single job.
    fn make_array_joblist() -> JobList {
        let mut jobs = JobList::new();
        let mut task1 = make_running_job();
        task1.id = "12345_1".to_string();
        let mut task2 = make_running_job();
        task2.id = "12345_2".to_string();
        task2.status = JobStatus::Pending;
        jobs.jobs.push(task1);
        jobs.jobs.push(task2);
        jobs.jobs.push(make_running_job());
        jobs.set_index(0).unwrap();
        jobs
    }

    #[test]
    fn test_render_collapsed_group_row_shows_base_id_and_counts() {
        let jobs = make_array_joblist();

        let content = render_to_string(100, 20, &jobs);
        // the group row shows the collapsed marker, "base[]" and the
        // aggregate status counts
        assert!(content.contains("▶ 12345[]"));
        assert!(content.contains("1R 1PD"));
        // the task ids are hidden while the group is collapsed
        assert!(!content.contains("12345_1"));
        assert!(!content.contains("12345_2"));
        // the single job is unaffected
        assert!(content.contains("424242"));
    }

    #[test]
    fn test_render_expanded_group_shows_indented_tasks() {
        let mut jobs = make_array_joblist();
        // expand the group under the cursor (row 0)
        jobs.handle_joblist_action(JobListAction::ToggleGroup, &JobColumn::defaults());

        let content = render_to_string(100, 20, &jobs);
        assert!(content.contains("▼ 12345[]"));
        // the tasks are rendered indented below the header, the last
        // one with the closing tree glyph
        assert!(content.contains("├ 12345_1"));
        assert!(content.contains("└ 12345_2"));
    }

    /// Regression test: rendering into a terminal narrower than the
    /// manually constructed squeue command rect must not panic.
    #[test]
    fn test_render_narrow_terminal_does_not_panic() {
        let mut jobs = JobList::new();
        jobs.jobs.push(make_running_job());
        jobs.set_index(0).unwrap();

        render_to_string(10, 5, &jobs);
    }

    #[test]
    fn test_render_empty_joblist() {
        let mut jobs = JobList::new();
        jobs.set_index(0).unwrap();

        let content = render_to_string(80, 20, &jobs);
        assert!(content.contains("SLURM TASK MANAGER"));
        assert!(content.contains("No jobs found"));
    }

    // ----------------------------------------------------------------
    // details pane: pending reason and efficiency stats
    // ----------------------------------------------------------------

    #[test]
    fn test_render_pending_job_shows_reason_explanation() {
        let mut job = make_running_job();
        job.status = JobStatus::Pending;
        job.reason = Some("Priority".to_string());
        let mut jobs = JobList::new();
        jobs.jobs.push(job);
        jobs.set_index(0).unwrap();

        let content = render_with_details(100, 24, &jobs);
        assert!(content.contains("Pending — Priority:"));
        assert!(content.contains("higher-priority jobs are ahead in the queue"));
    }

    #[test]
    fn test_render_pending_job_with_unknown_reason_shows_raw_code() {
        let mut job = make_running_job();
        job.status = JobStatus::Pending;
        job.reason = Some("SomeNewSlurmReason".to_string());
        let mut jobs = JobList::new();
        jobs.jobs.push(job);
        jobs.set_index(0).unwrap();

        let content = render_with_details(100, 24, &jobs);
        assert!(content.contains("Pending — SomeNewSlurmReason"));
    }

    #[test]
    fn test_render_running_job_with_stats_shows_gauges() {
        let mut job = make_running_job();
        job.stats = Some(Box::new(crate::job::JobStats {
            cpu_efficiency: Some(0.85),
            mem_efficiency: Some(0.42),
            elapsed_frac_of_limit: Some(0.61),
        }));
        let mut jobs = JobList::new();
        jobs.jobs.push(job);
        jobs.set_index(0).unwrap();

        let content = render_with_details(100, 24, &jobs);
        assert!(content.contains("Efficiency"));
        assert!(content.contains("CPU"));
        assert!(content.contains("85%"));
        assert!(content.contains("Mem"));
        assert!(content.contains("42%"));
        assert!(content.contains("Time"));
        assert!(content.contains("61%"));
        // the bars are drawn with block characters
        assert!(content.contains("████"));
    }

    #[test]
    fn test_render_running_job_without_stats_shows_no_gauges() {
        let mut jobs = JobList::new();
        jobs.jobs.push(make_running_job());
        jobs.set_index(0).unwrap();

        let content = render_with_details(100, 24, &jobs);
        // no stats fetched: the efficiency block is omitted entirely
        assert!(!content.contains("Efficiency"));
        // a running job never shows the pending line
        assert!(!content.contains("Pending —"));
    }

    // ----------------------------------------------------------------
    // bar rendering helpers
    // ----------------------------------------------------------------

    #[test]
    fn test_bar_string_widths_and_clamping() {
        // empty and full bars are exactly `width` cells
        assert_eq!(bar_string(0.0, 4), "░░░░");
        assert_eq!(bar_string(1.0, 4), "████");
        // values beyond the range are clamped
        assert_eq!(bar_string(-0.5, 4), "░░░░");
        assert_eq!(bar_string(2.5, 4), "████");
        // a half-filled bar uses the half block
        assert_eq!(bar_string(0.5, 4), "██░░");
        assert_eq!(bar_string(0.625, 4), "██▌░");
        // every bar is padded to the requested width
        for i in 0..=10 {
            let bar = bar_string(i as f64 / 10.0, 7);
            assert_eq!(bar.chars().count(), 7, "wrong width for {}", i);
        }
    }

    #[test]
    fn test_efficiency_colors() {
        // CPU/memory: low utilization is the warning
        assert_eq!(utilization_color(0.1), Color::Red);
        assert_eq!(utilization_color(0.45), Color::Yellow);
        assert_eq!(utilization_color(0.9), Color::Green);
        // time limit: high usage is the warning
        assert_eq!(time_limit_color(0.5), Color::Green);
        assert_eq!(time_limit_color(0.8), Color::Yellow);
        assert_eq!(time_limit_color(0.95), Color::Red);
    }
}
