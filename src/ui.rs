use ratatui::{
    prelude::{Alignment, Frame, Layout, Direction, Constraint},
    style::{Color, Style},
    widgets::{Paragraph},
};
use crate::app::App;

pub fn render(app: &mut App, f: &mut Frame) {

    let outer_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints(
            [
                Constraint::Min(3),
                Constraint::Length(1),
            ]
            .as_ref(),
        )
        .split(f.size());

    // make a info text at the bottom
    f.render_widget(
        Paragraph::new("Press `Ctrl-C` or `q` for exit, `?` for help")
            .style(Style::default().fg(Color::LightCyan))
            .alignment(Alignment::Center),
        outer_layout[1],
    );

    // render the windows
    app.job_overview.render(f, &outer_layout[0]);
    app.job_actions_menu.render(f, &outer_layout[0]);
    app.user_options.render(f, &outer_layout[0]);
    app.message.render(f, &outer_layout[0]);

}


