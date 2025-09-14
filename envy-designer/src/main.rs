use std::{path::PathBuf, sync::Arc};

use bytemuck::{Pod, Zeroable};
use camino::Utf8PathBuf;
use eframe::{App, NativeOptions};
use egui::{IconData, Rect, ViewportBuilder};
use egui_wgpu::CallbackTrait;
use envy::{EmptyNode, ImageNode, LayoutRoot, LayoutTree, Node, NodeItem, NodeTransform, SublayoutNode, TextNode};
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

        egui_renderer.callback_resources.insert(resources);

        Some(Self {
            editing_file_path: None,
            editing_node_path: None,
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
                    data.tree = LayoutRoot::new();
                    data.tree.setup(&mut data.backend);
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
                        data.tree = envy::asset::deserialize(&mut data.backend, &bytes);
                        data.tree.setup(&mut data.backend);
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
                        let bytes = envy::asset::serialize(&data.tree, &data.backend);
                        std::fs::write(current, bytes).unwrap();
                    } else {
                        let file = rfd::FileDialog::new()
                            .add_filter("ENVY Layout File", &["envy"])
                            .save_file();
                        if let Some(file) = file {
                            let bytes = envy::asset::serialize(&data.tree, &data.backend);
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
                        let bytes = envy::asset::serialize(&data.tree, &data.backend);
                        std::fs::write(&file, bytes).unwrap();
                        self.editing_file_path = Some(file);
                    }
                    ui.close();
                }
            });
        });

        egui::SidePanel::left("tree_viewer").show(ctx, |ui| {
            let commands = tree_viewer::show_tree_viewer(ui, &data.icons, data.tree.as_layout());
            for command in commands {
                match command {
                    ItemTreeCommand::NewItem { parent, new_id: _ } => {
                        let node = data.tree.as_layout_mut().get_node_by_path_mut(&parent).unwrap();

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

                            assert!(node.add_child(NodeItem::new(
                                name,
                                NodeTransform::default(),
                                [255; 4],
                                EmptyNode,
                            )));
                            break;
                        }
                    }
                    ItemTreeCommand::OpenItem(path) => self.editing_node_path = Some(path),
                    ItemTreeCommand::RenameItem { id, new_name } => {
                        if data.tree.as_layout_mut().rename_node(&id, new_name.clone()) {
                            if self
                                .editing_node_path
                                .as_ref()
                                .is_some_and(|path| *path == id)
                            {
                                self.editing_node_path = Some(id.with_file_name(new_name));
                            }
                        }
                    }
                    ItemTreeCommand::DeleteItem(path) => {
                        assert!(data.tree.as_layout_mut().remove_node(&path).is_some());
                    }
                    ItemTreeCommand::UserCommand {
                        id,
                        command: TreeViewerCommand::MoveBackward,
                    } => {
                        assert!(data.tree.as_layout_mut().move_node_backward_by_path(&id));
                    }
                    ItemTreeCommand::UserCommand {
                        id,
                        command: TreeViewerCommand::MoveForward,
                    } => {
                        assert!(data.tree.as_layout_mut().move_node_forward_by_path(&id));
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

                    if data.tree.as_layout().has_root(&name) {
                        continue;
                    }

                    data.tree.as_layout_mut().add_child(NodeItem::new(
                        name,
                        NodeTransform::default(),
                        [255; 4],
                        EmptyNode,
                    ));
                    break;
                }
            }
        });

        if let Some(node) = self.editing_node_path.as_ref() {
            if let Some(node) = data.tree.as_layout_mut().get_node_by_path_mut(node) {
                egui::SidePanel::left("node_editor").show(ctx, |ui| {
                    ui.heading("Transform");
                    ui.separator();

                    egui::Grid::new("transform").show(ui, |ui| {
                        let transform = node.transform_mut();
                        ui.label("Position");
                        egui::Grid::new("position").show(ui, |ui| {
                            ui.add(egui::DragValue::new(&mut transform.position.x).speed(1.0));
                            ui.add(egui::DragValue::new(&mut transform.position.y).speed(1.0));
                        });
                        ui.end_row();
                        ui.label("Size");
                        egui::Grid::new("Size").show(ui, |ui| {
                            ui.add(egui::DragValue::new(&mut transform.size.x).speed(1.0));
                            ui.add(egui::DragValue::new(&mut transform.size.y).speed(1.0));
                        });
                        ui.end_row();
                        ui.label("Scale");
                        egui::Grid::new("scale").show(ui, |ui| {
                            ui.add(egui::DragValue::new(&mut transform.scale.x).speed(1.0));
                            ui.add(egui::DragValue::new(&mut transform.scale.y).speed(1.0));
                        });
                        ui.end_row();
                        ui.label("Rotation");
                        ui.add(egui::DragValue::new(&mut transform.angle).speed(1.0));
                        if transform.angle < 0.0 {
                            transform.angle += -(transform.angle / 360.0).floor() * 360.0;
                        }
                        transform.angle = transform.angle % 360.0;
                        ui.end_row();
                    });

                    ui.heading("Node Implementation");
                    ui.separator();

                    egui::Grid::new("node-kind-selector").show(ui, |ui| {
                        ui.label("Node Kind");
                        egui::ComboBox::new("selector", "")
                            .selected_text(if node.is::<EmptyNode>() {
                                "Empty"
                            } else if node.is::<ImageNode<WgpuBackend>>() {
                                "Image"
                            } else if node.is::<TextNode<WgpuBackend>>() {
                                "Text"
                            } else if node.is::<SublayoutNode<WgpuBackend>>() {
                                "Sublayout"
                            } else {
                                unimplemented!()
                            })
                            .show_ui(ui, |ui| {
                                if ui.selectable_label(false, "Empty").clicked() {
                                    if !node.is::<EmptyNode>() {
                                        node.set_implementation(EmptyNode);
                                    }
                                    ui.close();
                                } else if ui.selectable_label(false, "Image").clicked() {
                                    if !node.is::<ImageNode<WgpuBackend>>() {
                                        let mut image = ImageNode::new("");
                                        image.setup_resources(&mut data.backend);
                                        node.set_implementation(image);
                                    }
                                    ui.close();
                                } else if ui.selectable_label(false, "Text").clicked() {
                                    if !node.is::<TextNode<WgpuBackend>>() {
                                        let mut text = TextNode::new("", 32.0, 32.0, "");
                                        text.setup_resources(&mut data.backend);
                                        node.set_implementation(text);
                                    }
                                    ui.close();
                                } else if ui.selectable_label(false, "Sublayout").clicked() {
                                    if !node.is::<SublayoutNode<WgpuBackend>>() {
                                        let mut sublayout = SublayoutNode::new("", LayoutTree::new());
                                        sublayout.setup_resources(&mut data.backend);
                                        node.set_implementation(sublayout);
                                    }
                                }
                            })
                    });

                    if let Some(image) = node.downcast_mut::<ImageNode<WgpuBackend>>() {
                        egui::Grid::new("image-selector").show(ui, |ui| {
                            ui.label("Texture");
                            egui::ComboBox::new("texture", "")
                                .selected_text(image.resource_name())
                                .show_ui(ui, |ui| {
                                    for texture in data.backend.iter_texture_names() {
                                        if ui.selectable_label(false, texture).clicked() {
                                            image.set_resource_name(texture);
                                            ui.close();
                                            break;
                                        }
                                    }
                                });
                            ui.end_row();
                        });
                    } else if let Some(text) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                        egui::Grid::new("font-selector").show(ui, |ui| {
                            ui.label("Font");
                            egui::ComboBox::new("font", "")
                                .selected_text(text.font_name())
                                .show_ui(ui, |ui| {
                                    for font_name in data.backend.iter_font_names() {
                                        if ui.selectable_label(false, font_name).clicked() {
                                            text.set_font_name(font_name);
                                            ui.close();
                                            break;
                                        }
                                    }
                                });
                            ui.end_row();
                            let mut font_size = text.font_size();
                            let mut line_height = text.line_height();
                            ui.label("Font Size");
                            ui.add(
                                egui::DragValue::new(&mut font_size)
                                    .range(1.0..=std::f32::INFINITY),
                            );
                            ui.end_row();
                            ui.label("Line Height");
                            ui.add(
                                egui::DragValue::new(&mut line_height)
                                    .range(1.0..=std::f32::INFINITY),
                            );
                            ui.end_row();
                            text.set_font_size(font_size);
                            text.set_line_height(line_height);
                        });
                        ui.horizontal(|ui| {
                            ui.label("Text");
                            ui.text_edit_multiline(text.text_mut());
                        });
                    }
                });
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

                data.tree.as_layout_mut().walk_tree_mut(|node| {
                    if let Some(node) = node.downcast_mut::<ImageNode<WgpuBackend>>() {
                        if node.resource_name() == image_name {
                            node.invalidate_image_handle();
                        }
                    }
                });
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
                data.tree.as_layout_mut().walk_tree_mut(|node| {
                    if let Some(image) = node.downcast_mut::<ImageNode<WgpuBackend>>() {
                        if image.resource_name() == old {
                            image.set_resource_name(new.clone());
                        }
                    }
                });

                self.image_resources.rename(&old, new.clone());
                data.backend.rename_texture(&old, new);
            }
            None => {}
        }

        match font_command {
            Some(FontViewerCommand::Remove(font_name)) => {
                let _ = self.font_resources.remove(&font_name);

                data.tree.as_layout_mut().walk_tree_mut(|node| {
                    if let Some(node) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                        if node.font_name() == font_name {
                            node.invalidate_font_handle();
                        }
                    }
                });
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

                    data.tree.as_layout_mut().walk_tree_mut(|node| {
                        if let Some(text) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                            if text.font_name() == font_name {
                                text.invalidate_font_handle();
                            }
                        }
                    });

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
                data.tree.as_layout_mut().walk_tree_mut(|node| {
                    if let Some(text) = node.downcast_mut::<TextNode<WgpuBackend>>() {
                        if text.font_name() == old {
                            text.set_font_name(new.clone());
                        }
                    }
                });

                self.font_resources.rename(&old, new.clone());
                data.backend.rename_font(&old, new);
            }
            None => {}
        }

        frame
            .wgpu_render_state()
            .unwrap()
            .renderer
            .write()
            .callback_resources
            .insert(data);

        egui::CentralPanel::default().show(ctx, |ui| {
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
                CustomPaintCallback {},
            ));
    }
}

fn fit_aspect_in_viewport(target_w: f32, target_h: f32, rect: Rect) -> Rect {
    let w_frac = rect.width() / target_w;
    let h_frac = rect.height() / target_h;
    let scale = w_frac.min(h_frac);
    let scaled_w = target_w * scale;
    let scaled_h = target_h * scale;

    let x_offset = (rect.width() - scaled_w) / 2.0;
    let y_offset = (rect.height() - scaled_h) / 2.0;

    Rect::from_min_size(
        rect.min + egui::Vec2::new(x_offset, y_offset),
        egui::Vec2::new(scaled_w, scaled_h),
    )
}

pub struct CustomPaintCallback {}

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
        resources.prepare();

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

        {
            resources.backend.prep_render(&mut rpass);
            resources.tree.render(&resources.backend, &mut rpass);
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
        let rect = fit_aspect_in_viewport(1920.0, 1080.0, info.clip_rect);
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
    tree: envy::LayoutRoot<WgpuBackend>,
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

        let mut tree = LayoutRoot::new();
        tree.setup(&mut backend);

        Self {
            backend,
            tree,
            icons: Icons::new(),
            render_target,
            resolved_render_target,
            render_target_bind_group: bg,
            copy_texture_pipeline: pipeline,
        }
    }

    fn prepare(&mut self) {
        self.tree.as_layout_mut().update_animations();
        self.tree.update();
        self.tree.as_layout_mut().propagate();
        self.tree.prepare(&mut self.backend);
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
