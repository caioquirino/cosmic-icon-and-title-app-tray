use cosmic::{
    iced::{Background, Color},
    widget,
};

pub fn win11_button_style() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(|_focused, theme| {
            let cosmic = theme.cosmic();
            widget::button::Style {
                background: None,
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 4.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                ..Default::default()
            }
        }),
        hovered: Box::new(|_focused, theme| {
            let cosmic = theme.cosmic();
            widget::button::Style {
                background: Some(Background::Color(Color::from(cosmic.background.component.hover))),
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 4.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                ..Default::default()
            }
        }),
        disabled: Box::new(|_theme| Default::default()),
        pressed: Box::new(|_focused, theme| {
            let cosmic = theme.cosmic();
            widget::button::Style {
                background: Some(Background::Color(Color::from(cosmic.background.component.pressed))),
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 4.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                ..Default::default()
            }
        }),
    }
}

/// Strips desktop file Exec field placeholders (%u, %U, %f, %F).
pub fn strip_exec_args(exec: &str) -> String {
    exec.replace("%u", "").replace("%U", "").replace("%f", "").replace("%F", "")
}

pub fn truncate_text(text: &str, max_len: usize) -> String {
    if text.chars().count() > max_len {
        let mut truncated: String = text.chars().take(max_len).collect();
        truncated.push_str("...");
        truncated
    } else {
        text.to_string()
    }
}
