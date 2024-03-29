use ratatui::{
    prelude::*,
    style::{Color, Style},
    widgets::*,
};

pub enum WindowFocus {
    JobDetails,
    Log,
}

pub struct JobOverview {
    pub is_active: bool,      // if the window is active 
    pub search_args: String,  // the search arguments for squeue
    pub focus: WindowFocus,   // which part of the window is in focus
}

// ====================================================================
//  CONSTRUCTOR
// ====================================================================

impl JobOverview {
    pub fn new() -> Self {
        Self {
            is_active: true,
            search_args: "-U u301533".to_string(),
            focus: WindowFocus::JobDetails,
        }
    }
}


// ====================================================================
//  RENDERING
// ====================================================================

impl JobOverview {
    pub fn render(&self, f: &mut Frame, area: &Rect) {
        // only render if the window is active
        if !self.is_active { return; }

        // create a layout for the title
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(
                [
                    Constraint::Min(1),
                    Constraint::Percentage(30),
                    Constraint::Percentage(70),
                ]
                .as_ref(),
            )
            .split(*area);

        // render the title, job list, and job details
        self.render_title(f, &layout[0]);
        self.render_joblist(f, &layout[1]);
        self.render_details(f, &layout[2]);
    }

    fn render_title(&self, f: &mut Frame, area: &Rect) {
        f.render_widget(
            Paragraph::new("JOB OVERVIEW")
                .style(Style::default().fg(Color::Red))
                .alignment(Alignment::Center),
            *area,
        );
    }

    fn render_joblist(&self, f: &mut Frame, area: &Rect) {
        // title should be "Job list: squeue {search_args}}"
        let title = format!("Job list: squeue {}", self.search_args);
        
        let block = Block::default().title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        f.render_widget(block.clone(), *area);
        let _ = block.inner(*area);
    }

    fn render_details(&self, f: &mut Frame, area: &Rect) {
        let mut title = vec!{
            Span::raw("1. Job details"), 
            Span::raw("  "),
            Span::raw("2. Log"),
        };

        match self.focus {
            WindowFocus::JobDetails => {
                title[0] = Span::styled("1. Job details", 
                                        Style::default().fg(Color::Blue));
            },
            WindowFocus::Log => {
                title[2] = Span::styled("2. Log", 
                                        Style::default().fg(Color::Blue));
            },
        }
        
        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded);

        f.render_widget(block.clone(), *area);
        let _ = block.inner(*area);
    }
}


// ====================================================================
//  USER INPUT
// ====================================================================
