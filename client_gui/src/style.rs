use iced::{button, container, Background, Color, Vector};

pub enum Button {
    Simple,
}

impl button::StyleSheet for Button {
    fn active(&self) -> button::Style {
        match self {
            Button::Simple => button::Style {
                background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.7))),
                border_radius: 10.0,
                text_color: Color::WHITE,
                ..button::Style::default()
            },
        }
    }

    fn hovered(&self) -> button::Style {
        let active = self.active();

        button::Style {
            shadow_offset: active.shadow_offset + Vector::new(2.0, 2.0),
            ..active
        }
    }
}

pub enum Container {
    SystemMessage,
    UserMessage,
}

impl container::StyleSheet for Container {
    fn style(&self) -> container::Style {
        let color = match self {
            Container::SystemMessage => Color::from_rgb8(199, 243, 239),
            Container::UserMessage => Color::from_rgb8(220, 220, 220),
        };

        container::Style {
            background: Some(Background::Color(color)),
            border_radius: 10.0,
            ..container::Style::default()
        }
    }
}