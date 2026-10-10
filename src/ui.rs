use crate::catalog::{Catalog, PluginRef};
use crate::domain::{Advice, AdviceKind, Board, Flight, Query, Scope, Tier};
use crate::graph::{self, Camera, Layout, Placed};
use crate::ledger::{Scanner, Snapshot};
use crate::project::{self, Directory};
use crate::theme;
use eframe::egui::{
    self, Align2, Color32, CornerRadius, CursorIcon, FontId, PointerButton, Pos2, Rect, Sense,
    Stroke, Vec2, Visuals,
};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub struct App {
    catalog: Catalog,
    plugins: Vec<PluginRef>,
    plugin_id: String,
    cursor_home: PathBuf,
    query: Query,
    scanner: Scanner,
    snap: Option<Snapshot>,
    directory: Directory,
    disk_stamp: u64,
    snap_digest: u64,
    board: Option<Board>,
    last_scan: Instant,
    selected: Option<String>,
    hover: Option<String>,
    status: String,
    layout: Layout,
    camera: Option<Camera>,
    dragging: bool,
    flashes: Flashes,
    chat_filter: String,
    locked: bool,
    always_labels: bool,
    needs_fit: bool,
    roster: bool,
    open: HudOpen,
}

struct HudOpen {
    plugin: bool,
    scope: bool,
    stats: bool,
    prompt: bool,
    graph: bool,
    lanes: bool,
    tools: bool,
}

impl Default for HudOpen {
    fn default() -> Self {
        Self {
            plugin: true,
            scope: true,
            stats: true,
            prompt: true,
            graph: true,
            lanes: true,
            tools: true,
        }
    }
}

impl App {
    pub fn new(
        catalog: Catalog,
        plugins: Vec<PluginRef>,
        cursor_home: PathBuf,
        query: Query,
    ) -> Self {
        let plugin_id = plugins
            .iter()
            .find(|p| p.root == catalog.plugin_root)
            .map(|p| p.id.clone())
            .unwrap_or_else(|| catalog.plugin_name.clone());
        let mut app = Self {
            catalog,
            plugins,
            plugin_id,
            cursor_home,
            query,
            scanner: Scanner::default(),
            snap: None,
            directory: Directory {
                workspaces: Vec::new(),
            },
            disk_stamp: 0,
            snap_digest: 0,
            board: None,
            last_scan: Instant::now() - Duration::from_secs(10),
            selected: None,
            hover: None,
            status: String::new(),
            layout: Layout::default(),
            camera: None,
            dragging: false,
            flashes: Flashes::default(),
            chat_filter: String::new(),
            locked: false,
            always_labels: false,
            needs_fit: true,
            roster: false,
            open: HudOpen::default(),
        };
        app.refresh();
        app
    }

    fn refresh(&mut self) {
        let stamp = Scanner::disk_stamp(&self.cursor_home);
        if stamp == self.disk_stamp && self.board.is_some() {
            self.last_scan = Instant::now();
            return;
        }
        self.disk_stamp = stamp;
        match self.scanner.refresh(&self.cursor_home, &self.catalog) {
            Ok(snap) => {
                self.status = format!("digest {:x}  events {}", snap.digest, snap.events.len());
                if self.board.is_none() || snap.digest != self.snap_digest {
                    self.snap_digest = snap.digest;
                    self.directory = project::directory(&snap);
                    self.snap = Some(snap);
                    self.rebuild_board();
                }
            }
            Err(err) => self.status = format!("scan failed: {err}"),
        }
        self.last_scan = Instant::now();
    }

    fn rebuild_board(&mut self) {
        let Some(snap) = &self.snap else {
            return;
        };
        let board = project::board(snap, &self.catalog, &self.query);
        self.layout = graph::layout_seeded(&board, Some(&self.layout));
        if let Some(label) = &self.selected {
            if !self.layout.nodes.iter().any(|n| n.label == *label) {
                self.selected = None;
            }
        }
        self.board = Some(board);
    }

    fn switch_plugin(&mut self, plugin: PluginRef) {
        if plugin.id == self.plugin_id && plugin.root == self.catalog.plugin_root {
            return;
        }
        match Catalog::load(plugin.root.clone()) {
            Ok(catalog) => {
                self.catalog = catalog;
                self.plugin_id = plugin.id;
                self.scanner = Scanner::default();
                self.disk_stamp = 0;
                self.snap_digest = 0;
                self.board = None;
                self.layout = Layout::default();
                self.camera = None;
                self.needs_fit = true;
                self.selected = None;
                self.refresh();
            }
            Err(err) => self.status = format!("plugin load failed: {err}"),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.last_scan.elapsed() > Duration::from_millis(250) {
            self.refresh();
        }
        if board_has_flights(&self.board) || self.flashes.alive() || self.needs_fit {
            ctx.request_repaint();
        } else {
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        apply_visuals(ctx);
        if self.roster {
            let advice = self.board.as_ref().map(project::advise).unwrap_or_default();
            paint_roster(ctx, &mut self.roster, &self.catalog.plugin_name, &advice);
        }

        let Some(board) = self.board.clone() else {
            return;
        };

        egui::SidePanel::left("hud")
            .exact_width(theme::HUD_W)
            .resizable(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.add_space(theme::PAD);
                        ui.label(
                            egui::RichText::new("llm-plugin-analyzer")
                                .color(theme::TEXT)
                                .size(theme::TYPE_TITLE),
                        );
                        ui.add_space(6.0);
                        ui.label(
                            egui::RichText::new(&self.status)
                                .color(theme::MUTED)
                                .size(11.0),
                        );
                        ui.add_space(10.0);
                        if section(ui, "PLUGIN", &mut self.open.plugin) {
                            if paint_plugin(ui, &self.plugins, &mut self.plugin_id) {
                                if let Some(plugin) = self
                                    .plugins
                                    .iter()
                                    .find(|p| p.id == self.plugin_id)
                                    .cloned()
                                {
                                    self.switch_plugin(plugin);
                                }
                            }
                            if ui
                                .add_sized(
                                    [ui.available_width(), 24.0],
                                    egui::Button::new(
                                        egui::RichText::new("Skill roster")
                                            .color(theme::TEXT)
                                            .size(theme::TYPE_HUD),
                                    ),
                                )
                                .clicked()
                            {
                                self.roster = true;
                            }
                            ui.add_space(8.0);
                        }
                        if section(ui, "SCOPE", &mut self.open.scope) {
                            if paint_scope(
                                ui,
                                &mut self.query,
                                &self.directory,
                                &mut self.chat_filter,
                            ) {
                                self.camera = None;
                                self.needs_fit = true;
                                self.rebuild_board();
                            }
                            ui.add_space(8.0);
                        }
                        if section(ui, "STATS", &mut self.open.stats) {
                            stat_row(ui, &mut self.flashes, "Sessions", board.sessions);
                            stat_row(ui, &mut self.flashes, "Events", board.events);
                            stat_row(
                                ui,
                                &mut self.flashes,
                                "Links",
                                self.layout.edges.len() as u32,
                            );
                            stat_row(ui, &mut self.flashes, "Routing", board.routing_hits);
                            let of_flash =
                                self.flashes.tick("RoutingExpected", board.routing_expected);
                            ui.label(
                                egui::RichText::new(format!("of {}", board.routing_expected))
                                    .color(mix(theme::MUTED, theme::FOCUS, of_flash * 0.7))
                                    .size(11.0 + of_flash * 1.2),
                            );
                            ui.add_space(8.0);
                        }
                        if section(ui, "PROMPT", &mut self.open.prompt) {
                            if let Some(preview) = &board.prompt_preview {
                                ui.label(
                                    egui::RichText::new(preview)
                                        .color(theme::TEXT)
                                        .size(theme::TYPE_HUD),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("No prompt in this scope")
                                        .color(theme::MUTED)
                                        .size(theme::TYPE_HUD),
                                );
                            }
                            ui.add_space(8.0);
                        }
                        if section(ui, "GRAPH", &mut self.open.graph) {
                            if board.live {
                                ui.label(
                                    egui::RichText::new("LIVE")
                                        .color(theme::HIT)
                                        .size(11.0)
                                        .strong(),
                                );
                            }
                            ui.checkbox(&mut self.locked, "locked");
                            ui.checkbox(&mut self.always_labels, "always show labels");
                            stat_row(
                                ui,
                                &mut self.flashes,
                                "Nodes",
                                self.layout.nodes.len() as u32,
                            );
                            stat_row(ui, &mut self.flashes, "Visits", board.visits);
                            ui.add_space(8.0);
                        }
                        if section(ui, "LANES", &mut self.open.lanes) {
                            swatch(ui, theme::PROMPT, "Chat");
                            swatch(ui, theme::PLAYBOOK, "Playbooks");
                            swatch(ui, theme::SKILL, "Skills");
                            swatch(ui, theme::PRINCIPLE, "Principles");
                            swatch(ui, theme::MODEL, "Models");
                            ui.add_space(8.0);
                        }
                        if section(ui, "TOOLS", &mut self.open.tools) {
                            if board.tools.is_empty() {
                                ui.label(
                                    egui::RichText::new("No tool calls yet").color(theme::MUTED),
                                );
                            }
                            for (name, n) in &board.tools {
                                stat_row(ui, &mut self.flashes, name, *n);
                            }
                        }
                        ui.add_space(theme::PAD);
                    });
            });

        egui::SidePanel::right("inspect")
            .exact_width(theme::INSPECT_W)
            .resizable(false)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        paint_inspect(
                            ui,
                            &mut self.flashes,
                            &board,
                            self.selected.as_deref(),
                            self.snap.as_ref(),
                            &self.catalog,
                            &self.query,
                        );
                    });
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            paint_graph(
                ui,
                &mut self.camera,
                &self.layout,
                &board.flights,
                &mut self.selected,
                &mut self.hover,
                &mut self.dragging,
                &mut self.needs_fit,
                self.locked,
                self.always_labels,
            );
        });
    }
}

fn board_has_flights(board: &Option<Board>) -> bool {
    board.as_ref().is_some_and(|b| !b.flights.is_empty())
}

fn paint_inspect(
    ui: &mut egui::Ui,
    flashes: &mut Flashes,
    board: &Board,
    selected: Option<&str>,
    snap: Option<&Snapshot>,
    catalog: &Catalog,
    query: &Query,
) {
    ui.add_space(theme::PAD);
    ui.label(
        egui::RichText::new("Inspect")
            .color(theme::TEXT)
            .size(theme::TYPE_TITLE),
    );
    ui.add_space(10.0);
    let Some(label) = selected else {
        ui.label(
            egui::RichText::new("Click a node. Drag to pan. Scroll to zoom.")
                .color(theme::MUTED)
                .size(theme::TYPE_HUD),
        );
        return;
    };
    let Some(info) = project::inspect(board, label) else {
        ui.label(
            egui::RichText::new("No data for this node.")
                .color(theme::MUTED)
                .size(theme::TYPE_HUD),
        );
        return;
    };
    ui.label(
        egui::RichText::new(&info.label)
            .color(theme::TEXT)
            .size(16.0),
    );
    ui.label(
        egui::RichText::new(&info.lane)
            .color(theme::MUTED)
            .size(11.0),
    );
    if let Some(detail) = &info.detail {
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(detail)
                .color(theme::TEXT)
                .size(theme::TYPE_HUD),
        );
    }
    ui.add_space(12.0);
    egui::Grid::new("inspect-grid")
        .num_columns(2)
        .spacing([16.0, 10.0])
        .min_col_width(88.0)
        .show(ui, |ui| {
            metric_pill(
                ui,
                flashes,
                &info.label,
                "Hit",
                info.hit,
                theme::HIT,
                Color32::BLACK,
            );
            metric_pill(
                ui,
                flashes,
                &info.label,
                "Miss",
                info.miss,
                theme::MISS,
                Color32::WHITE,
            );
            metric_pill(
                ui,
                flashes,
                &info.label,
                "Unknown",
                info.unknown,
                theme::UNK,
                Color32::BLACK,
            );
            metric_num(ui, flashes, &info.label, "Contacts", info.contacts);
            metric_num(ui, flashes, &info.label, "Ambient", info.ambient);
        });
    if let Some(snap) = snap {
        let series = project::inspect_series(snap, catalog, query, label);
        ui.add_space(14.0);
        paint_series_chart(ui, "Hits", &series.hits, theme::HIT);
        ui.add_space(10.0);
        paint_series_chart(ui, "Misses", &series.misses, theme::MISS);
    }
}

fn paint_series_chart(ui: &mut egui::Ui, title: &str, values: &[u32], color: Color32) {
    ui.label(egui::RichText::new(title).color(theme::MUTED).size(11.0));
    ui.add_space(4.0);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 72.0), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, Color32::from_rgb(4, 6, 12));
    painter.rect_stroke(
        rect,
        0.0,
        Stroke::new(1.0_f32, theme::EDGE),
        egui::StrokeKind::Inside,
    );
    if values.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No history",
            FontId::proportional(11.0),
            theme::MUTED,
        );
        return;
    }
    let max = values.iter().copied().max().unwrap_or(0).max(1) as f32;
    draw_series_line(&painter, rect.shrink(8.0), values, max, color);
}

fn draw_series_line(painter: &egui::Painter, rect: Rect, values: &[u32], max: f32, color: Color32) {
    if values.is_empty() {
        return;
    }
    let last = (values.len() - 1).max(1) as f32;
    let pts: Vec<Pos2> = values
        .iter()
        .enumerate()
        .map(|(i, v)| {
            let x = rect.left() + rect.width() * (i as f32 / last);
            let y = rect.bottom() - rect.height() * (*v as f32 / max);
            Pos2::new(x, y)
        })
        .collect();
    if pts.len() == 1 {
        painter.circle_filled(pts[0], 2.5, color);
        return;
    }
    for pair in pts.windows(2) {
        painter.line_segment([pair[0], pair[1]], Stroke::new(1.4_f32, color));
    }
}

fn metric_pill(
    ui: &mut egui::Ui,
    flashes: &mut Flashes,
    node: &str,
    name: &str,
    n: u32,
    fill: Color32,
    fg: Color32,
) {
    let amt = flashes.tick(&format!("{node}:{name}"), n);
    ui.label(
        egui::RichText::new(name)
            .color(theme::MUTED)
            .size(theme::TYPE_HUD),
    );
    let text = format!("{n}");
    let galley =
        ui.painter()
            .layout_no_wrap(text, FontId::proportional(theme::TYPE_HUD + amt * 1.2), fg);
    let size = Vec2::new(
        (galley.size().x + 16.0).max(32.0) + amt * 4.0,
        22.0 + amt * 2.0,
    );
    let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
    ui.painter().rect_filled(
        rect,
        CornerRadius::same(theme::PILL_R),
        mix(fill, theme::FOCUS, amt * 0.45),
    );
    ui.painter()
        .galley(rect.center() - galley.size() * 0.5, galley, fg);
    ui.end_row();
}

fn metric_num(ui: &mut egui::Ui, flashes: &mut Flashes, node: &str, name: &str, n: u32) {
    let amt = flashes.tick(&format!("{node}:{name}"), n);
    ui.label(
        egui::RichText::new(name)
            .color(theme::MUTED)
            .size(theme::TYPE_HUD),
    );
    ui.label(
        egui::RichText::new(n.to_string())
            .color(mix(theme::TEXT, theme::FOCUS, amt))
            .size(theme::TYPE_HUD + amt * 1.8),
    );
    ui.end_row();
}

fn section(ui: &mut egui::Ui, title: &str, open: &mut bool) -> bool {
    let t = ui
        .ctx()
        .animate_bool_with_time(ui.id().with(title), *open, 0.16);
    ui.horizontal(|ui| {
        paint_chevron(ui, t);
        let resp = ui.add(
            egui::Label::new(
                egui::RichText::new(title)
                    .color(theme::MUTED)
                    .size(11.0)
                    .strong(),
            )
            .sense(Sense::click()),
        );
        if resp.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }
        if resp.clicked() {
            *open = !*open;
        }
    });
    ui.add_space(4.0);
    *open
}

fn paint_chevron(ui: &mut egui::Ui, t: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(12.0), Sense::hover());
    let rot = egui::emath::Rot2::from_angle(t * std::f32::consts::FRAC_PI_2);
    let c = rect.center();
    let tip = c + rot * Vec2::new(4.0, 0.0);
    let a = c + rot * Vec2::new(-2.4, -3.1);
    let b = c + rot * Vec2::new(-2.4, 3.1);
    ui.painter().add(egui::Shape::convex_polygon(
        vec![tip, a, b],
        theme::MUTED,
        Stroke::NONE,
    ));
}

fn paint_plugin(ui: &mut egui::Ui, plugins: &[PluginRef], plugin_id: &mut String) -> bool {
    let before = plugin_id.clone();
    let selected = plugins
        .iter()
        .find(|p| p.id == *plugin_id)
        .map(|p| p.name.clone())
        .unwrap_or_else(|| plugin_id.clone());
    ui.label(egui::RichText::new("Plugin").color(theme::MUTED).size(11.0));
    egui::ComboBox::from_id_salt("plugin")
        .selected_text(ellipsize(&selected, 28))
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            for plugin in plugins {
                if ui
                    .selectable_label(plugin.id == *plugin_id, &plugin.name)
                    .clicked()
                {
                    *plugin_id = plugin.id.clone();
                }
            }
        });
    *plugin_id != before
}

fn paint_roster(ctx: &egui::Context, open: &mut bool, plugin: &str, advice: &[Advice]) {
    ctx.show_viewport_immediate(
        egui::ViewportId::from_hash_of("skill-roster"),
        egui::ViewportBuilder::default()
            .with_title("Skill roster")
            .with_inner_size([560.0, 720.0]),
        |ctx, _class| {
            apply_visuals(ctx);
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add_space(theme::PAD);
                ui.label(
                    egui::RichText::new("SKILL ROSTER")
                        .color(theme::MUTED)
                        .size(11.0)
                        .strong(),
                );
                ui.label(
                    egui::RichText::new(plugin)
                        .color(theme::TEXT)
                        .size(theme::TYPE_TITLE),
                );
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new(
                        "Used skills, ranked. Remove or tighten the ones that miss.",
                    )
                    .color(theme::MUTED)
                    .size(theme::TYPE_HUD),
                );
                ui.add_space(12.0);
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        if advice.is_empty() {
                            ui.label(
                                egui::RichText::new("No skill reads in this scope yet")
                                    .color(theme::MUTED),
                            );
                            return;
                        }
                        for row in advice {
                            roster_row(ui, row);
                            ui.add_space(10.0);
                        }
                    });
            });
            if ctx.input(|i| i.viewport().close_requested()) {
                *open = false;
            }
        },
    );
}

fn roster_row(ui: &mut egui::Ui, row: &Advice) {
    ui.horizontal(|ui| {
        let (tier_bg, tier_fg) = match row.tier {
            Tier::S | Tier::A => (theme::HIT, Color32::BLACK),
            Tier::B => (theme::UNK, Color32::BLACK),
            Tier::C | Tier::D => (theme::MODEL, Color32::WHITE),
            Tier::F => (theme::MISS, Color32::WHITE),
        };
        let (rect, _) = ui.allocate_exact_size(Vec2::new(22.0, 22.0), Sense::hover());
        ui.painter()
            .rect_filled(rect, CornerRadius::same(4), tier_bg);
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            row.tier.label(),
            FontId::proportional(12.0),
            tier_fg,
        );
        ui.label(
            egui::RichText::new(&row.label)
                .color(theme::TEXT)
                .size(theme::TYPE_TITLE),
        );
        ui.label(
            egui::RichText::new(&row.lane)
                .color(theme::MUTED)
                .size(11.0),
        );
    });
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "hit {}  miss {}  unk {}  n {}",
                row.hit, row.miss, row.unknown, row.contacts
            ))
            .color(theme::MUTED)
            .size(11.0),
        );
    });
    match &row.kind {
        AdviceKind::Keep => {
            ui.label(
                egui::RichText::new("Keep. This skill earns its reads.")
                    .color(theme::HIT)
                    .size(theme::TYPE_HUD),
            );
        }
        AdviceKind::Optimize { reason } => {
            ui.label(
                egui::RichText::new(format!("Optimize. {reason}"))
                    .color(theme::UNK)
                    .size(theme::TYPE_HUD),
            );
        }
        AdviceKind::Remove { reason } => {
            ui.label(
                egui::RichText::new(format!("Remove. {reason}"))
                    .color(theme::MISS)
                    .size(theme::TYPE_HUD),
            );
        }
    }
}

fn paint_scope(
    ui: &mut egui::Ui,
    query: &mut Query,
    directory: &Directory,
    chat_filter: &mut String,
) -> bool {
    let before = query.scope.clone();
    ui.label(
        egui::RichText::new("Workspace")
            .color(theme::MUTED)
            .size(11.0),
    );
    let workspace_text = match &query.scope {
        Scope::All => "All workspaces".to_string(),
        Scope::Workspace(workspace) | Scope::Chat { workspace, .. } => tail_label(workspace, 28),
    };
    egui::ComboBox::from_id_salt("workspace")
        .selected_text(workspace_text)
        .width(ui.available_width())
        .show_ui(ui, |ui| {
            if ui
                .selectable_label(matches!(query.scope, Scope::All), "All workspaces")
                .clicked()
            {
                query.scope = Scope::All;
            }
            for workspace in &directory.workspaces {
                let selected = match &query.scope {
                    Scope::Workspace(id) | Scope::Chat { workspace: id, .. } => id == &workspace.id,
                    Scope::All => false,
                };
                if ui
                    .selectable_label(selected, tail_label(&workspace.id, 36))
                    .clicked()
                {
                    query.scope = Scope::Workspace(workspace.id.clone());
                }
            }
        });
    if !matches!(query.scope, Scope::All) {
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Chat").color(theme::MUTED).size(11.0));
        let chat_text = match &query.scope {
            Scope::Chat { conversation, .. } => directory
                .workspaces
                .iter()
                .flat_map(|w| w.chats.iter())
                .find(|c| c.id == *conversation)
                .map(|c| c.label.clone())
                .unwrap_or_else(|| tail_label(conversation, 28)),
            _ => "All chats".to_string(),
        };
        let workspace_id = match &query.scope {
            Scope::Workspace(id) | Scope::Chat { workspace: id, .. } => Some(id.clone()),
            Scope::All => None,
        };
        let chats = workspace_id.as_ref().and_then(|id| {
            directory
                .workspaces
                .iter()
                .find(|w| &w.id == id)
                .map(|w| w.chats.as_slice())
        });
        egui::ComboBox::from_id_salt("chat")
            .selected_text(ellipsize(&chat_text, 32))
            .width(ui.available_width())
            .show_ui(ui, |ui| {
                if chats.is_some_and(|c| c.len() > 12) {
                    ui.add(
                        egui::TextEdit::singleline(chat_filter)
                            .hint_text("Filter chats…")
                            .desired_width(ui.available_width()),
                    );
                }
                if ui
                    .selectable_label(matches!(query.scope, Scope::Workspace(_)), "All chats")
                    .clicked()
                {
                    if let Some(id) = workspace_id.clone() {
                        query.scope = Scope::Workspace(id);
                    }
                }
                if let Some(chats) = chats {
                    let filter = chat_filter.to_ascii_lowercase();
                    egui::ScrollArea::vertical()
                        .max_height(240.0)
                        .show(ui, |ui| {
                            for chat in chats {
                                if !filter.is_empty()
                                    && !chat.label.to_ascii_lowercase().contains(&filter)
                                    && !chat.id.to_ascii_lowercase().contains(&filter)
                                {
                                    continue;
                                }
                                let selected = matches!(
                                    &query.scope,
                                    Scope::Chat { conversation, .. } if conversation == &chat.id
                                );
                                if ui.selectable_label(selected, &chat.label).clicked() {
                                    if let Some(id) = workspace_id.clone() {
                                        query.scope = Scope::Chat {
                                            workspace: id,
                                            conversation: chat.id.clone(),
                                        };
                                    }
                                }
                            }
                        });
                }
            });
    }
    query.scope != before
}

fn tail_label(id: &str, keep: usize) -> String {
    let count = id.chars().count();
    if count <= keep {
        return id.to_string();
    }
    let tail: String = id.chars().skip(count - keep).collect();
    format!("…{tail}")
}

fn ellipsize(text: &str, keep: usize) -> String {
    let count = text.chars().count();
    if count <= keep {
        text.to_string()
    } else {
        format!("{}…", text.chars().take(keep).collect::<String>())
    }
}

fn paint_graph(
    ui: &mut egui::Ui,
    camera: &mut Option<Camera>,
    layout: &Layout,
    flights: &[Flight],
    selected: &mut Option<String>,
    hover: &mut Option<String>,
    dragging: &mut bool,
    needs_fit: &mut bool,
    locked: bool,
    always_labels: bool,
) {
    let rect = ui.available_rect_before_wrap();
    let resp = ui.allocate_rect(rect, Sense::click_and_drag());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, theme::CANVAS);

    if layout.nodes.is_empty() {
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            "No skill reads in this scope yet",
            FontId::proportional(theme::TYPE_HUD),
            theme::MUTED,
        );
        return;
    }

    let ready = rect.width() > 80.0 && rect.height() > 80.0;
    if locked && ready {
        *camera = Some(Camera::fitted(layout, rect.width(), rect.height()));
        *dragging = false;
    } else if *needs_fit && ready {
        *camera = Some(Camera::fitted(layout, rect.width(), rect.height()));
        *needs_fit = false;
    }
    let Some(cam) = camera.as_mut() else {
        return;
    };

    if !locked
        && (resp.dragged_by(PointerButton::Primary) || resp.dragged_by(PointerButton::Middle))
    {
        let d = resp.drag_delta();
        cam.pan_x += d.x;
        cam.pan_y += d.y;
        *dragging = true;
    }
    if resp.drag_stopped() {
        *dragging = false;
    }
    let keys = ui.input(|i| {
        (
            i.key_down(egui::Key::ArrowLeft) || i.key_down(egui::Key::A),
            i.key_down(egui::Key::ArrowRight) || i.key_down(egui::Key::D),
            i.key_down(egui::Key::ArrowUp) || i.key_down(egui::Key::W),
            i.key_down(egui::Key::ArrowDown) || i.key_down(egui::Key::S),
            i.pointer.primary_down(),
        )
    });
    let step = 12.0;
    if !locked && keys.0 {
        cam.pan_x += step;
    }
    if !locked && keys.1 {
        cam.pan_x -= step;
    }
    if !locked && keys.2 {
        cam.pan_y += step;
    }
    if !locked && keys.3 {
        cam.pan_y -= step;
    }
    if !locked && (keys.0 || keys.1 || keys.2 || keys.3) {
        ui.ctx().request_repaint();
    }
    if !locked {
        if let Some(pos) = resp.hover_pos() {
            let y = ui.input(|i| i.smooth_scroll_delta.y);
            if y.abs() > 0.1 {
                let world = cam.to_world(pos.x - rect.min.x, pos.y - rect.min.y);
                let next = (cam.zoom * (1.0 + y * 0.0015)).clamp(0.15, 8.0);
                cam.zoom = next;
                let after = cam.to_screen(world.0, world.1);
                cam.pan_x += pos.x - rect.min.x - after.0;
                cam.pan_y += pos.y - rect.min.y - after.1;
            }
        }
    }

    for &(a, b) in &layout.edges {
        let (ax, ay) = cam.to_screen(layout.nodes[a].x, layout.nodes[a].y);
        let (bx, by) = cam.to_screen(layout.nodes[b].x, layout.nodes[b].y);
        painter.line_segment(
            [
                Pos2::new(rect.min.x + ax, rect.min.y + ay),
                Pos2::new(rect.min.x + bx, rect.min.y + by),
            ],
            Stroke::new(0.5_f32, theme::EDGE),
        );
    }

    let pointer = resp
        .hover_pos()
        .map(|p| cam.to_world(p.x - rect.min.x, p.y - rect.min.y));
    let over = pointer.and_then(|w| graph::hit_test(layout, w, 6.0 / cam.zoom));
    *hover = over.map(|i| layout.nodes[i].label.clone());

    if resp.hovered() {
        ui.ctx().set_cursor_icon(if locked {
            if over.is_some() {
                CursorIcon::PointingHand
            } else {
                CursorIcon::Default
            }
        } else if *dragging {
            CursorIcon::Grabbing
        } else if over.is_some() {
            CursorIcon::PointingHand
        } else {
            CursorIcon::Grab
        });
        if ui
            .input(|i| i.pointer.is_moving() || i.pointer.any_pressed() || i.pointer.any_released())
        {
            ui.ctx().request_repaint();
        }
    }

    if resp.clicked() && !*dragging {
        if let Some(i) = over {
            *selected = Some(layout.nodes[i].label.clone());
        }
    }

    let pressed = keys.4 && over.is_some() && !*dragging;

    for n in &layout.nodes {
        let (sx, sy) = cam.to_screen(n.x, n.y);
        let center = Pos2::new(rect.min.x + sx, rect.min.y + sy);
        let hovered = hover.as_deref() == Some(n.label.as_str());
        let chosen = selected.as_deref() == Some(n.label.as_str());
        let active = pressed && hovered;
        let scale = if active {
            0.82
        } else if hovered {
            1.35
        } else if chosen {
            1.15
        } else {
            1.0
        };
        let r = (n.r * cam.zoom * scale).max(2.0);
        paint_node(&painter, n, center, r, hovered, chosen, active);
        if always_labels || chosen || hovered || cam.zoom > 1.4 || n.label == theme::CHAT_LABEL {
            painter.text(
                center + Vec2::new(r + 6.0, 0.0),
                Align2::LEFT_CENTER,
                &n.label,
                FontId::proportional(theme::TYPE_LABEL),
                theme::TEXT,
            );
        }
    }

    let now = ui.input(|i| i.time);
    paint_flights(&painter, rect, cam, layout, flights, now);
}

fn paint_node(
    painter: &egui::Painter,
    node: &Placed,
    center: Pos2,
    r: f32,
    hovered: bool,
    chosen: bool,
    active: bool,
) {
    let fill = if active {
        let [cr, cg, cb, _] = node.fill.to_srgba_unmultiplied();
        Color32::from_rgb(
            cr.saturating_add(40),
            cg.saturating_add(40),
            cb.saturating_add(40),
        )
    } else {
        node.fill
    };
    if chosen {
        painter.circle_filled(center, r * theme::NODE_HALO, theme::wash(theme::FOCUS, 40));
    } else if hovered {
        painter.circle_filled(center, r * theme::NODE_HALO, theme::wash(fill, 44));
    }
    painter.circle_filled(center, r, fill);
    painter.circle_stroke(center, r + 1.1, Stroke::new(1.0_f32, node.ring));
    if node.label == theme::CHAT_LABEL {
        painter.circle_filled(center, (r * 0.42).max(1.2), theme::PROMPT);
    }
}

fn paint_flights(
    painter: &egui::Painter,
    rect: Rect,
    cam: &Camera,
    layout: &Layout,
    flights: &[Flight],
    now: f64,
) {
    for (i, flight) in flights.iter().enumerate() {
        let pts: Vec<_> = flight
            .stops
            .iter()
            .filter_map(|stop| layout.nodes.iter().find(|n| n.label == *stop))
            .collect();
        if pts.is_empty() {
            continue;
        }
        for pair in pts.windows(2) {
            let (ax, ay) = cam.to_screen(pair[0].x, pair[0].y);
            let (bx, by) = cam.to_screen(pair[1].x, pair[1].y);
            painter.line_segment(
                [
                    Pos2::new(rect.min.x + ax, rect.min.y + ay),
                    Pos2::new(rect.min.x + bx, rect.min.y + by),
                ],
                Stroke::new(1.0_f32, theme::wash(theme::PRIMARY, 90)),
            );
        }
        let hops = pts.len().saturating_sub(1) as f64;
        let phase = if hops < 1.0 {
            0.0
        } else {
            (now / theme::HOP_SECS + i as f64 * 0.17).rem_euclid(hops)
        };
        for k in (0..theme::TRAIL).rev() {
            let along = if hops < 1.0 {
                0.0
            } else {
                (phase - k as f64 * theme::TRAIL_STEP).rem_euclid(hops)
            };
            let (x, y) = along_path(&pts, along);
            let (sx, sy) = cam.to_screen(x, y);
            let center = Pos2::new(rect.min.x + sx, rect.min.y + sy);
            let fade = 1.0 - k as f32 / theme::TRAIL as f32;
            let r = (1.4 + fade * 1.8) * cam.zoom.max(0.7);
            painter.circle_filled(
                center,
                r * 3.2,
                theme::wash(theme::PRIMARY, (18.0 * fade) as u8),
            );
            painter.circle_filled(
                center,
                r * 1.8,
                theme::wash(theme::PRIMARY, (48.0 * fade) as u8),
            );
            painter.circle_filled(
                center,
                r * 0.7,
                theme::wash(theme::TRAVEL, (90.0 * fade) as u8),
            );
        }
        let (x, y) = along_path(&pts, phase);
        let (sx, sy) = cam.to_screen(x, y);
        let center = Pos2::new(rect.min.x + sx, rect.min.y + sy);
        let pulse = if hops < 1.0 {
            1.0 + (now * 4.0).sin() as f32 * 0.2
        } else {
            1.0
        };
        let r = (2.1 * cam.zoom * pulse).max(1.8);
        painter.circle_filled(center, r * 5.5, theme::wash(theme::PRIMARY, 18));
        painter.circle_filled(center, r * 3.0, theme::wash(theme::PRIMARY, 50));
        painter.circle_filled(center, r * 1.5, theme::wash(theme::TRAVEL, 160));
        painter.circle_filled(center, r * 0.55, Color32::WHITE);
    }
}

fn along_path(pts: &[&Placed], along: f64) -> (f32, f32) {
    if pts.len() == 1 {
        return (pts[0].x, pts[0].y);
    }
    let hops = pts.len() - 1;
    let hop = (along.floor() as usize).min(hops - 1);
    let t = (along - hop as f64) as f32;
    let t = t * t * (3.0 - 2.0 * t);
    let a = pts[hop];
    let b = pts[hop + 1];
    (a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

fn apply_visuals(ctx: &egui::Context) {
    let mut v = Visuals::dark();
    v.panel_fill = theme::HUD;
    v.window_fill = theme::HUD;
    v.extreme_bg_color = theme::CANVAS;
    v.override_text_color = Some(theme::TEXT);
    v.widgets.noninteractive.fg_stroke.color = theme::TEXT;
    ctx.set_visuals(v);
}

#[derive(Default)]
struct Flashes {
    last: BTreeMap<String, u32>,
    at: BTreeMap<String, Instant>,
}

impl Flashes {
    fn tick(&mut self, key: &str, value: u32) -> f32 {
        match self.last.get(key).copied() {
            None => {
                self.last.insert(key.to_string(), value);
                0.0
            }
            Some(old) if old == value => self.amount(key),
            Some(_) => {
                self.last.insert(key.to_string(), value);
                self.at.insert(key.to_string(), Instant::now());
                1.0
            }
        }
    }

    fn amount(&self, key: &str) -> f32 {
        let Some(at) = self.at.get(key) else {
            return 0.0;
        };
        let t = at.elapsed().as_secs_f32() / theme::FLASH_SECS;
        if t >= 1.0 {
            0.0
        } else {
            1.0 - t
        }
    }

    fn alive(&self) -> bool {
        self.at
            .values()
            .any(|at| at.elapsed().as_secs_f32() < theme::FLASH_SECS)
    }
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let [ar, ag, ab, aa] = a.to_srgba_unmultiplied();
    let [br, bg, bb, ba] = b.to_srgba_unmultiplied();
    Color32::from_rgba_unmultiplied(
        lerp(ar, br, t),
        lerp(ag, bg, t),
        lerp(ab, bb, t),
        lerp(aa, ba, t),
    )
}

fn lerp(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t).round() as u8
}

fn stat_row(ui: &mut egui::Ui, flashes: &mut Flashes, name: &str, n: u32) {
    let amt = flashes.tick(name, n);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(name)
                .color(theme::MUTED)
                .size(theme::TYPE_HUD),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(n.to_string())
                    .color(mix(theme::TEXT, theme::FOCUS, amt))
                    .size(theme::TYPE_HUD + amt * 2.0),
            );
        });
    });
}

#[cfg(test)]
mod flash_tests {
    use super::Flashes;

    #[test]
    fn first_value_does_not_flash() {
        let mut flashes = Flashes::default();
        assert_eq!(flashes.tick("Events", 10), 0.0);
        assert_eq!(flashes.tick("Events", 10), 0.0);
    }

    #[test]
    fn changed_value_flashes() {
        let mut flashes = Flashes::default();
        flashes.tick("Events", 10);
        assert_eq!(flashes.tick("Events", 11), 1.0);
        assert!(flashes.alive());
    }
}

fn swatch(ui: &mut egui::Ui, color: Color32, name: &str) {
    ui.horizontal(|ui| {
        let (rect, _) = ui.allocate_exact_size(Vec2::splat(8.0), Sense::hover());
        ui.painter().circle_filled(rect.center(), 3.5, color);
        ui.label(
            egui::RichText::new(name)
                .color(theme::TEXT)
                .size(theme::TYPE_HUD),
        );
    });
}
