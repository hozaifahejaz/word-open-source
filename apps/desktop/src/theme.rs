//! Shared visual treatment for the ribbon, paper workspace and status panels.
use egui::{Color32, Context, FontId, Stroke, TextStyle, Vec2};

pub const ACCENT: Color32 = Color32::from_rgb(25, 98, 91);
pub const INK: Color32 = Color32::from_rgb(37, 48, 57);
pub const MUTED: Color32 = Color32::from_rgb(94, 109, 119);
pub const BORDER: Color32 = Color32::from_rgb(218, 226, 229);
pub const WORKSPACE: Color32 = Color32::from_rgb(237, 241, 243);
pub const WARNING: Color32 = Color32::from_rgb(135, 79, 15);

pub fn install(ctx: &Context) {
    // Select the light style before customizing it. Otherwise initial OS theme
    // detection can switch to the untouched style after the app is constructed.
    ctx.set_theme(egui::Theme::Light);
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = Vec2::new(10.0, 8.0);
    style.spacing.button_padding = Vec2::new(12.0, 6.0);
    style.spacing.interact_size.y = 30.0;
    style
        .text_styles
        .insert(TextStyle::Body, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Button, FontId::proportional(14.0));
    style
        .text_styles
        .insert(TextStyle::Small, FontId::proportional(12.0));
    style.visuals = egui::Visuals::light();
    style.visuals.override_text_color = Some(INK);
    style.visuals.panel_fill = Color32::WHITE;
    style.visuals.window_fill = Color32::WHITE;
    style.visuals.extreme_bg_color = Color32::from_rgb(247, 249, 250);
    style.visuals.faint_bg_color = WORKSPACE;
    style.visuals.selection.bg_fill = Color32::from_rgb(213, 235, 231);
    style.visuals.selection.stroke = Stroke::new(1.0, ACCENT);
    for widget in [
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
    ] {
        widget.corner_radius = 6.into();
        widget.bg_stroke = Stroke::new(1.0, BORDER);
    }
    style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(246, 248, 249);
    style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(246, 248, 249);
    style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(226, 239, 236);
    style.visuals.widgets.hovered.weak_bg_fill = Color32::from_rgb(226, 239, 236);
    style.visuals.widgets.active.bg_fill = Color32::from_rgb(213, 235, 231);
    style.visuals.widgets.active.weak_bg_fill = Color32::from_rgb(213, 235, 231);
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
