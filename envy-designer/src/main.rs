use std::{path::PathBuf, sync::Arc};

use bytemuck::{Pod, Zeroable};
use camino::Utf8PathBuf;
use eframe::{App, NativeOptions};
use egui::{epaint::ViewportInPixels, IconData, Rect, ViewportBuilder};
use egui_wgpu::CallbackTrait;
use envy::{ImageNode, ImageNodeTemplate, LayoutRoot, LayoutTemplate, LayoutTree, NodeImplTemplate, NodeTemplate, NodeTransform, SublayoutNodeTemplate, TextNode, TextNodeTemplate};
use envy_wgpu::WgpuBackend;

use crate::{
    resource_viewer::{
        FontResourceData, FontResourceViewer, FontViewerCommand, ImageResourceData,
        ImageResourceViewer, ImageViewerCommand,
    },
    tree_viewer::{ItemTreeCommand, TreeViewerCommand},
};

mod resource_viewer;
mod tree_viewer;

pub struct EnvyDesigner {
    editing_file_path: Option<PathBuf>,
    editing_node_path: Option<Utf8PathBuf>,
    sublayout_editing_paths: Vec<Option<Utf8PathBuf>>,
    currently_viewing: Option<usize>,
    image_resources: ImageResourceViewer,
    font_resources: FontResourceViewer,
}

impl EnvyDesigner {
    pub fn new<'a>(ctx: &'a eframe::CreationContext<'a>) -> Option<Self> {
        egui_extras::install_image_loaders(&ctx.egui_ctx);

        let wgpu_render_state = ctx.wgpu_render_state.as_ref()?;

        let resources = EnvyResources::new(wgpu_render_state);

        let mut egui_renderer = wgpu_render_state.renderer.write();
        let mut viewer = ImageResourceViewer::new();
        update_image_viewer_from_backend(
            &wgpu_render_state.device,
            &resources.backend,
            &mut egui_renderer,
            &mut viewer,
        );

        let mut font_viewer = FontResourceViewer::new();
        update_font_viewer_from_backend(&resources.backend, &mut font_viewer);

        let sublayout_editing_paths = vec![None; resources.sublayouts.len()];

        egui_renderer.callback_resources.insert(resources);

        Some(Self {
            editing_file_path: None,
            editing_node_path: None,
            sublayout_editing_paths,
            currently_viewing: None,
            image_resources: viewer,
            font_resources: font_viewer,
        })
    }
}

fn update_font_viewer_from_backend(backend: &WgpuBackend, font_viewer: &mut FontResourceViewer) {
    font_viewer.clear();
    for font in backend.iter_font_names() {
        let face = backend.get_font_face_info(font).unwrap();
        font_viewer.add_font(font, FontResourceData { face: face.clone() });
    }
}

fn update_image_viewer_from_backend(
    device: &wgpu::Device,
    backend: &WgpuBackend,
    egui_renderer: &mut egui_wgpu::Renderer,
    viewer: &mut ImageResourceViewer,
) {
    viewer.clear();
    for name in backend.iter_texture_names() {
        let texture = backend.get_texture(name).unwrap();
        let id = egui_renderer.register_native_texture(
            device,
            &texture.create_view(&Default::default()),
            wgpu::FilterMode::Linear,
        );
        let size = texture.size();
        viewer.add_image(
            name,
            ImageResourceData {
                texture_id: id,
                size: egui::Vec2::new(size.width as f32, size.height as f32),
            },
        );
    }
}

impl App for EnvyDesigner {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint();
        let mut data = frame
            .wgpu_render_state()
            .unwrap()
            .renderer
            .write()
            .callback_resources
            .remove::<EnvyResources>()
            .unwrap();

        egui::TopBottomPanel::top("file_bar").show(ctx, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New File").clicked() {
                    data.backend.clear();
                    data.root = LayoutRoot::new();
                    data.root.setup(&mut data.backend);
                    data.sublayouts.clear();
                    self.editing_file_path = None;
                    update_font_viewer_from_backend(&data.backend, &mut self.font_resources);
                    let state = frame.wgpu_render_state().unwrap();
                    update_image_viewer_from_backend(
                        &state.device,
                        &data.backend,
                        &mut state.renderer.write(),
                        &mut self.image_resources,
                    );
                    ui.close();
                } else if ui.button("Open File").clicked() {
                    let file = rfd::FileDialog::new()
                        .add_filter("ENVY Layout File", &["envy"])
                        .pick_file();
                    if let Some(file) = file {
                        let bytes = std::fs::read(&file).unwrap();
                        data.backend.clear();
                        data.root = envy::asset::deserialize(&mut data.backend, &bytes);
                        data.root.setup(&mut data.backend);

                        data.sublayouts.clear();
                        for (name, _) in data.root.templates() {
                            let mut tree = data.root.instantiate_tree_from_template(name).unwrap();
                            tree.setup(&mut data.backend);
                            data.sublayouts.push((name.to_string(), tree));
                        }

                        self.sublayout_editing_paths.clear();
                        self.sublayout_editing_paths.resize(data.sublayouts.len(), None);

                        self.editing_file_path = Some(file);
                        update_font_viewer_from_backend(&data.backend, &mut self.font_resources);
                        let state = frame.wgpu_render_state().unwrap();
                        update_image_viewer_from_backend(
                            &state.device,
                            &data.backend,
                            &mut state.renderer.write(),
                            &mut self.image_resources,
                        );
                    }
                    ui.close();
                } else if ui.button("Save File").clicked() {
                    if let Some(current) = self.editing_file_path.as_ref() {
                        let bytes = envy::asset::serialize(&data.root, &data.backend);
                        std::fs::write(current, bytes).unwrap();
                    } else {
                        let file = rfd::FileDialog::new()
                            .add_filter("ENVY Layout File", &["envy"])
                            .save_file();
                        if let Some(file) = file {
                            let bytes = envy::asset::serialize(&data.root, &data.backend);
                            std::fs::write(&file, bytes).unwrap();
                            self.editing_file_path = Some(file);
                        }
                    }
                    ui.close();
                } else if ui.button("Save File As").clicked() {
                    let file = rfd::FileDialog::new()
                        .add_filter("ENVY Layout File", &["envy"])
                        .save_file();
                    if let Some(file) = file {
                        let bytes = envy::asset::serialize(&data.root, &data.backend);
                        std::fs::write(&file, bytes).unwrap();
                        self.editing_file_path = Some(file);
                    }
                    ui.close();
                }
            });
        });

        egui::TopBottomPanel::top("layout-selector").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("root").clicked() {
                    self.currently_viewing = None;
                }

                for (idx, (name, _)) in data.sublayouts.iter_mut().enumerate().filter(|(_, (name, _))| !name.is_empty()) {
                    let id = ui.id().with(idx);
                    if let Some(mut editing) = ui.data_mut(|data| data.remove_temp::<String>(id.with("is_renaming"))) {
                        let resp = ui.text_edit_singleline(&mut editing);
                        if resp.lost_focus() {
                            data.root.rename_template(name.as_str(), &editing);
                            *name = editing;
                        } else {
                            ui.data_mut(|data| data.insert_temp(id.with("is_renaming"), editing));
                        }
                    } else {
                        let resp = ui.button(name.as_str());
                        if resp.clicked() {
                            self.currently_viewing = Some(idx);
                        }

                        resp.context_menu(|ui| {
                            if ui.button("Rename").clicked() {
                                ui.data_mut(|data| data.insert_temp(id.with("is_renaming"), name.clone()));
                            }
                        });
                    }
                }

                if ui.button("+").clicked() {
                    data.root.add_template("new_sublayout", LayoutTemplate::default());
                    data.sublayouts.push(("new_sublayout".to_string(), LayoutTree::new()));
                    self.sublayout_editing_paths.push(None);
                    self.currently_viewing = Some(data.sublayouts.len() - 1);
                }
            });
        });

        egui::SidePanel::left("tree_viewer").show(ctx, |ui| {
            let (name, editing, template) = if let Some(idx) = self.currently_viewing {
                let name = &data.sublayouts[idx].0;
                let template = data.root.template_mut(name).unwrap();
                (Some(name), &mut self.sublayout_editing_paths[idx], template)
            } else {
                (None, &mut self.editing_node_path, data.root.root_template_mut())
            };

            let commands = tree_viewer::show_tree_viewer(ui, &data.icons, template);
            for command in commands {
                match command {
                    ItemTreeCommand::NewItem { parent, new_id: _ } => {
                        let node = template.get_node_by_path_mut(&parent).unwrap();

                        let mut current_child_test = 0;
                        loop {
                            let name = if current_child_test == 0 {
                                "new_node".to_string()
                            } else {
                                format!("new_node_{}", current_child_test + 1)
                            };
                            current_child_test += 1;
                            if node.has_child(&name) {
                                continue;
                            }

                            assert!(node.add_child(NodeTemplate {
                                name,
                                transform: Default::default(),
                                color: [255; 4],
                                children: vec![],
                                implementation: NodeImplTemplate::Empty,
                            }));
                            break;
                        }
                    }
                    ItemTreeCommand::OpenItem(path) => *editing = Some(path),
                    ItemTreeCommand::RenameItem { id, new_name } => {
                        if template.rename_node(&id, new_name.clone()) 
                            && editing.as_ref().is_some_and(|path| *path == id) {
                            *editing = Some(id.with_file_name(new_name));
                        }
                    }
                    ItemTreeCommand::DeleteItem(path) => {
                        assert!(template.remove_node(&path).is_some());
                    }
                    ItemTreeCommand::UserCommand {
                        id,
                        command: TreeViewerCommand::MoveBackward,
                    } => {
                        assert!(template.move_node_backward_by_path(&id));
                    }
                    ItemTreeCommand::UserCommand {
                        id,
                        command: TreeViewerCommand::MoveForward,
                    } => {
                        assert!(template.move_node_forward_by_path(&id));
                    }
                    _ => {}
                }
            }

            if ui.button("Add New Root Node").clicked() {
                let mut current_child_test = 0;
                loop {
                    let name = if current_child_test == 0 {
                        "new_node".to_string()
                    } else {
                        format!("new_node_{}", current_child_test + 1)
                    };
                    current_child_test += 1;

                    if template.has_root(&name) {
                        continue;
                    }

                    template.add_child(NodeTemplate {
                        name,
                        transform: NodeTransform::default(),
                        color: [255; 4],
                        children: vec![],
                        implementation: NodeImplTemplate::Empty
                    });
                    break;
                }
            }

            if let Some(name) = name {
                let idx = self.currently_viewing.unwrap();
                let name = name.clone();
                data.root.sync_template(&name, &mut data.backend);
                data.sublayouts[idx].1.sync_to_template(data.root.template(&name).unwrap(), &data.root, &mut data.backend);
            } else {
                data.root.sync_root_template(&mut data.backend);
            }
        });

        let (path, template_name, template) = if let Some(idx) = self.currently_viewing {
            let path = self.sublayout_editing_paths[idx].as_ref();
            let template_name = &data.sublayouts[idx].0;
            let template = data.root.template_mut(template_name).unwrap();
            (path, Some(template_name), template)
        } else {
            (self.editing_node_path.as_ref(), None, data.root.root_template_mut())
        };

        if let Some(node_path) = path {
            if let Some(node) = template.get_node_by_path_mut(node_path) {
                egui::SidePanel::left("node_editor").show(ctx, |ui| {
                    ui.heading("Transform");
                    ui.separator();

                    egui::Grid::new("transform").show(ui, |ui| {
                        ui.label("Position");
                        egui::Grid::new("position").show(ui, |ui| {
                            ui.add(egui::DragValue::new(&mut node.transform.position.x).speed(1.0));
                            ui.add(egui::DragValue::new(&mut node.transform.position.y).speed(1.0));
                        });
                        ui.end_row();
                        ui.label("Size");
                        egui::Grid::new("Size").show(ui, |ui| {
                            ui.add(egui::DragValue::new(&mut node.transform.size.x).speed(1.0));
                            ui.add(egui::DragValue::new(&mut node.transform.size.y).speed(1.0));
                        });
                        ui.end_row();
                        ui.label("Scale");
                        egui::Grid::new("scale").show(ui, |ui| {
                            ui.add(egui::DragValue::new(&mut node.transform.scale.x).speed(1.0));
                            ui.add(egui::DragValue::new(&mut node.transform.scale.y).speed(1.0));
                        });
                        ui.end_row();
                        ui.label("Rotation");
                        ui.add(egui::DragValue::new(&mut node.transform.angle).speed(1.0));
                        if node.transform.angle < 0.0 {
                            node.transform.angle += -(node.transform.angle / 360.0).floor() * 360.0;
                        }
                        node.transform.angle %= 360.0;
                        ui.end_row();
                        ui.label("Color");
                        ui.color_edit_button_srgba_unmultiplied(&mut node.color);
                        ui.end_row();
                    });

                    ui.heading("Node Implementation");
                    ui.separator();

                    egui::Grid::new("node-kind-selector").show(ui, |ui| {
                        ui.label("Node Kind");
                        let current = match &node.implementation {
                            NodeImplTemplate::Empty => "Empty",
                            NodeImplTemplate::Image(_) => "Image",
                            NodeImplTemplate::Text(_) => "Text",
                            NodeImplTemplate::Sublayout(_) => "Sublayout"
                        };

                        egui::ComboBox::new("selector", "")
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(false, "Empty").clicked() {
                                    if !matches!(&node.implementation, NodeImplTemplate::Empty) {
                                        node.implementation = NodeImplTemplate::Empty;
                                    }
                                    ui.close();
                                } else if ui.selectable_label(false, "Image").clicked() {
                                    if !matches!(&node.implementation, NodeImplTemplate::Image(_)) {
                                        node.implementation = NodeImplTemplate::Image(ImageNodeTemplate {
                                            texture_name: "".to_string(),
                                        });
                                    }
                                    ui.close();
                                } else if ui.selectable_label(false, "Text").clicked() {
                                    if !matches!(&node.implementation, NodeImplTemplate::Text(_)) {
                                        node.implementation = NodeImplTemplate::Text(TextNodeTemplate {
                                            font_name: "".to_string(),
                                            text: "".to_string(),
                                            font_size: 32.0,
                                            line_height: 32.0
                                        });
                                    }
                                    ui.close();
                                } else if ui.selectable_label(false, "Sublayout").clicked() 
                                    && !matches!(&node.implementation, NodeImplTemplate::Sublayout(_)) {
                                    node.implementation = NodeImplTemplate::Sublayout(SublayoutNodeTemplate { sublayout_name: "".to_string() });
                                }
                            })
                    });

                    match &mut node.implementation {
                        NodeImplTemplate::Empty => {},
                        NodeImplTemplate::Image(image) => {
                            ui.label("Texture");
                            egui::ComboBox::new("texture", "")
                                .selected_text(&image.texture_name)
                                .show_ui(ui, |ui| {
                                    for texture in data.backend.iter_texture_names() {
                                        if ui.selectable_label(false, texture).clicked() {
                                            image.texture_name = texture.to_string();
                                            ui.close();
                                            break;
                                        }
                                    }
                                });
                            ui.end_row();
                        },
                        NodeImplTemplate::Text(text) => {
                            egui::Grid::new("font-selector").show(ui, |ui| {
                                ui.label("Font");
                                egui::ComboBox::new("font", "")
                                    .selected_text(&text.font_name)
                                    .show_ui(ui, |ui| {
                                        for font_name in data.backend.iter_font_names() {
                                            if ui.selectable_label(false, font_name).clicked() {
                                                text.font_name = font_name.to_string();
                                                ui.close();
                                                break;
                                            }
                                        }
                                    });
                                ui.end_row();
                                ui.label("Font Size");
                                ui.add(
                                    egui::DragValue::new(&mut text.font_size)
                                        .range(1.0..=f32::INFINITY),
                                );
                                ui.end_row();
                                ui.label("Line Height");
                                ui.add(
                                    egui::DragValue::new(&mut text.line_height)
                                        .range(1.0..=f32::INFINITY),
                                );
                                ui.end_row();
                            });
                            ui.horizontal(|ui| {
                                ui.label("Text");
                                ui.text_edit_multiline(&mut text.text);
                            });
                        }
                        NodeImplTemplate::Sublayout(sublayout) => {
                            ui.label("Sublayout");
                            egui::ComboBox::new("sublayout", "")
                                .selected_text(&sublayout.sublayout_name)
                                .show_ui(ui, |ui| {
                                    for (name, _) in data.sublayouts.iter() {
                                        if !name.is_empty() && ui.selectable_label(false, name).clicked() {
                                            sublayout.sublayout_name = name.to_string();
                                            ui.close();
                                            break;
                                        }
                                    }
                                });
                            ui.end_row();
                        }
                    }
                });

                if let Some(name) = template_name {
                    data.root.sync_template_by_path(name, node_path, &mut data.backend);
                } else {
                    data.root.sync_root_template_by_path(node_path, &mut data.backend);
                }
            }
        }

        let mut image_command = None;
        let mut font_command = None;
        egui::SidePanel::right("resource_viewer").show(ctx, |ui| {
            ui.heading("Resources");
            ui.separator();
            ui.heading("Images");
            image_command = self.image_resources.show(ui);
            ui.separator();
            ui.heading("Fonts");
            font_command = self.font_resources.show(ui);
        });

        match image_command {
            Some(ImageViewerCommand::Remove(image_name)) => {
                if let Some(prev) = self.image_resources.remove(&image_name) {
                    frame
                        .wgpu_render_state()
                        .unwrap()
                        .renderer
                        .write()
                        .free_texture(&prev.texture_id);
                }

                data.root.as_layout_mut().walk_tree_mut(|node| {
                    if let Some(node) = node.downcast_mut::<ImageNode<WgpuBackend>>() {
                        if node.resource_name() == image_name {
                            node.invalidate_image_handle();
                        }
                    }
                });

                for (_, sublayout) in data.sublayouts.iter_mut() {
                    sublayout.walk_tree_mut(|node| {
                        if let Some(image) = node.downcast_mut::<ImageNode<WgpuBackend>>() {
                            if image.resource_name() == image_name {
                                image.invalidate_image_handle();
                            }
                        }
                    })
                }

                data.backend.remove_texture(&image_name);
            }
            Some(ImageViewerCommand::Replace(image_name)) => {
                let new_image_path = rfd::FileDialog::new()
                    .add_filter("PNG Image", &["png"])
                    .pick_file();

                if let Some(path) = new_image_path {
                    let new_texture = data
                        .backend
                        .add_texture(image_name.clone(), &std::fs::read(&path).unwrap());

                    let view = new_texture.create_view(&Default::default());
                    let state = frame.wgpu_render_state().unwrap();
                    let texture_id = state.renderer.write().register_native_texture(
                        &state.device,
                        &view,
                        wgpu::FilterMode::Linear,
                    );

                    if let Some(prev) = self.image_resources.add_image(
                        image_name,
                        ImageResourceData {
                            texture_id,
                            size: egui::Vec2::new(
                                new_texture.size().width as f32,
                                new_texture.size().height as f32,
                            ),
                        },
                    ) {
                        state.renderer.write().free_texture(&prev.texture_id);
                    }
                }
            }
            Some(ImageViewerCommand::Import) => {
                let new_image_path = rfd::FileDialog::new()
                    .add_filter("PNG Image", &["png"])
                    .pick_file();

                if let Some(path) = new_image_path {
                    let image_name = path.file_stem().unwrap().to_str().unwrap().to_string();

                    let new_texture = data
                        .backend
                        .add_texture(image_name.clone(), &std::fs::read(&path).unwrap());

                    let view = new_texture.create_view(&Default::default());
                    let state = frame.wgpu_render_state().unwrap();
                    let texture_id = state.renderer.write().register_native_texture(
                        &state.device,
                        &view,
                        wgpu::FilterMode::Linear,
                    );

                    if let Some(prev) = self.image_resources.add_image(
                        image_name,
                        ImageResourceData {
                            texture_id,
                            size: egui::Vec2::new(
                                new_texture.size().width as f32,
                                new_texture.size().height as f32,
                            ),
                        },
                    ) {
                        state.renderer.write().free_texture(&prev.texture_id);
                    }
                }
            }
            Some(ImageViewerCommand::Rename { old, new }) => {
                data.root.as_layout_mut().walk_tree_mut(|node| {
                    if let Some(image) = node.downcast_mut::<ImageNode<WgpuBackend>>() {
                        if image.resource_name() == old {
                            image.set_resource_name(new.clone());
                        }
                    }
                });

                for (_, sublayout) in data.sublayouts.iter_mut() {
                    sublayout.walk_tree_mut(|node| {
                        if let Some(image) = node.downcast_mut::<ImageNode<WgpuBackend>>() {
                            if image.resource_name() == old {
                                image.set_resource_name(new.clone());
                            }
                        }
                    })
                }

                self.image_resources.rename(&old, new.clone());
                data.backend.rename_texture(&old, new);
            }
            None => {}
        }

        match font_command {
            Some(FontViewerCommand::Remove(font_name)) => {
                let _ = self.font_resources.remove(&font_name);

                data.root.as_layout_mut().walk_tree_mut(|node| {
                    if let Some(node) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                        if node.font_name() == font_name {
                            node.invalidate_font_handle();
                        }
                    }
                });

                for (_, sublayout) in data.sublayouts.iter_mut() {
                    sublayout.walk_tree_mut(|node| {
                        if let Some(node) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                            if node.font_name() == font_name {
                                node.invalidate_font_handle();
                            }
                        }
                    })
                }

                data.backend.remove_font(&font_name);
            }
            Some(FontViewerCommand::Replace(font_name)) => {
                let new_image_path = rfd::FileDialog::new()
                    .add_filter("Font Files", &["ttf", "otf"])
                    .pick_file();

                if let Some(path) = new_image_path {
                    let new_font = data
                        .backend
                        .add_font(font_name.clone(), std::fs::read(&path).unwrap());

                    data.root.as_layout_mut().walk_tree_mut(|node| {
                        if let Some(text) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                            if text.font_name() == font_name {
                                text.invalidate_font_handle();
                            }
                        }
                    });

                    for (_, sublayout) in data.sublayouts.iter_mut() {
                        sublayout.walk_tree_mut(|node| {
                            if let Some(node) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                                if node.font_name() == font_name {
                                    node.invalidate_font_handle();
                                }
                            }
                        })
                    }

                    let _ = self
                        .font_resources
                        .add_font(font_name, FontResourceData { face: new_font });
                }
            }
            Some(FontViewerCommand::Import) => {
                let new_image_path = rfd::FileDialog::new()
                    .add_filter("Font Files", &["ttf", "otf"])
                    .pick_file();

                if let Some(path) = new_image_path {
                    let font_name = path.file_stem().unwrap().to_str().unwrap().to_string();

                    let new_font = data
                        .backend
                        .add_font(font_name.clone(), std::fs::read(&path).unwrap());

                    let _ = self
                        .font_resources
                        .add_font(font_name, FontResourceData { face: new_font });
                }
            }
            Some(FontViewerCommand::Rename { old, new }) => {
                data.root.as_layout_mut().walk_tree_mut(|node| {
                    if let Some(text) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                        if text.font_name() == old {
                            text.set_font_name(new.clone());
                        }
                    }
                });

                for (_, sublayout) in data.sublayouts.iter_mut() {
                    sublayout.walk_tree_mut(|node| {
                        if let Some(node) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                            if node.font_name() == old {
                                node.set_font_name(new.clone());
                            }
                        }
                    })
                }

                self.font_resources.rename(&old, new.clone());
                data.backend.rename_font(&old, new);
            }
            None => {}
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            let template = if let Some(current) = self.currently_viewing {
                let name = &data.sublayouts[current].0;
                data.root.template_mut(name).unwrap()
            } else {
                data.root.root_template_mut()
            };

            egui::Grid::new("canvas size")
                .show(ui, |ui| {
                    ui.label("Canvas Size");
                    egui::Grid::new("vec")
                        .show(ui, |ui| {
                            ui.add(egui::DragValue::new(&mut template.canvas_size[0]));
                            ui.add(egui::DragValue::new(&mut template.canvas_size[1]));
                        });
                });

            frame
                .wgpu_render_state()
                .unwrap()
                .renderer
                .write()
                .callback_resources
                .insert(data);

            egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
                egui::Frame::canvas(ui.style()).show(ui, |ui| {
                    self.custom_painting(ui);
                });
            });
        });
    }
}

impl EnvyDesigner {
    fn custom_painting(&mut self, ui: &mut egui::Ui) {
        let (rect, _response) =
            ui.allocate_exact_size(egui::Vec2::new(1920.0, 1080.0), egui::Sense::drag());

        ui.painter()
            .with_clip_rect(rect)
            .add(egui_wgpu::Callback::new_paint_callback(
                rect,
                CustomPaintCallback {
                    currently_viewing: self.currently_viewing
                },
            ));
    }
}

fn fit_aspect_in_viewport(target_w: f32, target_h: f32, viewport: ViewportInPixels) -> Rect {
    let w_frac = viewport.width_px as f32 / target_w;
    let h_frac = viewport.height_px as f32 / target_h;
    let scale = w_frac.min(h_frac);
    let scaled_w = target_w * scale;
    let scaled_h = target_h * scale;

    let x_offset = (viewport.width_px as f32 - scaled_w) / 2.0;
    let y_offset = (viewport.height_px as f32 - scaled_h) / 2.0;

    Rect::from_min_size(
        egui::Pos2::new(viewport.left_px as f32, viewport.top_px as f32) + egui::Vec2::new(x_offset, y_offset),
        egui::Vec2::new(scaled_w, scaled_h),
    )
}

pub struct CustomPaintCallback {
    currently_viewing: Option<usize>,
}

impl CallbackTrait for CustomPaintCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources: &mut EnvyResources = resources.get_mut().unwrap();
        resources.prepare(self.currently_viewing);

        let mut rpass = egui_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("envy_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &resources.render_target.create_view(&Default::default()),
                resolve_target: Some(
                    &resources
                        .resolved_render_target
                        .create_view(&Default::default()),
                ),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        let layout = if let Some(viewing) = self.currently_viewing {
            &resources.sublayouts[viewing].1
        } else {
            resources.root.as_layout()
        };

        {
            resources.backend.prep_render(&mut rpass);
            layout.render(&resources.backend, &mut rpass);
        }
        drop(rpass);

        Vec::new()
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        resources: &egui_wgpu::CallbackResources,
    ) {
        let rect = fit_aspect_in_viewport(1920.0, 1080.0, info.clip_rect_in_pixels());
        render_pass.set_viewport(
            rect.left(),
            rect.top(),
            rect.width(),
            rect.height(),
            0.0,
            1.0,
        );
        let resources: &EnvyResources = resources.get().unwrap();
        resources.paint(render_pass);
    }
}

#[repr(align(256), C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct ViewUniform {
    view_matrix: glam::Mat4,
    proj_matrix: glam::Mat4,
    padding: [u8; 0x80],
}

#[repr(align(256), C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
struct DrawUniform {
    model_matrix: glam::Mat4,
    base_color: glam::Vec4,
    model_inverse_matrix: glam::Mat4,
    padding: [u8; 0x70],
}

struct Icons {
    texture: egui::ImageSource<'static>,
    empty: egui::ImageSource<'static>,
    text: egui::ImageSource<'static>,
}

impl Icons {
    fn new() -> Self {
        Self {
            texture: egui::ImageSource::Bytes {
                uri: "file://texture_node.svg".into(),
                bytes: egui::load::Bytes::Static(include_bytes!("../../texture_node.svg")),
            },
            empty: egui::ImageSource::Bytes {
                uri: "file://empty_node.svg".into(),
                bytes: egui::load::Bytes::Static(include_bytes!("../../empty_node.svg")),
            },
            text: egui::ImageSource::Bytes {
                uri: "file://text_node.svg".into(),
                bytes: egui::load::Bytes::Static(include_bytes!("../../text_node.svg")),
            },
        }
    }
}

struct EnvyResources {
    backend: WgpuBackend,
    root: envy::LayoutRoot<WgpuBackend>,
    sublayouts: Vec<(String, envy::LayoutTree<WgpuBackend>)>,
    icons: Icons,
    render_target: wgpu::Texture,
    resolved_render_target: wgpu::Texture,
    render_target_bind_group: wgpu::BindGroup,
    copy_texture_pipeline: wgpu::RenderPipeline,
}

impl EnvyResources {
    pub fn new(state: &egui_wgpu::RenderState) -> Self {
        let render_target = state.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("envy_render_target"),
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 4,
            dimension: wgpu::TextureDimension::D2,
            format: state.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let resolved_render_target = state.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("envy_render_target"),
            size: wgpu::Extent3d {
                width: 1920,
                height: 1080,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: state.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let shader_module = state
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("copy_texture_shader"),
                source: wgpu::ShaderSource::Wgsl(include_str!("copy_texture.wgsl").into()),
            });

        let bgl = state
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("envy_copy_texture_bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });

        let layout = state
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("envy_copy_texture_pipeline_layout"),
                bind_group_layouts: &[&bgl],
                push_constant_ranges: &[],
            });

        let pipeline = state
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("envy_copy_texture_pipeline"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader_module,
                    entry_point: None,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader_module,
                    entry_point: None,
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: state.target_format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::all(),
                    })],
                }),
                multiview: None,
                cache: None,
            });

        let bg = state.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("envy_copy_texture_bind_group"),
            layout: &bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Sampler(
                        &state
                            .device
                            .create_sampler(&wgpu::SamplerDescriptor::default()),
                    ),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(
                        &resolved_render_target.create_view(&Default::default()),
                    ),
                },
            ],
        });

        let mut backend = WgpuBackend::new(
            state.device.clone(),
            state.queue.clone(),
            state.target_format,
            4,
        );

        let mut root = LayoutRoot::new();
        root.setup(&mut backend);

        let mut sublayouts = vec![];
        for (name, template) in root.templates() {
            sublayouts.push((name.to_string(), LayoutTree::from_template(template, &root)));
        }

        Self {
            backend,
            root,
            sublayouts,
            icons: Icons::new(),
            render_target,
            resolved_render_target,
            render_target_bind_group: bg,
            copy_texture_pipeline: pipeline,
        }
    }

    fn prepare(&mut self, viewing: Option<usize>) {
        let layout = if let Some(viewing) = viewing {
            &mut self.sublayouts[viewing].1
        } else {
            self.root.as_layout_mut()
        };

        layout.update_animations();
        layout.update();
        layout.propagate();
        layout.prepare(&mut self.backend);
        self.backend.update();
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass) {
        render_pass.set_pipeline(&self.copy_texture_pipeline);
        render_pass.set_bind_group(0, &self.render_target_bind_group, &[]);
        render_pass.draw(0..6, 0..1);
    }
}

static IMAGE_PNG: &[u8] = include_bytes!("../../icon.png");

fn main() {
    env_logger::init();
    let image = image::load(std::io::Cursor::new(IMAGE_PNG), image::ImageFormat::Png)
        .unwrap()
        .to_rgba8();
    eframe::run_native(
        concat!("ENVY Layout Designer ", env!("CARGO_PKG_VERSION")),
        NativeOptions {
            viewport: ViewportBuilder {
                inner_size: Some(egui::Vec2::new(3000.0, 2000.0)),
                icon: Some(Arc::new(IconData {
                    width: image.width(),
                    height: image.height(),
                    rgba: image.to_vec(),
                })),
                app_id: Some("Envy Designer".to_string()),
                ..Default::default()
            },
            ..Default::default()
        },
        Box::new(|ctx| Ok(Box::new(EnvyDesigner::new(ctx).unwrap()))),
    )
    .unwrap();
}
