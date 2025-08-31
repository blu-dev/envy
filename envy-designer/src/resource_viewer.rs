use cosmic_text::fontdb::FaceInfo;
use egui::{Color32, CornerRadius, Stroke, StrokeKind};
use indexmap::IndexMap;

pub struct FontResourceData {
    pub face: FaceInfo,
}

pub struct FontResourceViewer {
    fonts: IndexMap<String, FontResourceData>,
}

pub enum FontViewerCommand {
    Remove(String),
    Replace(String),
    Import,
    Rename { old: String, new: String },
}

impl FontResourceViewer {
    pub fn new() -> Self {
        Self {
            fonts: IndexMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.fonts.clear();
    }

    pub fn rename(&mut self, old: impl AsRef<str>, new: impl Into<String>) {
        let entry = self.fonts.swap_remove_full(old.as_ref());

        if let Some((index, _, data)) = entry {
            self.fonts.shift_insert(index, new.into(), data);
        }
    }

    pub fn remove(&mut self, name: impl AsRef<str>) -> Option<FontResourceData> {
        self.fonts.shift_remove(name.as_ref())
    }

    pub fn add_font(
        &mut self,
        name: impl Into<String>,
        data: FontResourceData,
    ) -> Option<FontResourceData> {
        self.fonts.insert(name.into(), data)
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<FontViewerCommand> {
        let mut command = None;
        egui::ScrollArea::vertical()
            .max_height(70.0 * 5.0)
            .show(ui, |ui| {
                let (_id, rect) = ui.allocate_space(egui::Vec2::new(300.0, 70.0 * 5 as f32));
                ui.painter().rect(
                    ui.max_rect(),
                    CornerRadius::ZERO,
                    Color32::BLACK,
                    Stroke::NONE,
                    StrokeKind::Middle,
                );

                for (idx, (name, data)) in self.fonts.iter().enumerate() {
                    let rect = egui::Rect::from_min_size(
                        rect.min + egui::Vec2::Y * 70.0 * idx as f32,
                        egui::Vec2::new(300.0, 70.0),
                    );
                    let resp = ui.allocate_rect(rect, egui::Sense::all());
                    show_font_row(ui, rect, name, data, &resp, &mut command);

                    let id = ui.id();

                    resp.context_menu(|ui| {
                        if ui.button("Remove").clicked() {
                            command = Some(FontViewerCommand::Remove(name.clone()));
                            ui.close();
                        } else if ui.button("Replace").clicked() {
                            command = Some(FontViewerCommand::Replace(name.clone()));
                            ui.close();
                        } else if ui.button("Rename").clicked() {
                            let id = id.with(name).with("renaming");
                            ui.data_mut(|data| {
                                data.insert_temp(id, name.clone());
                            });
                        }
                    });
                }
            });

        if ui.button("Import Font").clicked() {
            command = Some(FontViewerCommand::Import);
        }
        command
    }
}

pub struct ImageResourceData {
    pub texture_id: egui::TextureId,
    pub size: egui::Vec2,
}

pub struct ImageResourceViewer {
    images: IndexMap<String, ImageResourceData>,
}

pub enum ImageViewerCommand {
    Remove(String),
    Replace(String),
    Import,
    Rename { old: String, new: String },
}

impl ImageResourceViewer {
    pub fn new() -> Self {
        Self {
            images: IndexMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.images.clear();
    }

    pub fn rename(&mut self, old: impl AsRef<str>, new: impl Into<String>) {
        let entry = self.images.swap_remove_full(old.as_ref());

        if let Some((index, _, data)) = entry {
            self.images.shift_insert(index, new.into(), data);
        }
    }

    pub fn remove(&mut self, name: impl AsRef<str>) -> Option<ImageResourceData> {
        self.images.shift_remove(name.as_ref())
    }

    pub fn add_image(
        &mut self,
        name: impl Into<String>,
        data: ImageResourceData,
    ) -> Option<ImageResourceData> {
        self.images.insert(name.into(), data)
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<ImageViewerCommand> {
        let mut command = None;
        egui::ScrollArea::vertical()
            .max_height(70.0 * 5.0)
            .show(ui, |ui| {
                let (_id, rect) = ui.allocate_space(egui::Vec2::new(300.0, 70.0 * 5 as f32));
                ui.painter().rect(
                    ui.max_rect(),
                    CornerRadius::ZERO,
                    Color32::BLACK,
                    Stroke::NONE,
                    StrokeKind::Middle,
                );

                for (idx, (name, data)) in self.images.iter().enumerate() {
                    let rect = egui::Rect::from_min_size(
                        rect.min + egui::Vec2::Y * 70.0 * idx as f32,
                        egui::Vec2::new(300.0, 70.0),
                    );
                    let resp = ui.allocate_rect(rect, egui::Sense::all());
                    show_image_row(ui, rect, name, data, &resp, &mut command);

                    let id = ui.id();

                    resp.context_menu(|ui| {
                        if ui.button("Remove").clicked() {
                            command = Some(ImageViewerCommand::Remove(name.clone()));
                            ui.close();
                        } else if ui.button("Replace").clicked() {
                            command = Some(ImageViewerCommand::Replace(name.clone()));
                            ui.close();
                        } else if ui.button("Rename").clicked() {
                            let id = id.with(name).with("renaming");
                            ui.data_mut(|data| {
                                data.insert_temp(id, name.clone());
                            });
                        }
                    });
                }
            });

        if ui.button("Import Image").clicked() {
            command = Some(ImageViewerCommand::Import);
        }
        command
    }
}

fn show_font_row(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    name: &str,
    data: &FontResourceData,
    resp: &egui::Response,
    command: &mut Option<FontViewerCommand>,
) {
    let tile_color = if resp.hovered() {
        Color32::from_rgb(60, 60, 60)
    } else {
        Color32::from_rgb(30, 30, 30)
    };

    ui.painter()
        .rect_filled(rect.shrink(2.5), CornerRadius::ZERO, tile_color);

    let id = ui.id().with(name).with("renaming");

    if let Some(mut new_name) = ui.data_mut(|data| data.get_temp(id)) {
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                rect.min + egui::Vec2::new(10.0, 10.0),
                egui::Vec2::new(200.0, 16.0),
            )),
            |ui| {
                if ui.text_edit_singleline(&mut new_name).lost_focus() {
                    *command = Some(FontViewerCommand::Rename {
                        old: name.to_string(),
                        new: new_name,
                    });
                    ui.data_mut(|data| {
                        let _ = data.remove_temp::<String>(id);
                    });
                } else {
                    ui.data_mut(|data| data.insert_temp(id, new_name));
                }
            },
        );
    } else {
        ui.painter().text(
            rect.min + egui::Vec2::new(10.0, 10.0),
            egui::Align2::LEFT_TOP,
            name,
            egui::FontId::proportional(16.),
            ui.visuals().text_color(),
        );
    }

    ui.painter().text(
        rect.min + egui::Vec2::new(10.0, 40.0),
        egui::Align2::LEFT_CENTER,
        &data.face.families[0].0,
        egui::FontId::proportional(16.),
        ui.visuals().text_color(),
    );

    ui.painter().text(
        rect.min + egui::Vec2::new(10.0, 60.0),
        egui::Align2::LEFT_BOTTOM,
        format!(
            "Style: {:?} | Weight: {} | Stretch: {:?}",
            data.face.style, data.face.weight.0, data.face.stretch
        ),
        egui::FontId::proportional(10.),
        ui.visuals().text_color(),
    );
}

fn show_image_row(
    ui: &mut egui::Ui,
    rect: egui::Rect,
    name: &str,
    data: &ImageResourceData,
    resp: &egui::Response,
    command: &mut Option<ImageViewerCommand>,
) {
    let tile_color = if resp.hovered() {
        Color32::from_rgb(60, 60, 60)
    } else {
        Color32::from_rgb(30, 30, 30)
    };

    ui.painter()
        .rect_filled(rect.shrink(2.5), CornerRadius::ZERO, tile_color);
    let img_space =
        egui::Rect::from_min_size(rect.min + egui::Vec2::splat(5.0), egui::Vec2::splat(60.0));
    ui.painter()
        .rect_filled(img_space, CornerRadius::ZERO, egui::Color32::WHITE);
    ui.painter().image(
        data.texture_id,
        img_space,
        egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ONE),
        egui::Color32::WHITE,
    );

    let id = ui.id().with(name).with("renaming");

    if let Some(mut new_name) = ui.data_mut(|data| data.get_temp(id)) {
        ui.scope_builder(
            egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                rect.min + egui::Vec2::new(80.0, 15.0),
                egui::Vec2::new(200.0, 16.0),
            )),
            |ui| {
                if ui.text_edit_singleline(&mut new_name).lost_focus() {
                    *command = Some(ImageViewerCommand::Rename {
                        old: name.to_string(),
                        new: new_name,
                    });
                    ui.data_mut(|data| {
                        let _ = data.remove_temp::<String>(id);
                    });
                } else {
                    ui.data_mut(|data| data.insert_temp(id, new_name));
                }
            },
        );
    } else {
        ui.painter().text(
            rect.min + egui::Vec2::new(80.0, 15.0),
            egui::Align2::LEFT_TOP,
            name,
            egui::FontId::proportional(16.),
            ui.visuals().text_color(),
        );
    }

    ui.painter().text(
        rect.min + egui::Vec2::new(80.0, 55.0),
        egui::Align2::LEFT_BOTTOM,
        format!("size: {}x{}", data.size.x, data.size.y),
        egui::FontId::proportional(16.),
        ui.visuals().text_color(),
    );
}
