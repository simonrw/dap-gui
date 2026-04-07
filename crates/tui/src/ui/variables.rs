use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

use crate::app::App;
use crate::session::DebuggerState;

/// Render the variables panel showing local variables when paused,
/// with cursor navigation, expand/collapse, and yank support.
pub fn render(app: &App, frame: &mut Frame, area: Rect) {
    let border = super::border_style(app, super::Focus::Variables);
    let title = " Variables [Alt+1] [y:yank] ";
    let is_focused = app.focus == super::Focus::Variables;
    let theme = &app.theme;

    let items: Vec<ListItem> = if let Some(session) = &app.session {
        if let DebuggerState::Paused { paused_frame, .. } = &session.state {
            let mut flat_items: Vec<ListItem> = Vec::new();
            let mut flat_idx = 0usize;

            for var in &paused_frame.variables {
                flat_items.push(make_variable_item(
                    var,
                    0,
                    is_focused && flat_idx == app.variables_cursor,
                    &app.variables_cache,
                    theme,
                ));
                flat_idx += 1;

                // If expanded, add children
                if let Some(ref_id) = var.variables_reference
                    && ref_id > 0
                    && let Some(children) = app.variables_cache.get(&ref_id)
                {
                    for child in children {
                        flat_items.push(make_variable_item(
                            child,
                            1,
                            is_focused && flat_idx == app.variables_cursor,
                            &app.variables_cache,
                            theme,
                        ));
                        flat_idx += 1;
                    }
                }
            }
            flat_items
        } else {
            vec![ListItem::new(Span::styled(
                "  (not paused)",
                Style::default().fg(theme.text_muted),
            ))]
        }
    } else {
        vec![ListItem::new(Span::styled(
            "  (no session)",
            Style::default().fg(theme.text_muted),
        ))]
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border)
            .title(title),
    );
    frame.render_widget(list, area);
}

fn make_variable_item<'a>(
    var: &dap_types::Variable,
    indent: usize,
    is_selected: bool,
    cache: &std::collections::HashMap<i64, Vec<dap_types::Variable>>,
    theme: &crate::theme::Theme,
) -> ListItem<'a> {
    let indent_str = "  ".repeat(indent + 1);

    let expandable = var.variables_reference.is_some_and(|r| r > 0);
    let expanded = expandable
        && var
            .variables_reference
            .is_some_and(|r| cache.contains_key(&r));

    let tree_marker = if expandable {
        if expanded {
            "\u{25bc} " // ▼
        } else {
            "\u{25b6} " // ▶
        }
    } else {
        "  "
    };

    let name_span = Span::styled(var.name.clone(), Style::default().fg(theme.accent_alt));
    let type_span = if let Some(ref ty) = var.r#type {
        Span::styled(format!(": {ty}"), Style::default().fg(theme.text_muted))
    } else {
        Span::raw("")
    };
    let value_span = if let Some(ref val) = var.value {
        Span::styled(format!(" = {val}"), Style::default().fg(theme.text))
    } else {
        Span::raw("")
    };

    let mut line = Line::from(vec![
        Span::raw(indent_str),
        Span::styled(tree_marker, Style::default().fg(theme.accent)),
        name_span,
        type_span,
        value_span,
    ]);

    if is_selected {
        line = line.patch_style(Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED));
    }

    ListItem::new(line)
}
