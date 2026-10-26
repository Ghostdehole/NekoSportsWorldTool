//! Independent, local-only route planning. No account, synthetic activity, or upload integration.
use super::{map::MapState, mobile, theme};
use eframe::egui::{self, Color32, Pos2, Stroke, Vec2};
use route_workbench::{Point, RouteDocument};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::{mpsc, Arc},
};

const HISTORY_LIMIT: usize = 40;
const FILE_LIMIT: u64 = 5 * 1024 * 1024;

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Add,
    Edit,
    Pan,
}

enum Loaded {
    Network(Result<Arc<route_planner::RoadGraph>, String>),
    Document(Result<RouteDocument, String>),
    Planned(u64, Result<Vec<Point>, String>),
}
enum Confirmation {
    Replace(RouteDocument),
    Clear,
    Overwrite(PathBuf, String, RouteDocument),
}

pub struct RouteWorkbench {
    document: RouteDocument,
    saved_document: RouteDocument,
    undo: Vec<RouteDocument>,
    redo: Vec<RouteDocument>,
    revision: u64,
    selected: Option<usize>,
    drag_before: Option<RouteDocument>,
    map: MapState,
    fit: bool,
    mode: Mode,
    lon: String,
    lat: String,
    path: String,
    osm_path: String,
    status: String,
    error: bool,
    network: Option<Arc<route_planner::RoadGraph>>,
    pending: Option<Confirmation>,
    busy: bool,
    tx: mpsc::Sender<Loaded>,
    rx: mpsc::Receiver<Loaded>,
}

impl Default for RouteWorkbench {
    fn default() -> Self {
        let (tx, rx) = mpsc::channel();
        Self {
            document: RouteDocument::default(),
            saved_document: RouteDocument::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            revision: 0,
            selected: None,
            drag_before: None,
            map: MapState::default(),
            fit: false,
            mode: Mode::Add,
            lon: String::new(),
            lat: String::new(),
            path: "my-route.json".into(),
            osm_path: String::new(),
            status: "先输入校园附近的 WGS84 经纬度定位，或导入本地 OSM 路网。".into(),
            error: false,
            network: None,
            pending: None,
            busy: false,
            tx,
            rx,
        }
    }
}

fn read_limited(path: &std::path::Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("无法打开文件：{e}"))?;
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("无法读取文件：{e}"))?;
    if bytes.len() as u64 > limit {
        return Err(format!("文件超过 {} MiB", limit / 1024 / 1024));
    }
    Ok(bytes)
}

fn write_atomic(path: &std::path::Path, text: &str) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let temporary = parent.join(format!(".neko-route-{}-{stamp}.tmp", std::process::id()));
    let result = (|| -> Result<(), std::io::Error> {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(temporary);
    }
    result.map_err(|e| e.to_string())
}

impl RouteWorkbench {
    fn message(&mut self, value: impl Into<String>, error: bool) {
        self.status = value.into();
        self.error = error;
    }

    fn record(&mut self, previous: RouteDocument) {
        if previous == self.document {
            return;
        }
        if self.undo.len() >= HISTORY_LIMIT {
            self.undo.remove(0);
        }
        self.undo.push(previous);
        self.redo.clear();
        self.revision = self.revision.wrapping_add(1);
    }

    fn change(&mut self, edit: impl FnOnce(&mut RouteDocument)) {
        let previous = self.document.clone();
        edit(&mut self.document);
        if let Err(error) = self.document.validate() {
            self.document = previous;
            self.message(error, true);
        } else {
            self.record(previous);
        }
    }

    fn undo(&mut self) {
        if let Some(previous) = self.undo.pop() {
            self.redo
                .push(std::mem::replace(&mut self.document, previous));
            self.selected = None;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn redo(&mut self) {
        if let Some(next) = self.redo.pop() {
            self.undo.push(std::mem::replace(&mut self.document, next));
            self.selected = None;
            self.revision = self.revision.wrapping_add(1);
        }
    }

    fn replace(&mut self, next: RouteDocument) {
        self.saved_document = next.clone();
        self.change(|d| *d = next);
        self.selected = None;
        self.fit = true;
    }

    fn coordinate(&self) -> Result<Point, String> {
        let lon = self
            .lon
            .trim()
            .parse::<f64>()
            .map_err(|_| "经度必须是数字")?;
        let lat = self
            .lat
            .trim()
            .parse::<f64>()
            .map_err(|_| "纬度必须是数字")?;
        let mut doc = RouteDocument::default();
        doc.points = vec![Point { lat, lon }];
        doc.validate()?;
        Ok(Point { lat, lon })
    }

    fn load_network(&mut self, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        let path = PathBuf::from(self.osm_path.trim());
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.busy = true;
        self.message("正在后台解析本地 OSM…", false);
        std::thread::spawn(move || {
            let result = route_workbench::load_local_network(&path).map(Arc::new);
            let _ = tx.send(Loaded::Network(result));
            ctx.request_repaint();
        });
    }

    fn load_document(&mut self, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        let path = PathBuf::from(self.path.trim());
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.busy = true;
        self.message("正在读取路线文件…", false);
        std::thread::spawn(move || {
            let result = read_limited(&path, FILE_LIMIT).and_then(|bytes| {
                let text =
                    String::from_utf8(bytes).map_err(|_| "文件必须是 UTF-8 编码".to_owned())?;
                if text
                    .trim_start_matches('\u{feff}')
                    .trim_start()
                    .starts_with('<')
                {
                    RouteDocument::from_gpx(text.trim_start_matches('\u{feff}'))
                } else {
                    RouteDocument::from_json(text.trim_start_matches('\u{feff}'))
                }
            });
            let _ = tx.send(Loaded::Document(result));
            ctx.request_repaint();
        });
    }

    fn plan(&mut self, ctx: &egui::Context) {
        if self.busy {
            return;
        }
        let Some(network) = self.network.clone() else {
            return;
        };
        let mut points = self.document.points.clone();
        if self.document.closed && points.len() > 1 && points.first() != points.last() {
            points.push(points[0]);
        }
        let revision = self.revision;
        let tx = self.tx.clone();
        let ctx = ctx.clone();
        self.busy = true;
        self.message("正在本地路网中规划途经各点的路线…", false);
        std::thread::spawn(move || {
            let result = route_workbench::plan_on_network(&network, &points);
            let _ = tx.send(Loaded::Planned(revision, result));
            ctx.request_repaint();
        });
    }

    fn poll(&mut self) {
        while let Ok(loaded) = self.rx.try_recv() {
            self.busy = false;
            match loaded {
                Loaded::Network(Ok(network)) => {
                    self.map.center = (network.anchor.lat, network.anchor.lon);
                    self.network = Some(network);
                    self.fit = true;
                    self.message(
                        "本地路网已加载；打点后可按道路规划。道路数据可能不包含校园全部小路。",
                        false,
                    );
                }
                Loaded::Document(Ok(document)) => {
                    if self.document.points.is_empty() {
                        self.replace(document);
                    } else {
                        self.pending = Some(Confirmation::Replace(document));
                    }
                    self.message("文件解析成功。", false);
                }
                Loaded::Planned(revision, Ok(points)) => {
                    if revision != self.revision {
                        self.message("规划期间路线已修改，旧结果已丢弃，请重新规划。", true);
                    } else {
                        self.change(|doc| {
                            doc.points = points;
                            doc.closed = false;
                        });
                        self.selected = None;
                        self.fit = true;
                        self.message("已沿实际相连的道路规划；端点会吸附到 100 米以内的路网节点。可撤销回到原始打点。", false);
                    }
                }
                Loaded::Network(Err(e)) | Loaded::Document(Err(e)) | Loaded::Planned(_, Err(e)) => {
                    self.message(e, true)
                }
            }
        }
    }

    fn write_file(&mut self, path: PathBuf, text: String, snapshot: RouteDocument) {
        match write_atomic(&path, &text) {
            Ok(()) => {
                self.saved_document = snapshot;
                self.message(format!("已保存：{}", path.display()), false);
            }
            Err(e) => self.message(format!("保存失败：{e}"), true),
        }
    }

    fn export(&mut self, gpx: bool) {
        let text = if gpx {
            self.document.to_gpx()
        } else {
            self.document.to_json()
        };
        let text = match text {
            Ok(s) => s,
            Err(e) => {
                self.message(e, true);
                return;
            }
        };
        let extension = if gpx { "gpx" } else { "json" };
        let mut path = PathBuf::from(self.path.trim());
        path.set_extension(extension);
        #[cfg(not(target_os = "android"))]
        {
            let Some(chosen) = rfd::FileDialog::new()
                .add_filter("路线", &[extension])
                .set_file_name(
                    path.file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("my-route.json"),
                )
                .save_file()
            else {
                return;
            };
            path = chosen;
            if path.extension().is_none() {
                path.set_extension(extension);
            }
        }
        self.path = path.display().to_string();
        if path.exists() {
            self.pending = Some(Confirmation::Overwrite(path, text, self.document.clone()));
        } else {
            self.write_file(path, text, self.document.clone());
        }
    }

    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.poll();
        ui.add_enabled_ui(self.pending.is_none(), |ui| {
            egui::ScrollArea::vertical()
                .id_salt("route_workbench_scroll")
                .show(ui, |ui| {
                    ui.heading("自定义路线");
                    ui.label(
                        "不依赖学校点位或登录。绘制计划路线、测距和导出，不生成或提交运动成绩。",
                    );
                    ui.colored_label(
                        theme::text_dim(),
                        "坐标系：WGS84（GPS / OSM）。高德、百度坐标请先转换；不要直接混用。",
                    );
                    ui.horizontal_wrapped(|ui| {
                        ui.label(format!("{} 个点", self.document.points.len()));
                        ui.strong(format!("{:.3} km", self.document.length_m() / 1000.));
                        ui.label(if self.document.closed {
                            "闭合折线"
                        } else {
                            "开放折线"
                        });
                        if self.busy {
                            ui.spinner();
                        }
                    });
                    ui.colored_label(
                        if self.error {
                            theme::err()
                        } else {
                            theme::plain()
                        },
                        &self.status,
                    );
                    self.draw_toolbar(ui);
                    self.draw_coordinates(ui);
                    self.draw_map(ui);
                    self.draw_selection(ui);
                    self.draw_files(ui);
                });
        });
        self.draw_confirmation(ui.ctx());
    }

    fn draw_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.selectable_value(&mut self.mode, Mode::Add, "打点");
            ui.selectable_value(&mut self.mode, Mode::Edit, "选择 / 移动点");
            ui.selectable_value(&mut self.mode, Mode::Pan, "平移");
            if ui.button("适配视野").clicked() {
                self.fit = true;
            }
            if ui.button("＋").clicked() {
                self.map.scale = (self.map.scale * 1.5).min(200.);
            }
            if ui.button("－").clicked() {
                self.map.scale = (self.map.scale / 1.5).max(0.002);
            }
            if ui
                .add_enabled(!self.undo.is_empty(), egui::Button::new("撤销"))
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(!self.redo.is_empty(), egui::Button::new("重做"))
                .clicked()
            {
                self.redo();
            }
        });
        ui.horizontal_wrapped(|ui| {
            let mut closed = self.document.closed;
            if ui.checkbox(&mut closed, "闭合路线").changed() {
                self.change(|d| d.closed = closed);
            }
            if ui
                .add_enabled(self.document.points.len() > 1, egui::Button::new("反向"))
                .clicked()
            {
                self.change(|d| d.points.reverse());
                self.selected = None;
            }
            if ui
                .add_enabled(
                    !self.busy && !self.document.points.is_empty(),
                    egui::Button::new("清空路线"),
                )
                .clicked()
            {
                self.pending = Some(Confirmation::Clear);
            }
            if ui
                .add_enabled(
                    !self.busy
                        && self.network.is_some()
                        && (2..=64).contains(&self.document.points.len()),
                    theme::primary_btn("沿路网规划"),
                )
                .clicked()
            {
                self.plan(ui.ctx());
            }
        });
        ui.small("打点模式：点击添加；编辑模式：选点并拖动；平移模式：拖动画布。滚轮缩放。手绘直线不代表可通行道路。");
    }

    fn draw_coordinates(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_wrapped(|ui| {
            ui.label("经度");
            mobile::text_edit(
                ui,
                "route_lon",
                &mut self.lon,
                crate::platform::InputKind::Text,
                110.,
            );
            ui.label("纬度");
            mobile::text_edit(
                ui,
                "route_lat",
                &mut self.lat,
                crate::platform::InputKind::Text,
                110.,
            );
            if ui.button("定位视野").clicked() {
                match self.coordinate() {
                    Ok(p) => {
                        self.map.center = (p.lat, p.lon);
                        self.map.offset = Vec2::ZERO;
                        self.map.scale = 0.5;
                    }
                    Err(e) => self.message(e, true),
                }
            }
            if ui.button("添加坐标点").clicked() {
                match self.coordinate() {
                    Ok(p) => {
                        self.change(|d| d.points.push(p));
                        self.fit = true;
                    }
                    Err(e) => self.message(e, true),
                }
            }
        });
    }

    fn fit_points(&mut self, rect: egui::Rect, points: &[(f64, f64)]) {
        if points.is_empty() {
            return;
        }
        self.map.fit(rect, points);
        // Existing renderer scales from bounds, but its arithmetic mean center can
        // clip a sparse tail. Use the bounding-box midpoint for this editor only.
        let (mut min_lat, mut max_lat, mut min_lon, mut max_lon) = (
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
        );
        for &(lat, lon) in points {
            min_lat = min_lat.min(lat);
            max_lat = max_lat.max(lat);
            min_lon = min_lon.min(lon);
            max_lon = max_lon.max(lon);
        }
        self.map.center = ((min_lat + max_lat) / 2., (min_lon + max_lon) / 2.);
        self.map.offset = Vec2::ZERO;
    }

    fn draw_map(&mut self, ui: &mut egui::Ui) {
        let size = Vec2::new(
            ui.available_width().max(1.),
            if mobile::compact_ui(ui) { 270. } else { 410. },
        );
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());
        if self.fit {
            let mut points: Vec<_> = self
                .document
                .points
                .iter()
                .map(|p| (p.lat, p.lon))
                .collect();
            if points.is_empty() {
                if let Some(net) = &self.network {
                    points.extend(net.graph.node_weights().map(|n| (n.coord.lat, n.coord.lon)));
                }
            }
            if points.len() == 1 {
                self.map.center = points[0];
                self.map.offset = Vec2::ZERO;
            } else {
                self.fit_points(rect.shrink(24.), &points);
            }
            self.map.scale = self.map.scale.clamp(0.002, 200.);
            self.fit = false;
        }
        if self.mode == Mode::Pan {
            self.map.interact(ui, rect);
        } else if response.hovered() {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            self.map.scale = (self.map.scale * (scroll * 0.0015).exp()).clamp(0.002, 200.);
        }
        let closest = |this: &Self, position: Pos2| {
            this.document
                .points
                .iter()
                .enumerate()
                .map(|(i, p)| (i, this.map.to_screen(rect, p.lat, p.lon).distance(position)))
                .filter(|(_, distance)| *distance < 18.)
                .min_by(|a, b| a.1.total_cmp(&b.1))
                .map(|(i, _)| i)
        };
        if self.mode == Mode::Edit && response.drag_started() {
            if let Some(pos) = ui.input(|i| i.pointer.press_origin()) {
                self.selected = closest(self, pos);
                if self.selected.is_some() {
                    self.drag_before = Some(self.document.clone());
                }
            }
        }
        if self.mode == Mode::Edit && response.dragged() {
            if let (Some(index), Some(pos)) = (self.selected, response.interact_pointer_pos()) {
                let (lat, lon) = self.map.from_screen(rect, pos);
                if lat.abs() <= 90. && lon.abs() <= 180. && index < self.document.points.len() {
                    let point = Point { lat, lon };
                    if self.document.points[index] != point {
                        self.document.points[index] = point;
                        self.revision = self.revision.wrapping_add(1);
                    }
                }
            }
        }
        if response.drag_stopped() || !ui.input(|i| i.pointer.primary_down()) {
            if let Some(before) = self.drag_before.take() {
                self.record(before);
            }
        }
        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                match self.mode {
                    Mode::Add => {
                        let (lat, lon) = self.map.from_screen(rect, pos);
                        self.change(|d| d.points.push(Point { lat, lon }));
                    }
                    Mode::Edit => self.selected = closest(self, pos),
                    Mode::Pan => {}
                }
            }
        }
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6., Color32::from_rgb(231, 240, 236));
        let grid = Stroke::new(1.0_f32, Color32::from_rgb(216, 228, 222));
        for x in (0..size.x as usize).step_by(40) {
            painter.line_segment(
                [
                    rect.left_top() + Vec2::new(x as f32, 0.),
                    rect.left_bottom() + Vec2::new(x as f32, 0.),
                ],
                grid,
            );
        }
        for y in (0..size.y as usize).step_by(40) {
            painter.line_segment(
                [
                    rect.left_top() + Vec2::new(0., y as f32),
                    rect.right_top() + Vec2::new(0., y as f32),
                ],
                grid,
            );
        }
        if let Some(net) = &self.network {
            self.map.draw_network(&painter, rect, net);
        }
        let mut points: Vec<_> = self
            .document
            .points
            .iter()
            .map(|p| (p.lat, p.lon))
            .collect();
        if self.document.closed && points.len() > 1 {
            points.push(points[0]);
        }
        self.map
            .draw_polyline(&painter, rect, &points, theme::accent(), 3.);
        for (index, point) in self.document.points.iter().enumerate() {
            let pos = self.map.to_screen(rect, point.lat, point.lon);
            if !rect.contains(pos) {
                continue;
            }
            let selected = self.selected == Some(index);
            painter.circle_filled(
                pos,
                if selected { 7. } else { 4. },
                if selected {
                    theme::warn()
                } else {
                    theme::accent()
                },
            );
            if self.document.points.len() <= 100
                || selected
                || index == 0
                || index + 1 == self.document.points.len()
            {
                painter.text(
                    pos + Vec2::new(7., -7.),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{}", index + 1),
                    egui::FontId::proportional(12.),
                    theme::text(),
                );
            }
        }
        let hint = if self.network.is_none() {
            "离线坐标画布 · 尚未加载道路底图"
        } else {
            "本地 OSM 路网 · WGS84"
        };
        painter.text(
            rect.left_top() + Vec2::splat(10.),
            egui::Align2::LEFT_TOP,
            hint,
            egui::FontId::proportional(13.),
            theme::text(),
        );
    }

    fn draw_selection(&mut self, ui: &mut egui::Ui) {
        let Some(index) = self.selected.filter(|i| *i < self.document.points.len()) else {
            return;
        };
        let point = self.document.points[index];
        ui.horizontal_wrapped(|ui| {
            ui.label(format!(
                "已选 #{}：{:.6}, {:.6}",
                index + 1,
                point.lon,
                point.lat
            ));
            if ui.button("坐标填入上方").clicked() {
                self.lon = point.lon.to_string();
                self.lat = point.lat.to_string();
            }
            if ui.button("用输入坐标替换").clicked() {
                match self.coordinate() {
                    Ok(p) => self.change(|d| d.points[index] = p),
                    Err(e) => self.message(e, true),
                }
            }
            if ui.button("删除点").clicked() {
                self.change(|d| {
                    d.points.remove(index);
                });
                self.selected = None;
                return;
            }
            if ui
                .add_enabled(index > 0, egui::Button::new("前移"))
                .clicked()
            {
                self.change(|d| d.points.swap(index, index - 1));
                self.selected = Some(index - 1);
            }
            if ui
                .add_enabled(
                    index + 1 < self.document.points.len(),
                    egui::Button::new("后移"),
                )
                .clicked()
            {
                self.change(|d| d.points.swap(index, index + 1));
                self.selected = Some(index + 1);
            }
        });
    }

    fn draw_files(&mut self, ui: &mut egui::Ui) {
        ui.separator();
        ui.horizontal_wrapped(|ui| {
            ui.label("路线名称");
            let before = self.document.clone();
            mobile::text_edit(
                ui,
                "route_name",
                &mut self.document.name,
                crate::platform::InputKind::Text,
                220.,
            );
            if self.document != before {
                if self.document.validate().is_err() {
                    self.document = before;
                } else {
                    self.record(before);
                }
            }
        });
        ui.horizontal_wrapped(|ui| {
            ui.label("路线文件");
            mobile::text_edit(
                ui,
                "route_file",
                &mut self.path,
                crate::platform::InputKind::Text,
                ui.available_width().min(350.).max(100.),
            );
            #[cfg(not(target_os = "android"))]
            if ui
                .add_enabled(!self.busy, egui::Button::new("选择 JSON / GPX"))
                .clicked()
            {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("路线", &["json", "gpx"])
                    .pick_file()
                {
                    self.path = path.display().to_string();
                    self.load_document(ui.ctx());
                }
            }
            if ui
                .add_enabled(
                    !self.busy && !self.path.trim().is_empty(),
                    egui::Button::new("导入路径"),
                )
                .clicked()
            {
                self.load_document(ui.ctx());
            }
        });
        ui.horizontal_wrapped(|ui| {
            if ui
                .add_enabled(!self.busy, egui::Button::new("保存 JSON 草稿"))
                .clicked()
            {
                self.export(false);
            }
            if ui
                .add_enabled(
                    !self.busy && self.document.points.len() >= 2,
                    theme::primary_btn("导出 GPX 路线"),
                )
                .clicked()
            {
                self.export(true);
            }
            if ui.button("复制 JSON").clicked() {
                match self.document.to_json() {
                    Ok(text) => {
                        ui.ctx().copy_text(text);
                        self.message("JSON 已复制。", false);
                    }
                    Err(e) => self.message(e, true),
                }
            }
        });
        ui.small("JSON 保留名称与闭合设置；GPX 导出规划路线，不包含虚构时间、速度或活动成绩。保存前请留意文件路径。");
        egui::CollapsingHeader::new("本地 OSM 路网（可选，用于道路底图和起终点规划）").show(ui, |ui| {
            ui.label("从 OpenStreetMap 导出校园附近的 .osm / .xml 文件，再导入。最大 20 MiB；本模块不自动联网。");
            ui.hyperlink_to("OSM 数据 © OpenStreetMap contributors", "https://www.openstreetmap.org/copyright");
            ui.horizontal_wrapped(|ui| {
                mobile::text_edit(ui, "workbench_osm", &mut self.osm_path, crate::platform::InputKind::Text, ui.available_width().min(350.).max(100.));
                #[cfg(not(target_os = "android"))]
                if ui.add_enabled(!self.busy, egui::Button::new("选择路网")).clicked() {
                    if let Some(path) = rfd::FileDialog::new().add_filter("OSM XML", &["osm", "xml"]).pick_file() { self.osm_path = path.display().to_string(); self.load_network(ui.ctx()); }
                }
                if ui.add_enabled(!self.busy && !self.osm_path.trim().is_empty(), egui::Button::new("加载本地路网")).clicked() { self.load_network(ui.ctx()); }
            });
            if let Some(net) = &self.network { ui.label(format!("{} 节点 · {} 有向边", net.graph.node_count(), net.graph.edge_count())); }
            ui.small("按起点 → 途经点 → 终点打 2–64 个点，选择沿路网规划。没有相连道路时会报错，不会用直线冒充道路。规划结果只供参考，请核对校园通行条件。");
        });
    }

    fn draw_confirmation(&mut self, ctx: &egui::Context) {
        let Some(pending) = &self.pending else {
            return;
        };
        let text = match pending {
            Confirmation::Clear => "清空当前路线？可以通过撤销恢复。".to_owned(),
            Confirmation::Replace(doc) => format!(
                "用导入路线（{} 个点）替换当前路线？可以通过撤销恢复。",
                doc.points.len()
            ),
            Confirmation::Overwrite(path, _, _) => {
                format!("文件已存在，确认覆盖？\n{}", path.display())
            }
        };
        let mut decision = None;
        egui::Window::new("确认操作")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
            .show(ctx, |ui| {
                ui.set_max_width((ctx.screen_rect().width() - 48.).clamp(160., 440.));
                ui.label(text);
                ui.horizontal(|ui| {
                    if ui.button("取消").clicked() {
                        decision = Some(false);
                    }
                    if ui.add(theme::primary_btn("确认")).clicked() {
                        decision = Some(true);
                    }
                });
            });
        if let Some(accept) = decision {
            let pending = self.pending.take().unwrap();
            if accept {
                match pending {
                    Confirmation::Clear => {
                        self.change(|d| {
                            d.points.clear();
                            d.closed = false;
                        });
                        self.selected = None;
                    }
                    Confirmation::Replace(doc) => self.replace(doc),
                    Confirmation::Overwrite(path, text, snapshot) => {
                        self.write_file(path, text, snapshot)
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_is_bounded_and_a_new_edit_discards_redo() {
        let mut page = RouteWorkbench::default();
        for i in 0..60 {
            page.change(|d| {
                d.points.push(Point {
                    lat: 40.,
                    lon: 116. + i as f64 / 10000.,
                })
            });
        }
        assert_eq!(page.undo.len(), HISTORY_LIMIT);
        page.undo();
        assert_eq!(page.document.points.len(), 59);
        page.redo();
        assert_eq!(page.document.points.len(), 60);
        page.undo();
        page.change(|d| d.closed = true);
        assert!(page.redo.is_empty());
    }

    #[test]
    fn bad_coordinate_edit_is_atomic() {
        let mut page = RouteWorkbench::default();
        page.change(|d| {
            d.points.push(Point {
                lat: f64::NAN,
                lon: 116.,
            })
        });
        assert!(page.document.points.is_empty());
        assert!(page.undo.is_empty());
        assert!(page.error);
    }

    #[test]
    fn stale_route_result_does_not_replace_edits() {
        let mut page = RouteWorkbench::default();
        page.change(|d| {
            d.points.push(Point {
                lat: 40.,
                lon: 116.,
            })
        });
        page.tx
            .send(Loaded::Planned(
                0,
                Ok(vec![Point {
                    lat: 41.,
                    lon: 117.,
                }]),
            ))
            .unwrap();
        page.poll();
        assert_eq!(page.document.points[0].lat, 40.);
        assert!(page.error);
    }

    #[test]
    fn coordinate_canvas_round_trips_after_pan_and_zoom() {
        let map = MapState {
            center: (40., 116.),
            scale: 0.7,
            offset: Vec2::new(100., -30.),
            dragged: false,
        };
        let rect = egui::Rect::from_min_size(Pos2::new(10., 20.), Vec2::new(700., 400.));
        let screen = map.to_screen(rect, 40.001, 116.003);
        let (lat, lon) = map.from_screen(rect, screen);
        assert!((lat - 40.001).abs() < 1e-7);
        assert!((lon - 116.003).abs() < 1e-7);
    }

    fn frame(
        page: &mut RouteWorkbench,
        ctx: &egui::Context,
        events: Vec<egui::Event>,
    ) -> egui::FullOutput {
        ctx.run(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    Pos2::ZERO,
                    Vec2::new(1200., 1200.),
                )),
                events,
                ..Default::default()
            },
            |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| page.draw(ui));
            },
        )
    }

    fn pointer(page: &mut RouteWorkbench, ctx: &egui::Context, pos: Pos2, pressed: bool) {
        frame(
            page,
            ctx,
            vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ],
        );
    }

    #[test]
    fn canvas_clicks_add_points_and_pan_does_not_add() {
        let ctx = egui::Context::default();
        let mut page = RouteWorkbench::default();
        let out = frame(&mut page, &ctx, vec![]);
        let canvas = out
            .shapes
            .iter()
            .find_map(|s| match &s.shape {
                egui::Shape::Rect(r) if r.fill == Color32::from_rgb(231, 240, 236) => Some(r.rect),
                _ => None,
            })
            .expect("canvas must be rendered");
        for pos in [canvas.center(), canvas.center() + Vec2::new(80., 30.)] {
            pointer(&mut page, &ctx, pos, true);
            pointer(&mut page, &ctx, pos, false);
        }
        assert_eq!(page.document.points.len(), 2);
        assert!(page.document.length_m() > 10.);
        page.mode = Mode::Pan;
        pointer(&mut page, &ctx, canvas.center(), true);
        frame(
            &mut page,
            &ctx,
            vec![egui::Event::PointerMoved(
                canvas.center() + Vec2::new(50., 50.),
            )],
        );
        pointer(
            &mut page,
            &ctx,
            canvas.center() + Vec2::new(50., 50.),
            false,
        );
        assert_eq!(page.document.points.len(), 2);
        assert!(page.map.offset.length() > 0.);
        page.mode = Mode::Edit;
        let before = page.document.clone();
        let start = page
            .map
            .to_screen(canvas, before.points[0].lat, before.points[0].lon);
        pointer(&mut page, &ctx, start, true);
        frame(
            &mut page,
            &ctx,
            vec![egui::Event::PointerMoved(start + Vec2::new(40., 20.))],
        );
        pointer(&mut page, &ctx, start + Vec2::new(40., 20.), false);
        assert_ne!(page.document.points[0], before.points[0]);
        assert_eq!(page.document.points.len(), 2);
        page.undo();
        assert_eq!(page.document, before);
    }

    #[test]
    fn atomic_save_replaces_complete_file_and_leaves_original_on_failure() {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("neko-route-test-{}-{stamp}", std::process::id()));
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join("route.json");
        std::fs::write(&path, "old").unwrap();
        write_atomic(&path, "new document").unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new document");
        assert!(write_atomic(&directory, "cannot replace a directory").is_err());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new document");
        assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn fit_includes_sparse_tail_of_asymmetric_route() {
        let mut page = RouteWorkbench::default();
        let mut points = vec![(40., 116.); 100];
        points.extend([(40.001, 116.02), (40.01, 116.021)]);
        let rect = egui::Rect::from_min_size(Pos2::ZERO, Vec2::new(700., 400.));
        page.fit_points(rect, &points);
        assert!(points
            .iter()
            .all(|&(lat, lon)| rect.contains(page.map.to_screen(rect, lat, lon))));
    }

    #[test]
    fn page_renders_without_a_session_at_phone_and_desktop_sizes() {
        for width in [320., 1280.] {
            let ctx = egui::Context::default();
            let mut page = RouteWorkbench::default();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        Pos2::ZERO,
                        Vec2::new(width, 720.),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| page.draw(ui));
                },
            );
            assert!(!output.shapes.is_empty());
            assert!(page.network.is_none());
            assert!(!page.busy);
        }
    }
}

/// A separate application shell intentionally has no protocol App/session fields.
pub struct OfflinePlannerApp {
    page: RouteWorkbench,
    confirm_close: bool,
    allow_close: bool,
}
impl OfflinePlannerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        if super::fonts::install(&cc.egui_ctx).is_none() {
            let mut fonts = egui::FontDefinitions::default();
            fonts.font_data.insert(
                "Noto Sans SC".into(),
                egui::FontData::from_static(include_bytes!("../../assets/fonts/NotoSansSC-VF.ttf")),
            );
            fonts
                .families
                .entry(egui::FontFamily::Proportional)
                .or_default()
                .insert(0, "Noto Sans SC".into());
            fonts
                .families
                .entry(egui::FontFamily::Monospace)
                .or_default()
                .push("Noto Sans SC".into());
            cc.egui_ctx.set_fonts(fonts);
        }
        theme::apply(&cc.egui_ctx);
        Self {
            page: RouteWorkbench::default(),
            confirm_close: false,
            allow_close: false,
        }
    }
}
impl eframe::App for OfflinePlannerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.page.busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.page.document != self.page.saved_document
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirm_close = true;
        }
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_enabled_ui(!self.confirm_close, |ui| self.page.draw(ui));
        });
        if self.confirm_close {
            egui::Window::new("路线尚未保存")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0., 0.])
                .show(ctx, |ui| {
                    ui.set_max_width((ctx.screen_rect().width() - 48.).clamp(160., 440.));
                    ui.label("当前路线有未保存的更改。返回编辑器后可保存 JSON 草稿或导出 GPX。");
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("返回编辑器").clicked() {
                            self.confirm_close = false;
                        }
                        if ui.button("放弃更改并退出").clicked() {
                            self.allow_close = true;
                            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                        }
                    });
                });
        }
    }
}
