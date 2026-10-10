//! Original line icons painted in logical points, with labeled native egui buttons.
use crate::editing::Action;
use egui::{
    Color32, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, Ui, Vec2, Widget, WidgetInfo,
    WidgetType,
};

#[derive(Clone, Copy)]
pub enum Icon {
    Commands,
    ReadOnly,
    Templates,
    ExportText,
    ExportSelection,
    Recent,
    New,
    Open,
    Save,
    SaveAs,
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    Bold,
    Italic,
    Underline,
    Strike,
    Superscript,
    Subscript,
    Highlight,
    ClearFormatting,
    AlignLeft,
    AlignCenter,
    AlignRight,
    AlignJustify,
    Find,
    PageBreak,
    Zoom,
    FitWidth,
    Close,
    Statistics,
    SelectAll,
    Focus,
    ExitFocus,
    Moon,
    Sun,
    Info,
    Connection,
}

impl Icon {
    pub fn for_action(action: Action) -> Self {
        match action {
            Action::New => Self::New,
            Action::Open => Self::Open,
            Action::Save => Self::Save,
            Action::SaveAs => Self::SaveAs,
            Action::Quit => Self::Close,
            Action::Undo => Self::Undo,
            Action::Redo => Self::Redo,
            Action::Bold => Self::Bold,
            Action::Italic => Self::Italic,
            Action::Underline => Self::Underline,
            Action::Strike => Self::Strike,
            Action::Superscript => Self::Superscript,
            Action::Subscript => Self::Subscript,
            Action::ClearFormatting => Self::ClearFormatting,
            Action::Cut => Self::Cut,
            Action::Copy => Self::Copy,
            Action::Paste => Self::Paste,
            Action::SelectAll => Self::SelectAll,
            Action::Find => Self::Find,
            Action::PageBreak => Self::PageBreak,
        }
    }
    pub fn paint(self, painter: &Painter, rect: Rect, color: Color32) {
        let point = |x: f32, y: f32| rect.min + Vec2::new(x, y) * (rect.width() / 24.0);
        let stroke = Stroke::new(rect.width() / 24.0 * 1.8, color);
        let line = |a: (f32, f32), b: (f32, f32)| {
            painter.line_segment([point(a.0, a.1), point(b.0, b.1)], stroke);
        };
        let path = |points: &[(f32, f32)]| {
            painter.add(Shape::line(
                points.iter().map(|&(x, y)| point(x, y)).collect(),
                stroke,
            ));
        };
        let outline = |x: f32, y: f32, w: f32, h: f32| {
            painter.rect_stroke(
                Rect::from_min_max(point(x, y), point(x + w, y + h)),
                0.5,
                stroke,
                egui::StrokeKind::Inside,
            );
        };
        let circle = |x: f32, y: f32, r: f32| {
            painter.circle_stroke(point(x, y), r * rect.width() / 24.0, stroke);
        };
        match self {
            Self::ReadOnly => {
                outline(4., 10., 16., 11.);
                path(&[
                    (8., 10.),
                    (8., 6.),
                    (10., 3.),
                    (14., 3.),
                    (16., 6.),
                    (16., 10.),
                ]);
                circle(12., 15., 1.);
                line((12., 16.), (12., 18.));
            }
            Self::Templates => {
                outline(3., 3., 8., 8.);
                outline(14., 3., 7., 8.);
                outline(3., 14., 8., 7.);
                outline(14., 14., 7., 7.);
            }
            Self::ExportText | Self::ExportSelection => {
                path(&[(14., 3.), (4., 3.), (4., 21.), (14., 21.)]);
                if matches!(self, Self::ExportSelection) {
                    outline(7., 8., 7., 8.);
                    line((9., 11.), (12., 11.));
                    line((9., 13.), (12., 13.));
                } else {
                    line((7., 8.), (13., 8.));
                    line((7., 12.), (13., 12.));
                    line((7., 16.), (11., 16.));
                }
                line((15., 12.), (22., 12.));
                path(&[(19., 9.), (22., 12.), (19., 15.)]);
            }
            Self::Commands => {
                outline(3., 4., 18., 16.);
                path(&[(7., 8.), (10., 12.), (7., 16.)]);
                line((13., 16.), (17., 16.));
            }
            Self::Recent => {
                circle(12., 12., 9.);
                line((12., 6.), (12., 12.));
                line((12., 12.), (16., 14.));
            }
            Self::New => {
                path(&[
                    (14., 3.),
                    (5., 3.),
                    (5., 21.),
                    (19., 21.),
                    (19., 8.),
                    (14., 3.),
                    (14., 8.),
                    (19., 8.),
                ]);
                line((8., 14.), (16., 14.));
                line((12., 10.), (12., 18.));
            }
            Self::Open => {
                path(&[
                    (3., 19.),
                    (3., 5.),
                    (10., 5.),
                    (12., 8.),
                    (21., 8.),
                    (21., 11.),
                ]);
                path(&[(3., 19.), (7., 11.), (22., 11.), (18., 19.), (3., 19.)]);
            }
            Self::Save | Self::SaveAs => {
                path(&[
                    (20., 21.),
                    (3., 21.),
                    (3., 3.),
                    (17., 3.),
                    (21., 7.),
                    (21., 17.),
                ]);
                outline(7., 3., 9., 6.);
                outline(7., 13., 9., 8.);
                if matches!(self, Self::SaveAs) {
                    line((17., 20.), (23., 14.));
                    line((20., 14.), (23., 17.));
                } else {
                    line((21., 17.), (21., 21.));
                }
            }
            Self::Undo | Self::Redo => {
                let mirror = |x: f32| {
                    if matches!(self, Self::Redo) {
                        24. - x
                    } else {
                        x
                    }
                };
                let points: Vec<_> = [(4., 10.), (12., 10.), (17., 12.), (19., 16.), (17., 20.)]
                    .into_iter()
                    .map(|(x, y)| (mirror(x), y))
                    .collect();
                path(&points);
                path(&[(mirror(9.), 5.), (mirror(4.), 10.), (mirror(9.), 15.)]);
            }
            Self::Cut => {
                circle(6., 17., 3.);
                circle(18., 17., 3.);
                line((8., 15.), (19., 3.));
                line((16., 15.), (5., 3.));
            }
            Self::Copy => {
                outline(8., 7., 12., 14.);
                path(&[(15., 4.), (15., 3.), (3., 3.), (3., 17.), (5., 17.)]);
            }
            Self::Paste => {
                path(&[
                    (8., 5.),
                    (4., 5.),
                    (4., 21.),
                    (20., 21.),
                    (20., 5.),
                    (16., 5.),
                ]);
                outline(8., 3., 8., 4.);
                line((8., 12.), (16., 12.));
                line((8., 16.), (14., 16.));
            }
            Self::Bold => {
                path(&[
                    (6., 12.),
                    (14., 12.),
                    (17., 10.),
                    (17., 6.),
                    (14., 3.),
                    (6., 3.),
                    (6., 21.),
                    (14., 21.),
                    (18., 18.),
                    (18., 15.),
                    (14., 12.),
                ]);
            }
            Self::Italic => {
                line((9., 3.), (19., 3.));
                line((5., 21.), (15., 21.));
                line((15., 3.), (9., 21.));
            }
            Self::Underline => {
                path(&[
                    (5., 3.),
                    (5., 12.),
                    (7., 16.),
                    (12., 18.),
                    (17., 16.),
                    (19., 12.),
                    (19., 3.),
                ]);
                line((4., 22.), (20., 22.));
            }
            Self::Strike => {
                path(&[
                    (18., 5.),
                    (15., 3.),
                    (9., 3.),
                    (6., 6.),
                    (7., 10.),
                    (16., 14.),
                    (18., 18.),
                    (15., 21.),
                    (9., 21.),
                    (6., 19.),
                ]);
                line((3., 12.), (21., 12.));
            }
            Self::Superscript | Self::Subscript => {
                line((4., 8.), (13., 20.));
                line((13., 8.), (4., 20.));
                let y = if matches!(self, Self::Superscript) {
                    2.
                } else {
                    13.
                };
                path(&[
                    (16., y + 2.),
                    (18., y),
                    (21., y + 1.),
                    (21., y + 3.),
                    (16., y + 7.),
                    (22., y + 7.),
                ]);
            }
            Self::Highlight => {
                path(&[
                    (7., 16.),
                    (5., 13.),
                    (15., 3.),
                    (21., 9.),
                    (11., 19.),
                    (7., 16.),
                    (4., 20.),
                    (8., 20.),
                    (11., 19.),
                ]);
                line((11., 7.), (17., 13.));
                line((3., 23.), (21., 23.));
            }
            Self::ClearFormatting => {
                path(&[(3., 15.), (8., 3.), (13., 15.)]);
                line((5., 11.), (11., 11.));
                path(&[
                    (12., 21.),
                    (9., 18.),
                    (16., 11.),
                    (21., 16.),
                    (16., 21.),
                    (12., 21.),
                ]);
                line((13., 14.), (18., 19.));
            }
            Self::AlignLeft | Self::AlignCenter | Self::AlignRight | Self::AlignJustify => {
                for (index, y) in [4., 9., 14., 19.].into_iter().enumerate() {
                    let short = index % 2 == 1 && !matches!(self, Self::AlignJustify);
                    let (left, right) = if short {
                        match self {
                            Self::AlignCenter => (7., 17.),
                            Self::AlignRight => (11., 21.),
                            _ => (3., 13.),
                        }
                    } else {
                        (3., 21.)
                    };
                    line((left, y), (right, y));
                }
            }
            Self::Find | Self::Zoom => {
                circle(10., 10., 7.);
                line((15., 15.), (22., 22.));
                if matches!(self, Self::Zoom) {
                    line((6., 10.), (14., 10.));
                    line((10., 6.), (10., 14.));
                }
            }
            Self::PageBreak => {
                path(&[(5., 9.), (5., 3.), (19., 3.), (19., 9.)]);
                path(&[(5., 15.), (5., 21.), (19., 21.), (19., 15.)]);
                for x in [2., 8., 14., 20.] {
                    line((x, 12.), (x + 2., 12.));
                }
            }
            Self::FitWidth => {
                line((3., 4.), (3., 20.));
                line((21., 4.), (21., 20.));
                line((5., 12.), (19., 12.));
                path(&[(8., 8.), (4., 12.), (8., 16.)]);
                path(&[(16., 8.), (20., 12.), (16., 16.)]);
            }
            Self::Close => {
                line((5., 5.), (19., 19.));
                line((5., 19.), (19., 5.));
            }
            Self::Statistics => {
                outline(4., 13., 3., 8.);
                outline(10., 8., 3., 13.);
                outline(16., 3., 3., 18.);
            }
            Self::SelectAll => {
                for x in [3., 9., 15.] {
                    line((x, 3.), (x + 3., 3.));
                    line((x, 21.), (x + 3., 21.));
                }
                line((3., 3.), (3., 21.));
                line((21., 3.), (21., 21.));
                line((7., 9.), (17., 9.));
                line((7., 15.), (17., 15.));
            }
            Self::Focus | Self::ExitFocus => {
                path(&[(9., 3.), (3., 3.), (3., 9.)]);
                path(&[(15., 3.), (21., 3.), (21., 9.)]);
                path(&[(3., 15.), (3., 21.), (9., 21.)]);
                path(&[(21., 15.), (21., 21.), (15., 21.)]);
                if matches!(self, Self::ExitFocus) {
                    line((8., 8.), (16., 16.));
                    line((16., 8.), (8., 16.));
                }
            }
            Self::Moon => {
                path(&[
                    (17., 4.),
                    (14., 3.),
                    (10., 4.),
                    (7., 8.),
                    (7., 13.),
                    (10., 18.),
                    (15., 20.),
                    (20., 18.),
                ]);
                path(&[(17., 4.), (14., 8.), (14., 13.), (17., 17.), (20., 18.)]);
            }
            Self::Sun => {
                circle(12., 12., 5.);
                for (a, b) in [
                    ((12., 2.), (12., 5.)),
                    ((12., 19.), (12., 22.)),
                    ((2., 12.), (5., 12.)),
                    ((19., 12.), (22., 12.)),
                    ((5., 5.), (7., 7.)),
                    ((17., 17.), (19., 19.)),
                    ((19., 5.), (17., 7.)),
                    ((7., 17.), (5., 19.)),
                ] {
                    line(a, b);
                }
            }
            Self::Info => {
                circle(12., 12., 9.);
                line((12., 10.), (12., 17.));
                circle(12., 7., 0.8);
            }
            Self::Connection => {
                path(&[
                    (10., 6.),
                    (7., 6.),
                    (4., 9.),
                    (4., 15.),
                    (7., 18.),
                    (10., 18.),
                ]);
                path(&[
                    (14., 6.),
                    (17., 6.),
                    (20., 9.),
                    (20., 15.),
                    (17., 18.),
                    (14., 18.),
                ]);
                line((8., 12.), (16., 12.));
            }
        }
    }
}

/// One click/focus target covers both icon and label. Labels also reach AccessKit.
pub struct IconButton<'a> {
    icon: Icon,
    label: &'a str,
    selected: Option<bool>,
    primary: bool,
    compact: bool,
}
impl<'a> IconButton<'a> {
    pub fn new(icon: Icon, label: &'a str) -> Self {
        Self {
            icon,
            label,
            selected: None,
            primary: false,
            compact: false,
        }
    }
    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = Some(selected);
        self
    }
    pub fn primary(mut self) -> Self {
        self.primary = true;
        self
    }
    /// Show just the icon while retaining its accessible label and tooltip.
    pub fn compact(mut self) -> Self {
        self.compact = true;
        self
    }
}
impl Widget for IconButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let padding = ui.spacing().button_padding;
        let icon_size = 16.0;
        let gap = if self.compact { 0.0 } else { 7.0 };
        let galley = ui.painter().layout_no_wrap(
            if self.compact {
                String::new()
            } else {
                self.label.to_owned()
            },
            egui::TextStyle::Button.resolve(ui.style()),
            Color32::PLACEHOLDER,
        );
        let size = Vec2::new(
            padding.x * 2. + icon_size + gap + galley.size().x,
            (galley.size().y.max(icon_size) + padding.y * 2.).max(ui.spacing().interact_size.y),
        );
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        response.widget_info(|| match self.selected {
            Some(selected) => {
                WidgetInfo::selected(WidgetType::Button, ui.is_enabled(), selected, self.label)
            }
            None => WidgetInfo::labeled(WidgetType::Button, ui.is_enabled(), self.label),
        });
        if ui.is_rect_visible(rect) {
            let visuals = ui
                .style()
                .interact_selectable(&response, self.selected.unwrap_or(false));
            let (fill, stroke, color) = if self.primary {
                (crate::theme::ACCENT, Stroke::NONE, Color32::WHITE)
            } else {
                (
                    visuals.weak_bg_fill,
                    visuals.bg_stroke,
                    visuals.fg_stroke.color,
                )
            };
            ui.painter().rect(
                rect,
                visuals.corner_radius,
                fill,
                stroke,
                egui::StrokeKind::Inside,
            );
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect.expand(2.),
                    visuals.corner_radius,
                    Stroke::new(1., crate::theme::ACCENT),
                    egui::StrokeKind::Outside,
                );
            }
            let icon_rect = Rect::from_min_size(
                Pos2::new(rect.left() + padding.x, rect.center().y - icon_size * 0.5),
                Vec2::splat(icon_size),
            );
            let accent = if self.primary || !ui.is_enabled() {
                color
            } else {
                match self.icon {
                    Icon::Save | Icon::ExportText | Icon::ExportSelection => {
                        Color32::from_rgb(72, 122, 202)
                    }
                    Icon::Cut | Icon::Copy | Icon::Paste => Color32::from_rgb(137, 107, 184),
                    Icon::AlignLeft | Icon::AlignCenter | Icon::AlignRight | Icon::AlignJustify => {
                        Color32::from_rgb(46, 146, 138)
                    }
                    _ => color,
                }
            };
            self.icon.paint(ui.painter(), icon_rect, accent);
            ui.painter().galley(
                Pos2::new(
                    icon_rect.right() + gap,
                    rect.center().y - galley.size().y * 0.5,
                ),
                galley,
                color,
            );
        }
        response.on_hover_text(self.label)
    }
}

/// A bounded recent-document row with a readable filename/location hierarchy.
pub struct RecentDocumentButton<'a> {
    pub name: &'a str,
    pub location: &'a str,
    pub width: f32,
}
impl Widget for RecentDocumentButton<'_> {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(self.width.max(60.0), 44.0), Sense::click());
        response.widget_info(|| {
            WidgetInfo::labeled(
                WidgetType::Button,
                ui.is_enabled(),
                format!("{} — {}", self.name, self.location),
            )
        });
        if response.gained_focus() {
            response.scroll_to_me(Some(egui::Align::Center));
        }
        if ui.is_rect_visible(rect) {
            let visuals = ui.style().interact(&response);
            ui.painter().rect(
                rect,
                visuals.corner_radius,
                visuals.weak_bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Inside,
            );
            if response.has_focus() {
                ui.painter().rect_stroke(
                    rect.expand(1.0),
                    visuals.corner_radius,
                    Stroke::new(1.0, crate::theme::ACCENT),
                    egui::StrokeKind::Outside,
                );
            }
            Icon::Recent.paint(
                ui.painter(),
                Rect::from_min_size(rect.min + Vec2::new(8., 14.), Vec2::splat(16.)),
                visuals.fg_stroke.color,
            );
            for (text, size, y, color) in [
                (self.name, 13.0, 6.0, visuals.fg_stroke.color),
                (
                    self.location,
                    11.0,
                    24.0,
                    if ui.is_enabled() {
                        ui.visuals().weak_text_color()
                    } else {
                        visuals.fg_stroke.color
                    },
                ),
            ] {
                let mut job = egui::text::LayoutJob::simple(
                    text.to_owned(),
                    egui::FontId::proportional(size),
                    color,
                    (rect.width() - 42.0).max(1.0),
                );
                job.wrap.max_rows = 1;
                let galley = ui.painter().layout_job(job);
                ui.painter()
                    .galley(rect.min + Vec2::new(34., y), galley, color);
            }
        }
        response
    }
}
