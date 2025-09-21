use std::{collections::HashMap, sync::Arc};

use egui::Color32;
use envy_wgpu::WgpuBackend;
use parking_lot::Mutex;

pub enum ResourceViewerCommand {
    Add {
        kind: ResourceKind,
    },
    Rename {
        kind: ResourceKind,
        old_name: String,
        new_name: String,
    },
    Replace {
        kind: ResourceKind,
        name: String,
    },
    Remove {
        kind: ResourceKind,
        name: String,
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ResourceKind {
    Images,
    Fonts,
}

struct RenameCtx {
    old: String,
    new: String,
}

#[derive(Debug, Copy, Clone)]
struct ThumbnailInfo {
    texture: egui::TextureId,
    size: egui::Vec2,
}

pub struct ResourceViewer {
    backend: Arc<Mutex<WgpuBackend>>,
    kind: ResourceKind,
    thumbnails: HashMap<String, ThumbnailInfo>,
    rename: Option<RenameCtx>,
}

impl ResourceViewer {
    const TILE_HEIGHT: f32 = 70.0;

    pub fn new(backend: Arc<Mutex<WgpuBackend>>, kind: ResourceKind) -> Self {
        Self {
            backend,
            kind,
            thumbnails: HashMap::new(),
            rename: None,
        }
    }

    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        render_state: &egui_wgpu::RenderState
    ) -> Vec<ResourceViewerCommand> {
        let mut commands = vec![];
        egui::ScrollArea::vertical()
            .show(ui, |ui| {
                let backend = self.backend.lock();

                match self.kind {
                    ResourceKind::Images => {
                        for image_name in backend.iter_texture_names() {
                            let (rect, response) = ui.allocate_exact_size(egui::Vec2::new(ui.available_width(), Self::TILE_HEIGHT), egui::Sense::click());
                            let tile_color = if response.hovered() {
                                Color32::from_rgb(60, 60, 60)
                            } else {
                                Color32::from_rgb(30, 30, 30)
                            };

                            let thumbnail = *self.thumbnails.entry(image_name.to_string())
                                .or_insert_with(|| {
                                    let mut renderer = render_state.renderer.write();
                                    let texture = backend.get_texture(image_name).unwrap();
                                    let id = renderer.register_native_texture(&render_state.device, &texture.create_view(&Default::default()), wgpu::FilterMode::Linear);
                                    let size = texture.size();
                                    ThumbnailInfo { texture: id, size: egui::Vec2::new(size.width as f32, size.height as f32) }
                                });

                            ui.painter()
                                .rect_filled(rect, egui::CornerRadius::ZERO, Color32::BLACK);
                            ui.painter()
                                .rect_filled(rect.shrink(2.5), egui::CornerRadius::ZERO, tile_color);

                            
                            let img_space =
                                egui::Rect::from_min_size(rect.min + egui::Vec2::splat(5.0), egui::Vec2::splat(60.0));
                            ui.painter()
                                .rect_filled(img_space, egui::CornerRadius::ZERO, egui::Color32::WHITE);
                            ui.painter().image(
                                thumbnail.texture,
                                img_space,
                                egui::Rect::from_min_size(egui::Pos2::ZERO, egui::Vec2::ONE),
                                egui::Color32::WHITE,
                            );

                            if let Some(rename) = self.rename.as_mut().filter(|ctx| ctx.old == image_name) {
                                let width = (ui.available_width() - 20.0).max(0.0);
                                let is_done = ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                                    rect.min + egui::Vec2::new(80., 15.),
                                    egui::Vec2::new(width, 16.0),
                                )), |ui| {
                                    ui.text_edit_singleline(&mut rename.new).lost_focus()
                                });

                                if is_done.inner {
                                    commands.push(ResourceViewerCommand::Rename {
                                        kind: self.kind,
                                        old_name: rename.old.clone(),
                                        new_name: rename.new.clone()
                                    });
                                    self.rename = None;
                                }

                                ui.advance_cursor_after_rect(rect);
                            } else {
                                ui.painter().text(
                                    rect.min + egui::Vec2::new(80.0, 15.0),
                                    egui::Align2::LEFT_TOP,
                                    image_name,
                                    egui::FontId::proportional(16.),
                                    ui.visuals().text_color(),
                                );
                            }

                            ui.painter().text(
                                rect.min + egui::Vec2::new(80.0, 55.0),
                                egui::Align2::LEFT_BOTTOM,
                                format!("size: {}x{}", thumbnail.size.x, thumbnail.size.y),
                                egui::FontId::proportional(16.),
                                ui.visuals().text_color(),
                            );

                            response.context_menu(|ui| {
                                if ui.button("Remove").clicked() {
                                    let _ = self.thumbnails.remove(image_name);
                                    commands.push(ResourceViewerCommand::Remove {
                                        kind: self.kind,
                                        name: image_name.to_string()
                                    });
                                    ui.close();
                                } else if ui.button("Replace").clicked() {
                                    let _ = self.thumbnails.remove(image_name);
                                    commands.push(ResourceViewerCommand::Replace { kind: self.kind, name: image_name.to_string() });
                                    ui.close();
                                } else if ui.button("Rename").clicked() {
                                    let _ = self.thumbnails.remove(image_name);
                                    self.rename = Some(RenameCtx {
                                        old: image_name.to_string(),
                                        new: image_name.to_string()
                                    });
                                    ui.close();
                                }
                            });
                        }

                        if ui.button("Import New Image").clicked() {
                            commands.push(ResourceViewerCommand::Add { kind: self.kind });
                        }
                    },
                    ResourceKind::Fonts =>  {
                        for font_name in backend.iter_font_names() {
                            let (_id, rect) = ui.allocate_space(egui::Vec2::new(ui.available_width(), Self::TILE_HEIGHT));
                            let response = ui.allocate_rect(rect, egui::Sense::click());
                            let tile_color = if response.hovered() {
                                Color32::from_rgb(60, 60, 60)
                            } else {
                                Color32::from_rgb(30, 30, 30)
                            };

                            ui.painter()
                                .rect_filled(rect, egui::CornerRadius::ZERO, Color32::BLACK);
                            ui.painter()
                                .rect_filled(rect.shrink(2.5), egui::CornerRadius::ZERO, tile_color);

                            let font = backend.get_font_face_info(font_name).unwrap();

                            if let Some(rename) = self.rename.as_mut().filter(|ctx| ctx.old == font_name) {
                                let width = (ui.available_width() - 20.0).max(0.0);
                                let is_done = ui.scope_builder(egui::UiBuilder::new().max_rect(egui::Rect::from_min_size(
                                    rect.min + egui::Vec2::new(10., 10.),
                                    egui::Vec2::new(width, 16.0),
                                )), |ui| {
                                    ui.text_edit_singleline(&mut rename.new).lost_focus()
                                });

                                if is_done.inner {
                                    commands.push(ResourceViewerCommand::Rename {
                                        kind: self.kind,
                                        old_name: rename.old.clone(),
                                        new_name: rename.new.clone()
                                    });
                                    self.rename = None;
                                }

                                ui.advance_cursor_after_rect(rect);
                            } else {
                                ui.painter().text(
                                    rect.min + egui::Vec2::new(10., 10.),
                                    egui::Align2::LEFT_TOP,
                                    font_name,
                                    egui::FontId::proportional(16.),
                                    ui.visuals().text_color()
                                );
                            }

                            ui.painter().text(
                                rect.min + egui::Vec2::new(10.0, 40.0),
                                egui::Align2::LEFT_CENTER,
                                &font.families[0].0,
                                egui::FontId::proportional(16.0),
                                ui.visuals().text_color(),
                            );

                            ui.painter().text(
                                rect.min + egui::Vec2::new(10.0, 60.0),
                                egui::Align2::LEFT_BOTTOM,
                                format!(
                                    "Style: {:?} | Weight: {} | Stretch: {:?}",
                                    font.style, font.weight.0, font.stretch
                                ),
                                egui::FontId::proportional(10.),
                                ui.visuals().text_color()
                            );

                            response.context_menu(|ui| {
                                if ui.button("Remove").clicked() {
                                    commands.push(ResourceViewerCommand::Remove {
                                        kind: self.kind,
                                        name: font_name.to_string()
                                    });
                                    ui.close();
                                } else if ui.button("Replace").clicked() {
                                    commands.push(ResourceViewerCommand::Replace { kind: self.kind, name: font_name.to_string() });
                                    ui.close();
                                } else if ui.button("Rename").clicked() {
                                    self.rename = Some(RenameCtx {
                                        old: font_name.to_string(),
                                        new: font_name.to_string()
                                    });
                                    ui.close();
                                }
                            });
                        }

                        if ui.button("Import New Font").clicked() {
                            commands.push(ResourceViewerCommand::Add { kind: self.kind });
                        }
                    },
                }
            });

        commands
    }

    pub fn kind(&self) -> ResourceKind {
        self.kind

    }
}
