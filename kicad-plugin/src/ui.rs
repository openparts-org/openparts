//! Drawing only: every decision lives in the state modules. Widgets
//! record what was clicked as [`Action`]s, applied after the frame is
//! drawn (no borrow juggling inside closures).

use crate::bom::{self, BomSource};
use crate::search::{self, Column};
use crate::stale;
use crate::{kicad_detect, App, Tab};
use eframe::egui::{self, Color32, RichText, Ui};
use openparts_client::api::{CatalogResponse, CategoryNode, PartBrief};
use openparts_client::kicad_stale::{Origin, StaleStatus};

enum Action {
    Toggle(&'static str, String),
    ToggleOwned(String, String),
    Edited,
    EditedNow,
    Sort(String),
    Page(bool),
    ClearFilters,
    Select(usize),
    Install(Vec<(String, String)>, Option<String>, Tab),
    UseMpn(usize, Box<PartBrief>),
    BomSelect(usize),
    Alternatives(usize),
    Copy(String),
}

const OK: Color32 = Color32::from_rgb(60, 160, 80);
const WARN: Color32 = Color32::from_rgb(220, 150, 30);
const BAD: Color32 = Color32::from_rgb(210, 60, 60);

pub fn draw(app: &mut App, ctx: &egui::Context) {
    let mut actions = Vec::new();

    egui::TopBottomPanel::top("top").show(ctx, |ui| {
        ui.add_space(2.0);
        ui.horizontal(|ui| {
            ui.label("Registry");
            ui.add(egui::TextEdit::singleline(&mut app.registry_url).desired_width(180.0));
            ui.separator();
            ui.label("Project");
            ui.add(
                egui::TextEdit::singleline(&mut app.project_dir)
                    .desired_width(320.0)
                    .hint_text("KiCad project directory"),
            );
            if ui
                .button("Detect")
                .on_hover_text("Ask a running KiCad (IPC API) for its project")
                .clicked()
            {
                match kicad_detect::try_detect_project_dir() {
                    Some(dir) => app.project_dir = dir.display().to_string(),
                    None => app.status = "KiCad IPC not available.".into(),
                }
            }
        });
        ui.horizontal(|ui| {
            ui.selectable_value(&mut app.tab, Tab::Search, "Search");
            ui.selectable_value(&mut app.tab, Tab::Bom, "BOM check");
            ui.selectable_value(&mut app.tab, Tab::Stale, "Footprints");
        });
        ui.add_space(2.0);
    });

    egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
        ui.horizontal(|ui| {
            if app.installing() {
                ui.spinner();
            }
            ui.label(&app.status);
        });
    });

    match app.tab {
        Tab::Search => search_tab(app, ctx, &mut actions),
        Tab::Bom => bom_tab(app, ctx, &mut actions),
        Tab::Stale => stale_tab(app, ctx, &mut actions),
    }

    for a in actions {
        apply(app, ctx, a);
    }
}

fn apply(app: &mut App, ctx: &egui::Context, action: Action) {
    match action {
        Action::Toggle(k, v) => app.search.toggle(k, &v),
        Action::ToggleOwned(k, v) => app.search.toggle(&k, &v),
        Action::Edited => app.search.edited(),
        Action::EditedNow => app.search.edited_now(),
        Action::Sort(k) => app.search.sort_by(k),
        Action::Page(f) => app.search.page(f),
        Action::ClearFilters => app.search.clear_filters(),
        Action::Select(i) => app.search.selected = Some(i),
        Action::Install(parts, rev, from) => app.install(parts, rev, from),
        Action::BomSelect(i) => app.bom.selected = Some(i),
        Action::Alternatives(i) => {
            let registry = app.registry_url.trim().to_string();
            app.bom.fetch_alternatives(i, &registry, &app.waker);
        }
        Action::UseMpn(line, part) => {
            let (Some(report), Some(ex)) = (&app.bom.report, &app.bom.extracted) else {
                return;
            };
            let Some(line) = report.lines.get(line) else {
                return;
            };
            match bom::use_mpn(line, &ex.symbols, &part) {
                Ok((msg, copy)) => {
                    app.status = msg;
                    match copy {
                        Some(text) => ctx.output_mut(|o| o.copied_text = text),
                        None => app.run_bom(),
                    }
                }
                Err(e) => app.status = format!("MPN not written: {e}"),
            }
        }
        Action::Copy(text) => {
            app.status = format!("Copied {text}");
            ctx.output_mut(|o| o.copied_text = text);
        }
    }
}

/// `s` cut to `max` characters with an ellipsis.
fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() > max {
        format!("{}…", s.chars().take(max - 1).collect::<String>())
    } else {
        s.to_string()
    }
}

fn count_label(label: &str, count: usize) -> String {
    format!("{label} ({count})")
}

// ---------------------------------------------------------------- search

fn search_tab(app: &mut App, ctx: &egui::Context, actions: &mut Vec<Action>) {
    // Taken for the frame (no per-frame clone), put back at the end.
    let response = app.search.response.take();
    egui::SidePanel::left("facets")
        .resizable(true)
        .default_width(250.0)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                if let Some(r) = &response {
                    facets(ui, r, &mut app.search.request.ranges, actions);
                }
            });
        });

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            let edit = ui.add(
                egui::TextEdit::singleline(&mut app.search.request.q)
                    .desired_width(320.0)
                    .hint_text("MPN, manufacturer, value…"),
            );
            if edit.changed() {
                actions.push(Action::Edited);
            }
            if edit.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                actions.push(Action::EditedNow);
            }
            if app.search.busy() {
                ui.spinner();
            }
            if app.search.has_filters() && ui.button("Clear filters").clicked() {
                actions.push(Action::ClearFilters);
            }
            if let Some(r) = &response {
                let from = if r.total == 0 { 0 } else { r.offset + 1 };
                let to = (r.offset + r.items.len()).min(r.total);
                ui.label(format!("{from}–{to} of {}", r.total));
                if ui
                    .add_enabled(r.offset > 0, egui::Button::new("◀"))
                    .clicked()
                {
                    actions.push(Action::Page(false));
                }
                if ui
                    .add_enabled(r.offset + r.items.len() < r.total, egui::Button::new("▶"))
                    .clicked()
                {
                    actions.push(Action::Page(true));
                }
            }
        });
        if !app.search.status.is_empty() {
            ui.colored_label(BAD, &app.search.status);
        }

        // The install bar sits under the table, always visible.
        let selected = response
            .as_ref()
            .zip(app.search.selected)
            .and_then(|(r, i)| r.items.get(i).cloned());
        egui::TopBottomPanel::bottom("install_bar")
            .frame(egui::Frame::none().inner_margin(egui::Margin::symmetric(0.0, 4.0)))
            .show_inside(ui, |ui| {
                ui.horizontal(|ui| {
                    match &selected {
                        Some(item) => {
                            ui.label(RichText::new(&item.mpn).strong());
                            ui.label(&item.manufacturer_name);
                        }
                        None => {
                            ui.weak("No part selected");
                        }
                    }
                    ui.add(
                        egui::TextEdit::singleline(&mut app.search.revision)
                            .desired_width(70.0)
                            .hint_text("revision"),
                    )
                    .on_hover_text("Silicon revision (optional)");
                    let can = selected.is_some() && !app.installing();
                    if ui
                        .add_enabled(can, egui::Button::new("Install into project"))
                        .on_hover_text(
                            "Symbol, footprint and STEP into the project's openparts libraries",
                        )
                        .clicked()
                    {
                        if let Some(item) = &selected {
                            let rev = app.search.revision.trim();
                            actions.push(Action::Install(
                                vec![(item.manufacturer.clone(), item.mpn.clone())],
                                (!rev.is_empty()).then(|| rev.to_string()),
                                Tab::Search,
                            ));
                        }
                    }
                });
            });

        if let Some(r) = &response {
            results_table(ui, &app.search, r, actions);
        }
    });
    app.search.response = response;
}

fn facets(
    ui: &mut Ui,
    r: &CatalogResponse,
    ranges: &mut std::collections::BTreeMap<String, (String, String)>,
    actions: &mut Vec<Action>,
) {
    let f = &r.facets;
    if !f.category_tree.is_empty() {
        egui::CollapsingHeader::new("Category")
            .default_open(true)
            .show(ui, |ui| category_nodes(ui, &f.category_tree, actions));
    }
    for group in search::facet_groups(r) {
        let any_selected = group.values.iter().any(|v| v.selected);
        egui::CollapsingHeader::new(group.title)
            .default_open(any_selected || group.key == "family" || group.key == "size")
            .show(ui, |ui| {
                for v in group.values {
                    let mut on = v.selected;
                    if ui
                        .checkbox(&mut on, count_label(&v.label, v.count))
                        .changed()
                    {
                        actions.push(Action::Toggle(group.key, v.value.clone()));
                    }
                }
            });
    }

    let mut range_changed = false;
    let mut range_row =
        |ui: &mut Ui, key: &str, title: &str, lo: Option<String>, hi: Option<String>| {
            let entry = ranges.entry(key.to_string()).or_default();
            ui.horizontal(|ui| {
                ui.label(title);
                let a = ui.add(
                    egui::TextEdit::singleline(&mut entry.0)
                        .desired_width(60.0)
                        .hint_text(lo.unwrap_or_else(|| "min".into())),
                );
                ui.label("–");
                let b = ui.add(
                    egui::TextEdit::singleline(&mut entry.1)
                        .desired_width(60.0)
                        .hint_text(hi.unwrap_or_else(|| "max".into())),
                );
                if a.changed() || b.changed() {
                    range_changed = true;
                }
            });
        };

    for p in &f.parameters {
        let any_selected = p.values.iter().any(|v| v.selected);
        egui::CollapsingHeader::new(search::humanize(&p.key))
            .default_open(any_selected)
            .show(ui, |ui| {
                if p.is_numeric() {
                    range_row(ui, &p.key, "", p.min_label.clone(), p.max_label.clone());
                    ui.add_space(2.0);
                }
                for v in &p.values {
                    let mut on = v.selected;
                    if ui
                        .checkbox(&mut on, count_label(&v.label, v.count))
                        .changed()
                    {
                        actions.push(Action::ToggleOwned(p.key.clone(), v.value.clone()));
                    }
                }
            });
    }

    let fmt = |v: Option<f64>| v.map(|v| format!("{v}"));
    if f.body_width.max.is_some() || f.body_height.max.is_some() {
        egui::CollapsingHeader::new("Body (mm)").show(ui, |ui| {
            range_row(
                ui,
                "width",
                "W",
                fmt(f.body_width.min),
                fmt(f.body_width.max),
            );
            range_row(
                ui,
                "length",
                "L",
                fmt(f.body_length.min),
                fmt(f.body_length.max),
            );
            range_row(
                ui,
                "height",
                "H",
                fmt(f.body_height.min),
                fmt(f.body_height.max),
            );
        });
    }
    if range_changed {
        actions.push(Action::Edited);
    }
}

fn category_nodes(ui: &mut Ui, nodes: &[CategoryNode], actions: &mut Vec<Action>) {
    for n in nodes {
        let label = count_label(&n.label, n.count);
        if n.children.is_empty() {
            let mut on = n.selected;
            if ui.checkbox(&mut on, label).changed() {
                actions.push(Action::Toggle("category", n.value.clone()));
            }
        } else {
            let id = ui.make_persistent_id(("cat", &n.value));
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                id,
                search::category_open(n),
            )
            .show_header(ui, |ui| {
                let mut on = n.selected;
                if ui.checkbox(&mut on, label).changed() {
                    actions.push(Action::Toggle("category", n.value.clone()));
                }
            })
            .body(|ui| category_nodes(ui, &n.children, actions));
        }
    }
}

fn results_table(
    ui: &mut Ui,
    state: &search::SearchState,
    r: &CatalogResponse,
    actions: &mut Vec<Action>,
) {
    let cols = search::columns(r, &state.request);
    egui::ScrollArea::both()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            egui::Grid::new("results")
                .striped(true)
                .min_col_width(40.0)
                .show(ui, |ui| {
                    for c in &cols {
                        let title = c.title();
                        match c.sort_key(&r.facets.parameters) {
                            Some(key) => {
                                let arrow = match (&state.request.sort, state.request.descending) {
                                    (Some(s), false) if *s == key => " ▲",
                                    (Some(s), true) if *s == key => " ▼",
                                    _ => "",
                                };
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new(format!("{title}{arrow}")).strong(),
                                        )
                                        .frame(false),
                                    )
                                    .on_hover_text("Sort")
                                    .clicked()
                                {
                                    actions.push(Action::Sort(key));
                                }
                            }
                            None => {
                                ui.label(RichText::new(title).strong());
                            }
                        }
                    }
                    ui.end_row();
                    for (i, item) in r.items.iter().enumerate() {
                        let selected = state.selected == Some(i);
                        for c in &cols {
                            let full = c.cell(item);
                            let short = truncate(&full, 28);
                            let mut text = RichText::new(&short);
                            if *c == Column::Mpn {
                                text = text.strong();
                            }
                            if *c == Column::Lifecycle
                                && matches!(item.lifecycle.as_str(), "eol" | "nrnd" | "obsolete")
                            {
                                text = text.color(WARN);
                            }
                            let mut cell = ui.selectable_label(selected, text);
                            if short != full {
                                cell = cell.on_hover_text(full);
                            }
                            if cell.clicked() {
                                actions.push(Action::Select(i));
                            }
                        }
                        ui.end_row();
                    }
                });
        });
}

// ------------------------------------------------------------------- BOM

fn status_color(status: &str) -> Color32 {
    match status {
        "ok" => OK,
        "review" => WARN,
        _ => BAD,
    }
}

fn bom_tab(app: &mut App, ctx: &egui::Context, actions: &mut Vec<Action>) {
    if let (Some(report), Some(i)) = (&app.bom.report, app.bom.selected) {
        if let Some(line) = report.lines.get(i) {
            let line = line.clone();
            egui::SidePanel::right("bom_detail")
                .resizable(true)
                .default_width(380.0)
                .show(ctx, |ui| {
                    egui::ScrollArea::vertical()
                        .show(ui, |ui| bom_detail(ui, app, i, &line, actions));
                });
        }
    }

    egui::CentralPanel::default().show(ctx, |ui| {
        ui.horizontal(|ui| {
            let busy = app.bom.busy();
            if ui
                .add_enabled(!busy, egui::Button::new("Run BOM check"))
                .clicked()
            {
                app.run_bom();
            }
            egui::ComboBox::from_id_salt("bom_source")
                .selected_text(match app.bom_source {
                    BomSource::Auto => "kicad-cli / schematic",
                    BomSource::Schematic => "schematic",
                })
                .show_ui(ui, |ui| {
                    ui.selectable_value(
                        &mut app.bom_source,
                        BomSource::Auto,
                        "kicad-cli / schematic",
                    )
                    .on_hover_text("kicad-cli's netlist if installed, else the schematic");
                    ui.selectable_value(&mut app.bom_source, BomSource::Schematic, "schematic")
                        .on_hover_text("The .kicad_sch symbol fields");
                });
            ui.checkbox(&mut app.bom.show_ok, "Show OK");
            if busy {
                ui.spinner();
            }
            ui.label(&app.bom.status);
        });
        ui.separator();
        let Some(report) = &app.bom.report else {
            return;
        };
        let visible = app.bom.visible_lines();
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("bom").striped(true).show(ui, |ui| {
                    for h in [
                        "Status",
                        "Refs",
                        "Qty",
                        "Value",
                        "MPN",
                        "Footprint",
                        "Match",
                        "Issues",
                    ] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for i in visible {
                        let l = &report.lines[i];
                        let sel = app.bom.selected == Some(i);
                        let mut click = ui
                            .selectable_label(
                                sel,
                                RichText::new(bom::status_label(&l.status))
                                    .color(status_color(&l.status)),
                            )
                            .clicked();
                        let refs = l.references.join(",");
                        let refs_short = truncate(&refs, 24);
                        click |= ui
                            .selectable_label(sel, refs_short)
                            .on_hover_text(&refs)
                            .clicked();
                        click |= ui.selectable_label(sel, l.quantity.to_string()).clicked();
                        click |= ui.selectable_label(sel, &l.value).clicked();
                        click |= ui.selectable_label(sel, &l.mpn).clicked();
                        click |= ui.selectable_label(sel, &l.footprint).clicked();
                        let m = l
                            .matches
                            .first()
                            .map(|m| format!("{}/{}", m.part.manufacturer, m.part.mpn))
                            .unwrap_or_default();
                        click |= ui.selectable_label(sel, m).clicked();
                        let issues: Vec<String> = l
                            .issues
                            .iter()
                            .filter(|i| i.code != "not_found")
                            .map(|i| search::humanize(&i.code))
                            .collect();
                        click |= ui.selectable_label(sel, issues.join(", ")).clicked();
                        if click {
                            actions.push(Action::BomSelect(i));
                        }
                        ui.end_row();
                    }
                });
            });
    });
}

fn bom_detail(
    ui: &mut Ui,
    app: &App,
    index: usize,
    line: &openparts_client::api::BomLine,
    actions: &mut Vec<Action>,
) {
    ui.heading(line.references.join(", "));
    ui.label(format!(
        "{}  {}",
        if line.mpn.is_empty() {
            &line.value
        } else {
            &line.mpn
        },
        line.footprint
    ));
    if !line.issues.is_empty() {
        ui.add_space(4.0);
        for i in &line.issues {
            let color = if i.severity == "warning" {
                WARN
            } else {
                ui.visuals().text_color()
            };
            let text = match &i.detail {
                Some(d) => format!("{}: {d}", search::humanize(&i.code)),
                None => search::humanize(&i.code),
            };
            ui.colored_label(color, text);
        }
    }
    let fetched = app.bom.fetched.get(&index);
    let candidates = bom::candidates(line, fetched.map_or(&[][..], Vec::as_slice));
    if !line.matches.is_empty() && fetched.is_none() {
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !app.bom.fetching_alternatives(),
                    egui::Button::new("Find alternatives"),
                )
                .clicked()
            {
                actions.push(Action::Alternatives(index));
            }
            if app.bom.fetching_alternatives() {
                ui.spinner();
            }
        });
    }
    if fetched.is_some_and(Vec::is_empty) {
        ui.weak("No alternatives.");
    }
    if candidates.is_empty() {
        return;
    }
    let plan_note = app
        .bom
        .extracted
        .as_ref()
        .map(|e| match bom::mpn_plan(line, &e.symbols) {
            bom::MpnPlan::Write(_) => "Write this MPN into the symbols' MPN field".to_string(),
            bom::MpnPlan::Clipboard(why) => format!("Copy this MPN ({why})"),
        })
        .unwrap_or_default();
    ui.add_space(6.0);
    ui.separator();
    for c in candidates {
        ui.horizontal(|ui| {
            ui.label(RichText::new(&c.part.mpn).strong());
            ui.label(RichText::new(search::humanize(&c.role)).small().weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if c.role != "match" || line.mpn.is_empty() {
                    if ui.button("Use MPN").on_hover_text(&plan_note).clicked() {
                        actions.push(Action::UseMpn(index, Box::new(c.part.clone())));
                    }
                } else if ui.button("Copy").clicked() {
                    actions.push(Action::Copy(c.part.mpn.clone()));
                }
                if ui
                    .add_enabled(!app.installing(), egui::Button::new("Install"))
                    .on_hover_text("Into the project's openparts libraries")
                    .clicked()
                {
                    actions.push(Action::Install(
                        vec![(c.part.manufacturer.clone(), c.part.mpn.clone())],
                        None,
                        Tab::Bom,
                    ));
                }
            });
        });
        let mut sub = c.part.manufacturer_name.clone();
        if !c.part.lifecycle.is_empty() && c.part.lifecycle != "active" {
            sub.push_str(&format!(" · {}", search::humanize(&c.part.lifecycle)));
        }
        ui.label(RichText::new(sub).small());
        let params: Vec<&str> = c.part.parameters.values().map(String::as_str).collect();
        if !params.is_empty() {
            ui.label(RichText::new(params.join(" · ")).small().weak());
        }
        ui.separator();
    }
}

// ------------------------------------------------------------ footprints

fn short_hash(h: Option<&str>) -> String {
    h.map(|h| {
        let h = h.strip_prefix("sha256:").unwrap_or(h);
        h.chars().take(10).collect()
    })
    .unwrap_or_default()
}

fn stale_tab(app: &mut App, ctx: &egui::Context, actions: &mut Vec<Action>) {
    egui::CentralPanel::default().show(ctx, |ui| {
        let updatable = app.stale.updatable();
        ui.horizontal(|ui| {
            let busy = app.stale.busy();
            if ui
                .add_enabled(!busy, egui::Button::new("Check footprints"))
                .on_hover_text("Compare each OpenParts footprint's placement hash with the registry's")
                .clicked()
            {
                app.run_stale();
            }
            ui.checkbox(&mut app.stale.show_all, "Show all");
            let label = format!("Update library footprints ({})", updatable.len());
            if ui
                .add_enabled(
                    !updatable.is_empty() && !app.installing(),
                    egui::Button::new(label),
                )
                .on_hover_text(
                    updatable
                        .iter()
                        .map(|(m, p)| format!("{m}/{p}"))
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .clicked()
            {
                actions.push(Action::Install(updatable.clone(), None, Tab::Stale));
            }
            if busy {
                ui.spinner();
            }
            ui.label(&app.stale.status);
        });
        if app.stale.stale_on_board() {
            ui.label(
                RichText::new(
                    "Board copies: after updating, run Tools › Update Footprints from Library in the PCB editor.",
                )
                .color(WARN),
            );
        }
        ui.separator();
        if app.stale.scanned && app.stale.visible().is_empty() {
            ui.colored_label(OK, "No stale footprints.");
            return;
        }
        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::Grid::new("stale").striped(true).show(ui, |ui| {
                    for h in ["Status", "Where", "Footprint", "Part", "Has", "Current", ""] {
                        ui.label(RichText::new(h).strong());
                    }
                    ui.end_row();
                    for e in app.stale.visible() {
                        let color = match &e.status {
                            StaleStatus::Current => OK,
                            StaleStatus::Stale => WARN,
                            StaleStatus::NoHash => ui.visuals().weak_text_color(),
                            _ => BAD,
                        };
                        let status = ui.colored_label(color, stale::status_label(&e.status));
                        if let StaleStatus::Error(msg) = &e.status {
                            status.on_hover_text(msg);
                        }
                        match &e.footprint.origin {
                            Origin::Board { reference, .. } => ui.label(format!("Board {reference}")),
                            Origin::Library { file, .. } => ui
                                .label("Library")
                                .on_hover_text(file.display().to_string()),
                        };
                        ui.label(&e.footprint.lib_id);
                        ui.label(
                            e.part
                                .as_ref()
                                .map(|(m, p)| format!("{m}/{p}"))
                                .unwrap_or_default(),
                        );
                        ui.monospace(short_hash(e.footprint.hash.as_deref()))
                            .on_hover_text(e.footprint.hash.clone().unwrap_or_default());
                        ui.monospace(short_hash(e.current_hash.as_deref()))
                            .on_hover_text(e.current_hash.clone().unwrap_or_default());
                        if e.is_stale() {
                            match stale::update_target(e) {
                                Ok(part) => {
                                    if ui
                                        .add_enabled(!app.installing(), egui::Button::new("Update"))
                                        .on_hover_text("Reinstall into the openparts library")
                                        .clicked()
                                    {
                                        actions.push(Action::Install(vec![part], None, Tab::Stale));
                                    }
                                }
                                Err(why) => {
                                    ui.add_enabled(false, egui::Button::new("Update"))
                                        .on_disabled_hover_text(why);
                                }
                            }
                        } else {
                            ui.label("");
                        }
                        ui.end_row();
                    }
                });
            });
    });
}
