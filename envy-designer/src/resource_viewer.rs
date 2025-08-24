use std::sync::Arc;

use egui::{CornerRadius, TextureId, epaint::TextureManager, mutex::RwLock};
use image::RgbaImage;
use indexmap::IndexMap;

pub struct ImageResourceData {
    pub texture_id: egui::TextureId,
    pub size: egui::Vec2,
}

pub struct ResourceViewer {
    images: IndexMap<String, ImageResourceData>,
}

impl ResourceViewer {
    pub fn new() -> Self {
        Self {
            images: IndexMap::new(),
        }
    }

    pub fn add_image(&mut self, name: impl Into<String>, data: ImageResourceData) {
        self.images.insert(name.into(), data);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.push_id("resource_viewer", |ui| {
            let (id, rect) =
                ui.allocate_space(egui::Vec2::new(300.0, 70.0 * self.images.len() as f32));
            for (idx, (name, data)) in self.images.iter().enumerate() {
                let rect = egui::Rect::from_min_size(
                    rect.min + egui::Vec2::Y * 70.0 * idx as f32,
                    egui::Vec2::new(300.0, 70.0),
                );
                show_row(ui, id, rect, name, data);
            }
        });
    }
}

fn show_row(
    ui: &mut egui::Ui,
    id: egui::Id,
    rect: egui::Rect,
    name: &str,
    data: &ImageResourceData,
) {
    // let (id, rect) = ui.allocate_space(egui::Vec2::new(300.0, 70.0));

    // let response = ui.interact(rect, id, egui::Sense::all());

    let bg = // if !is_even {
        ui.visuals().faint_bg_color;
    // } else {
    //     ui.visuals().extreme_bg_color
    // };

    ui.painter().rect_filled(rect, CornerRadius::ZERO, bg);
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

    ui.painter().text(
        rect.min + egui::Vec2::new(80.0, 15.0),
        egui::Align2::LEFT_TOP,
        name,
        egui::FontId::proportional(16.),
        ui.visuals().text_color(),
    );

    ui.painter().text(
        rect.min + egui::Vec2::new(80.0, 55.0),
        egui::Align2::LEFT_BOTTOM,
        format!("size: {}x{}", data.size.x, data.size.y),
        egui::FontId::proportional(16.),
        ui.visuals().text_color(),
    );
}
