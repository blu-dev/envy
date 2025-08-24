use std::{borrow::Cow, sync::Arc};

use bytemuck::{Pod, Zeroable};
use camino::{Utf8Path, Utf8PathBuf};
use eframe::{App, NativeOptions};
use egui::{IconData, Rect, ViewportBuilder};
use egui_wgpu::CallbackTrait;
use envy::{
    Animation, AnimationChannel, AnimationTransform, EmptyNode, ImageNode, NodeAnimation,
    NodeDisjointAccessor, NodeItem, NodeTransform, TransformStep,
};
use envy_wgpu::WgpuBackend;
use glam::Vec2;

use crate::{
    resource_viewer::{ImageResourceData, ResourceViewer},
    tree_viewer::ItemTreeCommand,
};

// use crate::{
//     tree::{Anchor, EmptyNode, Node, TextNode, TextureNode},
//     tree_viewer::ItemTreeCommand,
// wgpu_backend::WgpuBackend,
// };

// mod file;
// mod tree;
mod resource_viewer;
mod tree_viewer;
// mod wgpu_backend;

pub struct EnvyDesigner {
    editing_node_path: Option<Utf8PathBuf>,
    resources: ResourceViewer,
}

impl EnvyDesigner {
    pub fn new<'a>(ctx: &'a eframe::CreationContext<'a>) -> Option<Self> {
        egui_extras::install_image_loaders(&ctx.egui_ctx);

        let wgpu_render_state = ctx.wgpu_render_state.as_ref()?;

        let mut resources = EnvyResources::new(wgpu_render_state);
        let image_data = std::fs::read("./icon.png").unwrap();
        let texture = resources.backend.add_texture("icon", &image_data);
        let image_data =
            std::fs::read("/home/blujay/.config/Ryujinx/sdcard/stratus_header.png").unwrap();
        let texture_2 = resources.backend.add_texture("header", &image_data);

        let view = texture.create_view(&Default::default());
        let view_2 = texture_2.create_view(&Default::default());

        let texture_id = wgpu_render_state.renderer.write().register_native_texture(
            &wgpu_render_state.device,
            &view,
            wgpu::FilterMode::Linear,
        );
        let texture_id_2 = wgpu_render_state.renderer.write().register_native_texture(
            &wgpu_render_state.device,
            &view_2,
            wgpu::FilterMode::Linear,
        );

        let mut viewer = ResourceViewer::new();
        viewer.add_image(
            "icon",
            ImageResourceData {
                texture_id,
                size: egui::Vec2::new(texture.width() as f32, texture.height() as f32),
            },
        );
        viewer.add_image(
            "header",
            ImageResourceData {
                texture_id: texture_id_2,
                size: egui::Vec2::new(texture_2.width() as f32, texture_2.height() as f32),
            },
        );

        wgpu_render_state
            .renderer
            .write()
            .callback_resources
            .insert(resources);

        Some(Self {
            editing_node_path: None,
            resources: viewer,
        })
    }
}

impl App for EnvyDesigner {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint();
        // egui::TopBottomPanel::top("file_bar").show(ctx, |ui| {
        //     if ui.button("Save File").clicked() {
        //         let state = frame.wgpu_render_state().unwrap().renderer.read();
        //         let res = &state.callback_resources.get::<EnvyResources>().unwrap();
        //         let mut file = file::MenuFile::from_tree(&res.tree);
        //         file.image_resources = res.backend.dump_textures();
        //         file.font_resources = res.backend.dump_fonts();

        //         file.save("/home/blujay/.config/Ryujinx/sdcard/stratus.envy");
        //     }
        // });

        egui::SidePanel::left("tree_viewer").show(ctx, |ui| {
            let mut data = frame
                .wgpu_render_state()
                .unwrap()
                .renderer
                .write()
                .callback_resources
                .remove::<EnvyResources>()
                .unwrap();

            let commands = tree_viewer::show_tree_viewer(ui, &data.icons, &data.tree);
            for command in commands {
                match command {
                    ItemTreeCommand::NewItem { parent, new_id: _ } => {
                        let node = data.tree.get_node_by_path_mut(&parent).unwrap();

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
                        if data.tree.rename_node(&id, new_name.clone()) {
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
                        assert!(data.tree.remove_node(&path).is_some());
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

                    if data.tree.has_root(&name) {
                        continue;
                    }

                    data.tree.add_child(NodeItem::new(
                        name,
                        NodeTransform::default(),
                        [255; 4],
                        EmptyNode,
                    ));
                    break;
                }
            }

            frame
                .wgpu_render_state()
                .unwrap()
                .renderer
                .write()
                .callback_resources
                .insert(data);
        });

        egui::SidePanel::right("test").show(ctx, |ui| {
            self.resources.show(ui);
            // ui.image(egui::ImageSource::Texture(egui::load::SizedTexture {
            //     id: self.texture_id,
            //     size: egui::Vec2::splat(40.0),
            // }));
        });

        // if let Some(editing_node_path) = self.editing_node_path.as_ref() {
        //     egui::SidePanel::left("node_viewer").show(ctx, |ui| {
        //         let mut data = frame
        //             .wgpu_render_state()
        //             .unwrap()
        //             .renderer
        //             .write()
        //             .callback_resources
        //             .remove::<EnvyResources>()
        //             .unwrap();

        //         if let Some(node) = data.tree.get_node_by_path_mut(editing_node_path) {
        //             ui.heading("Node Settings");
        //             ui.separator();

        //             ui.horizontal(|ui| {
        //                 ui.label("Node Position");
        //                 ui.add(egui::DragValue::new(&mut node.settings.position.x));
        //                 ui.add(egui::DragValue::new(&mut node.settings.position.y));
        //             });

        //             ui.horizontal(|ui| {
        //                 ui.label("Node Size");
        //                 ui.add(egui::DragValue::new(&mut node.settings.size.x));
        //                 ui.add(egui::DragValue::new(&mut node.settings.size.y));
        //             });

        //             ui.horizontal(|ui| {
        //                 const ANCHORS: &[&str] = &[
        //                     "Top Left",
        //                     "Top Center",
        //                     "Top Right",
        //                     "Center Left",
        //                     "Center",
        //                     "Center Right",
        //                     "Bottom Left",
        //                     "Bottom Center",
        //                     "Bottom Right",
        //                     "Custom",
        //                 ];

        //                 let idx = match node.settings.anchor {
        //                     Anchor::TopLeft => 0,
        //                     Anchor::TopCenter => 1,
        //                     Anchor::TopRight => 2,
        //                     Anchor::CenterLeft => 3,
        //                     Anchor::Center => 4,
        //                     Anchor::CenterRight => 5,
        //                     Anchor::BottomLeft => 6,
        //                     Anchor::BottomCenter => 7,
        //                     Anchor::BottomRight => 8,
        //                     Anchor::Custom(_) => 9,
        //                 };

        //                 let mut new_idx = idx;

        //                 ui.label("Anchor");
        //                 egui::ComboBox::new("anchor-picker", "").show_index(
        //                     ui,
        //                     &mut new_idx,
        //                     ANCHORS.len(),
        //                     |idx| ANCHORS[idx],
        //                 );

        //                 if new_idx != idx {
        //                     node.settings.anchor = match new_idx {
        //                         0 => Anchor::TopLeft,
        //                         1 => Anchor::TopCenter,
        //                         2 => Anchor::TopRight,
        //                         3 => Anchor::CenterLeft,
        //                         4 => Anchor::Center,
        //                         5 => Anchor::CenterRight,
        //                         6 => Anchor::BottomLeft,
        //                         7 => Anchor::BottomCenter,
        //                         8 => Anchor::BottomRight,
        //                         9 => Anchor::Custom(node.settings.anchor.as_vec()),
        //                         _ => unimplemented!(),
        //                     }
        //                 }

        //                 if let Anchor::Custom(mut custom) = node.settings.anchor {
        //                     ui.add(egui::DragValue::new(&mut custom.x).speed(0.001));
        //                     ui.add(egui::DragValue::new(&mut custom.y).speed(0.001));
        //                     node.settings.anchor = Anchor::Custom(custom);
        //                 }
        //             });

        //             ui.horizontal(|ui| {
        //                 ui.label("Rotation");
        //                 ui.add(egui::DragValue::new(&mut node.settings.rotation).speed(1.0));
        //                 if node.settings.rotation < 0.0 {
        //                     node.settings.rotation += -node.settings.rotation.floor() * 360.0;
        //                 }
        //                 node.settings.rotation = node.settings.rotation % 360.0;
        //             });

        //             ui.horizontal(|ui| {
        //                 ui.label("Scale");
        //                 ui.add(egui::DragValue::new(&mut node.settings.scale.x).speed(0.01));
        //                 ui.add(egui::DragValue::new(&mut node.settings.scale.y).speed(0.01));
        //             });

        //             node.changed = true;

        //             const NODE_KINDS: &[&str] = &["Empty Node", "Texture Node", "Text Node"];

        //             let idx = if node.try_downcast::<EmptyNode>().is_some() {
        //                 0
        //             } else if node.try_downcast::<TextureNode<WgpuBackend>>().is_some() {
        //                 1
        //             } else if node.try_downcast::<TextNode<WgpuBackend>>().is_some() {
        //                 2
        //             } else {
        //                 unimplemented!()
        //             };

        //             let mut new_idx = idx;

        //             egui::ComboBox::new("node-kind-picker", "Node Kind").show_index(
        //                 ui,
        //                 &mut new_idx,
        //                 NODE_KINDS.len(),
        //                 |x| NODE_KINDS[x],
        //             );

        //             if new_idx != idx {
        //                 match new_idx {
        //                     0 => node.set_impl(&mut data.backend, EmptyNode),
        //                     1 => node.set_impl(&mut data.backend, TextureNode::new("")),
        //                     2 => node.set_impl(
        //                         &mut data.backend,
        //                         TextNode::new("Rodin", 32.0, 32.0, ""),
        //                     ),
        //                     _ => unimplemented!(),
        //                 }
        //             }

        //             if let Some(image) = node.try_downcast_mut::<TextureNode<WgpuBackend>>() {
        //                 ui.heading("Texture Node Settings");
        //                 ui.separator();

        //                 egui::ComboBox::new("texture-picker", "Texture")
        //                     .selected_text(image.texture_name())
        //                     .show_ui(ui, |ui| {
        //                         let mut new_texture = None;
        //                         for texture in data.backend.iter_texture_names() {
        //                             if ui
        //                                 .selectable_label(image.texture_name() == texture, texture)
        //                                 .clicked()
        //                             {
        //                                 new_texture = Some(texture.to_string());
        //                                 ui.close();
        //                                 break;
        //                             }
        //                         }

        //                         if let Some(new_texture) = new_texture {
        //                             image.update_texture(&mut data.backend, new_texture);
        //                         }
        //                     });
        //             }

        //             if let Some(text) = node.try_downcast_mut::<TextNode<WgpuBackend>>() {
        //                 ui.heading("Text Node Settings");
        //                 ui.separator();

        //                 egui::ComboBox::new("font-picker", "Font")
        //                     .selected_text(text.font_name())
        //                     .show_ui(ui, |ui| {
        //                         let mut new_font = None;
        //                         for font in data.backend.iter_font_names() {
        //                             if ui
        //                                 .selectable_label(text.font_name() == font, font)
        //                                 .clicked()
        //                             {
        //                                 new_font = Some(font.to_string());
        //                                 ui.close();
        //                                 break;
        //                             }
        //                         }

        //                         if let Some(font) = new_font {
        //                             text.update_font_name(&mut data.backend, font);
        //                         }
        //                     });

        //                 ui.horizontal(|ui| {
        //                     ui.label("Font Size");
        //                     if ui
        //                         .add(egui::DragValue::new(&mut text.font_size).speed(1.0))
        //                         .changed()
        //                     {
        //                         text.set_dirty();
        //                     }
        //                 });

        //                 ui.horizontal(|ui| {
        //                     ui.label("Line Height");
        //                     if ui
        //                         .add(egui::DragValue::new(&mut text.line_height).speed(1.0))
        //                         .changed()
        //                     {
        //                         text.set_dirty();
        //                     }
        //                 });

        //                 ui.horizontal(|ui| {
        //                     ui.label("Text");
        //                     if ui.text_edit_multiline(&mut text.text).changed() {
        //                         text.set_dirty();
        //                     }
        //                 });
        //             }
        //         }

        //         frame
        //             .wgpu_render_state()
        //             .unwrap()
        //             .renderer
        //             .write()
        //             .callback_resources
        //             .insert(data);
        //     });
        // }

        // egui::SidePanel::right("resource_viewer").show(ctx, |ui| {
        //     let mut data = frame
        //         .wgpu_render_state()
        //         .unwrap()
        //         .renderer
        //         .write()
        //         .callback_resources
        //         .remove::<EnvyResources>()
        //         .unwrap();

        //     ui.heading("Textures");
        //     ui.separator();

        //     let mut renames = vec![];
        //     let mut removes = vec![];
        //     for name in data.backend.iter_texture_names() {
        //         ui.horizontal(|ui| {
        //             let mut new_name = name.to_string();
        //             if ui.text_edit_singleline(&mut new_name).changed() {
        //                 renames.push((name.to_string(), new_name));
        //             }

        //             if ui.button("Remove").clicked() {
        //                 removes.push(name.to_string());
        //             }
        //         });
        //     }

        //     for (old, new) in renames {
        //         data.backend.rename_texture(&old, new.clone());
        //         data.tree.visit_all_nodes_mut(|node| {
        //             if let Some(image) = node.try_downcast_mut::<TextureNode<WgpuBackend>>() {
        //                 if image.texture_name() == old {
        //                     image.update_texture(&mut data.backend, new.clone());
        //                 }
        //             }
        //         });
        //     }

        //     for remove in removes {
        //         data.backend.remove_texture(&remove);
        //         data.tree.visit_all_nodes_mut(|node| {
        //             if let Some(image) = node.try_downcast_mut::<TextureNode<WgpuBackend>>() {
        //                 if image.texture_name() == remove {
        //                     image.update_texture(&mut data.backend, "");
        //                 }
        //             }
        //         });
        //     }

        //     if ui.button("Import Texture").clicked() {
        //         let file = rfd::FileDialog::new()
        //             .add_filter("PNG Images", &["png"])
        //             .set_title("Import Texture To ENVY Layout")
        //             .pick_file();

        //         if let Some(file) = file {
        //             let utf8_path = Utf8PathBuf::from_path_buf(file).unwrap();
        //             let file_stem = utf8_path.file_stem().unwrap();
        //             data.backend
        //                 .load_textures_from_paths([(file_stem, utf8_path.as_path())]);
        //         }
        //     }

        //     ui.heading("Fonts");
        //     ui.separator();

        //     let mut renames = vec![];
        //     // let mut removes = vec![];
        //     for name in data.backend.iter_font_names() {
        //         ui.horizontal(|ui| {
        //             let mut new_name = name.to_string();
        //             if ui.text_edit_singleline(&mut new_name).changed() {
        //                 renames.push((name.to_string(), new_name));
        //             }
        //         });
        //     }

        //     for (old, new) in renames {
        //         data.backend.rename_font(&old, new.clone());
        //         data.tree.visit_all_nodes_mut(|node| {
        //             if let Some(image) = node.try_downcast_mut::<TextNode<WgpuBackend>>() {
        //                 if image.font_name() == old {
        //                     image.update_font_name(&mut data.backend, new.clone());
        //                 }
        //             }
        //         });
        //     }

        //     if ui.button("Import Font").clicked() {
        //         let file = rfd::FileDialog::new()
        //             .add_filter("Font Files", &["ttf", "otf"])
        //             .set_title("Import Font To ENVY Layout")
        //             .pick_file();

        //         if let Some(file) = file {
        //             let file_stem = file.file_stem().unwrap().to_str().unwrap();
        //             let bytes = std::fs::read(&file).unwrap();
        //             data.backend.load_fonts_from_bytes([(file_stem, bytes)]);
        //         }
        //     }

        //     frame
        //         .wgpu_render_state()
        //         .unwrap()
        //         .renderer
        //         .write()
        //         .callback_resources
        //         .insert(data);
        // });

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
        _egui_encoder: &mut wgpu::CommandEncoder,
        resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let resources: &mut EnvyResources = resources.get_mut().unwrap();
        resources.prepare();

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
    tree: envy::LayoutTree<WgpuBackend>,
    icons: Icons,
}

impl EnvyResources {
    pub fn new(state: &egui_wgpu::RenderState) -> Self {
        let mut backend = WgpuBackend::new(
            state.device.clone(),
            state.queue.clone(),
            state.target_format,
        );

        let animation = Animation {
            node_animations: vec![NodeAnimation {
                node_path: Utf8PathBuf::from("test_node"),
                angle_channel: Some(AnimationChannel {
                    start: 0.0,
                    transforms: vec![AnimationTransform {
                        duration: 240.0,
                        end: 360.0,
                        first_step: TransformStep::Linear,
                        additional_steps: vec![],
                    }],
                }),
            }],
        };

        let mut tree = envy::LayoutTree::new()
            .with_child(
                NodeItem::new(
                    "test_node",
                    NodeTransform::from_size(Vec2::splat(600.0)),
                    [255; 4],
                    ImageNode::new("icon"),
                )
                .with_on_update(|node: NodeDisjointAccessor<'_, WgpuBackend>| {
                    // let mut this = node.self_mut();
                    // this.transform_mut().angle += 1.0;
                    // this.mark_changed();

                    // let mut sibling = node.sibling_mut("test_node2").unwrap();

                    // sibling.transform_mut().angle -= 1.0;
                    // sibling.mark_changed();
                }),
            )
            .with_child(NodeItem::new(
                "test_node2",
                NodeTransform::from_size(Vec2::splat(300.0)).with_xy(700.0, 700.0),
                [255; 4],
                ImageNode::new("icon"),
            ));

        backend.load_textures_from_paths([("icon", Utf8Path::new("./icon.png"))]);

        tree.add_animation("test_animation", animation);
        tree.setup(&mut backend);

        tree.play_animation("test_animation");

        // let file = file::MenuFile::open("/home/blujay/.config/Ryujinx/sdcard/stratus.envy");
        // backend.load_textures_from_bytes(
        //     file.image_resources
        //         .iter()
        //         .map(|(name, bytes)| (name.as_str(), Cow::Borrowed(bytes.as_slice()))),
        // );
        // backend.load_fonts_from_bytes(
        //     file.font_resources
        //         .iter()
        //         .map(|(name, bytes)| (name.as_str(), bytes.clone())),
        // );

        // let mut tree = file.create_tree();
        // tree.setup(&mut backend);

        Self {
            backend,
            tree,
            icons: Icons::new(),
        }
    }

    fn prepare(&mut self) {
        self.tree.update_animations();
        self.tree.update();
        self.tree.propagate();
        self.tree.prepare(&mut self.backend);
        self.backend.update();
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass) {
        self.backend.prep_render(render_pass);
        self.tree.render(&self.backend, render_pass);
    }
}

static IMAGE_PNG: &'static [u8] = include_bytes!("../../icon.png");

fn main() {
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
