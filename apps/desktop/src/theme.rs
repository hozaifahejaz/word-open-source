//! Shared visual treatment for the ribbon, paper workspace and status panels.
use egui::{Color32, Context, FontId, Stroke, TextStyle, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(64, 100, 174);
pub const INK: Color32 = Color32::from_rgb(38, 42, 51);
pub const MUTED: Color32 = Color32::from_rgb(107, 113, 127);
pub const BORDER: Color32 = Color32::from_rgb(228, 230, 236);
pub const WORKSPACE: Color32 = Color32::from_rgb(245, 245, 247);
pub const TEXT_SELECTION: Color32 = Color32::from_rgb(220, 224, 238);
pub const WARNING: Color32 = Color32::from_rgb(135, 79, 15);

pub fn install(ctx: &Context) {
    install_mode(ctx, false);
}

pub fn install_mode(ctx: &Context, dark: bool) {
    // Select the requested style before customizing it. Otherwise initial OS
    // theme detection can switch to the untouched style after construction.
    ctx.set_theme(if dark {
        egui::Theme::Dark
    } else {
        egui::Theme::Light
    });
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(6.0, 8.0);
    style.spacing.button_padding = Vec2::new(9.0, 7.0);
    style.spacing.interact_size.y = 32.0;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    style.visuals = if dark {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };
    style.visuals.override_text_color = Some(if dark {
        Color32::from_rgb(235, 237, 244)
    } else {
        INK
    });
    style.visuals.panel_fill = surface(dark);
    style.visuals.window_fill = surface(dark);
    style.visuals.extreme_bg_color = if dark {
        Color32::from_rgb(20, 21, 27)
    } else {
        Color32::from_rgb(249, 249, 251)
    };
    style.visuals.faint_bg_color = workspace(dark);
    style.visuals.selection.bg_fill = if dark {
        Color32::from_rgb(66, 75, 116)
    } else {
        Color32::from_rgb(234, 237, 247)
    };
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, border(dark));
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, muted(dark));
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = 8.into();
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(
            1.0,
            if dark {
                Color32::from_rgb(235, 237, 244)
            } else {
                INK
            },
        );
    }
    style.visuals.widgets.inactive.bg_fill = workspace(dark);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.hovered.bg_fill = if dark {
        Color32::from_rgb(53, 57, 72)
    } else {
        Color32::from_rgb(238, 240, 246)
    };
    style.visuals.widgets.hovered.weak_bg_fill = style.visuals.widgets.hovered.bg_fill;
    style.visuals.widgets.active.bg_fill = if dark {
        Color32::from_rgb(66, 75, 116)
    } else {
        Color32::from_rgb(228, 232, 244)
    };
    style.visuals.widgets.active.weak_bg_fill = style.visuals.widgets.active.bg_fill;
    ctx.set_style(style);
}

pub fn surface(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(31, 33, 42)
    } else {
        Color32::WHITE
    }
}

pub fn workspace(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(20, 21, 27)
    } else {
        WORKSPACE
    }
}

pub fn border(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(69, 73, 88)
    } else {
        BORDER
    }
}

pub fn muted(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(168, 173, 190)
    } else {
        MUTED
    }
}

pub fn text(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(235, 237, 244)
    } else {
        INK
    }
}

/// Quiet section colors keep the compact ribbon legible in both themes.
pub fn font_group(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(40, 38, 54)
    } else {
        Color32::from_rgb(246, 243, 251)
    }
}
pub fn paragraph_group(dark: bool) -> Color32 {
    if dark {
        Color32::from_rgb(29, 46, 48)
    } else {
        Color32::from_rgb(237, 248, 247)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_theme_survives_initial_system_theme_detection() {
        let ctx = Context::default();
        let _ = ctx.run(
            egui::RawInput {
                system_theme: Some(egui::Theme::Dark),
                ..Default::default()
            },
            |_| {},
        );
        install(&ctx);
        let installed = ctx.style().visuals.selection;
        let _ = ctx.run(
            egui::RawInput {
                system_theme: Some(egui::Theme::Light),
                ..Default::default()
            },
            |_| {},
        );
        assert_eq!(ctx.style().visuals.selection, installed);
    }
}
