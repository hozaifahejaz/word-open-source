//! Shared visual treatment for the ribbon, paper workspace and status panels.
use egui::{Color32, Context, FontId, Stroke, TextStyle, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(62, 77, 131);
pub const INK: Color32 = Color32::from_rgb(38, 42, 51);
pub const MUTED: Color32 = Color32::from_rgb(107, 113, 127);
pub const BORDER: Color32 = Color32::from_rgb(228, 230, 236);
pub const WORKSPACE: Color32 = Color32::from_rgb(245, 245, 247);
pub const TEXT_SELECTION: Color32 = Color32::from_rgb(220, 224, 238);
pub const WARNING: Color32 = Color32::from_rgb(135, 79, 15);

pub fn install(ctx: &Context) {
    // Select the light style before customizing it. Otherwise initial OS theme
    // detection can switch to the untouched style after the app is constructed.
    ctx.set_theme(egui::Theme::Light);
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
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = Color32::WHITE;
    style.visuals.window_fill = Color32::WHITE;
    style.visuals.extreme_bg_color = Color32::from_rgb(249, 249, 251);
    style.visuals.faint_bg_color = WORKSPACE;
    style.visuals.selection.bg_fill = Color32::from_rgb(234, 237, 247);
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0, BORDER);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0, MUTED);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = 8.into();
        widget.bg_stroke = Stroke::NONE;
        widget.fg_stroke = Stroke::new(1.0, INK);
    }
    style.visuals.widgets.inactive.bg_fill = WORKSPACE;
    style.visuals.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(238, 240, 246);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(238, 240, 246);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(228, 232, 244);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(228, 232, 244);
    ctx.set_style(style);
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
