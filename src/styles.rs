use cosmic::{
    iced::{Background, Color},
    widget,
};

/// Button style for running window items.
///
/// All running-app buttons show the Windows 11-style pill: a visible rounded
/// border + subtle fill at rest.  `is_focused` controls how prominent the fill
/// is (active window = brighter pill).  Hovering always brightens to the full
/// hover color.
///
/// Use `win11_pinned_style()` for pinned-but-not-running items, which should
/// have no pill at rest.
pub fn win11_button_style(is_focused: bool) -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(move |_focused, theme| {
            let cosmic = theme.cosmic();
            let (background, border_width, border_color) = if is_focused {
                let mut bg = Color::from(cosmic.background.component.hover);
                bg.a *= 0.65;
                let mut bc = Color::from(cosmic.on_bg_color());
                bc.a = 0.20;
                (Some(Background::Color(bg)), 1.0, bc)
            } else {
                (None, 0.0, Color::TRANSPARENT)
            };
            widget::button::Style {
                background,
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 6.0.into(),
                border_width,
                border_color,
                ..Default::default()
            }
        }),
        hovered: Box::new(move |_focused, theme| {
            let cosmic = theme.cosmic();
            let mut border_color = Color::from(cosmic.on_bg_color());
            border_color.a = 0.22;
            widget::button::Style {
                background: Some(Background::Color(Color::from(cosmic.background.component.hover))),
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 6.0.into(),
                border_width: 1.0,
                border_color,
                ..Default::default()
            }
        }),
        disabled: Box::new(|_theme| Default::default()),
        pressed: Box::new(move |_focused, theme| {
            let cosmic = theme.cosmic();
            let mut border_color = Color::from(cosmic.on_bg_color());
            border_color.a = 0.22;
            widget::button::Style {
                background: Some(Background::Color(Color::from(cosmic.background.component.pressed))),
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 6.0.into(),
                border_width: 1.0,
                border_color,
                ..Default::default()
            }
        }),
    }
}

/// Button style for pinned-but-not-running items: no pill at rest, standard
/// hover highlight on hover.
pub fn win11_pinned_style() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(|_focused, theme| {
            let cosmic = theme.cosmic();
            widget::button::Style {
                background: None,
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 6.0.into(),
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
                ..Default::default()
            }
        }),
        hovered: Box::new(|_focused, theme| {
            let cosmic = theme.cosmic();
            let mut border_color = Color::from(cosmic.on_bg_color());
            border_color.a = 0.22;
            widget::button::Style {
                background: Some(Background::Color(Color::from(cosmic.background.component.hover))),
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 6.0.into(),
                border_width: 1.0,
                border_color,
                ..Default::default()
            }
        }),
        disabled: Box::new(|_theme| Default::default()),
        pressed: Box::new(|_focused, theme| {
            let cosmic = theme.cosmic();
            widget::button::Style {
                background: Some(Background::Color(Color::from(cosmic.background.component.pressed))),
                text_color: Some(Color::from(cosmic.on_bg_color())),
                border_radius: 6.0.into(),
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
