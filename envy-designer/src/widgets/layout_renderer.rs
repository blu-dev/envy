use std::sync::Arc;

use camino::{Utf8Path, Utf8PathBuf};
use egui::epaint::ViewportInPixels;
use egui_ltreeview::DirPosition;
use egui_wgpu::CallbackTrait;
use envy::{
    ImageNodeTemplate, ImageScalingMode, LayoutRoot, LayoutTemplate, LayoutTree, MoveNodePosition, NodeImplTemplate, NodeTemplate, NodeTransform, NodeVisibility, SublayoutNodeTemplate, TextNodeTemplate
};
use envy_wgpu::WgpuBackend;
use parking_lot::Mutex;

pub(super) mod pipeline;

pub use pipeline::SAMPLE_COUNT;

use crate::widgets::layout_renderer::pipeline::CopyTexturePipeline;

enum LayoutKind {
    Root,
    Sublayout {
        name: String,
        tree: Arc<Mutex<LayoutTree<WgpuBackend>>>,
    },
}

pub enum LayoutRendererCommand {
    RenameSublayout {
        old_name: String,
        new_name: String,
    },
    MoveNode {
        old_path: Utf8PathBuf,
        new_path: Utf8PathBuf,
    },
    RefreshSublayout {
        name: String,
        path: Option<Utf8PathBuf>,
    },
}

pub enum LayoutReference<'a> {
    Root,
    Sublayout(&'a str),
}

pub struct LayoutRenderer {
    pipeline: Arc<CopyTexturePipeline>,
    root: Arc<Mutex<LayoutRoot<WgpuBackend>>>,
    wgpu_backend: Arc<Mutex<envy_wgpu::WgpuBackend>>,
    editing: Option<Utf8PathBuf>,
    kind: LayoutKind,
}

impl LayoutRenderer {
    fn access_template(&self) -> parking_lot::MappedMutexGuard<'_, LayoutTemplate> {
        let root = self.root.lock();

        match &self.kind {
            LayoutKind::Root => parking_lot::MutexGuard::map(root, |root| root.root_template_mut()),
            LayoutKind::Sublayout { name, .. } => {
                parking_lot::MutexGuard::map(root, |root| root.template_mut(name).unwrap())
            }
        }
    }

    fn access_layout(&self) -> parking_lot::MappedMutexGuard<'_, LayoutTree<WgpuBackend>> {
        match &self.kind {
            LayoutKind::Root => {
                parking_lot::MutexGuard::map(self.root.lock(), |root| root.as_layout_mut())
            }
            LayoutKind::Sublayout { tree, .. } => {
                parking_lot::MutexGuard::map(tree.lock(), |tree| tree)
            }
        }
    }

    pub fn new(
        root: Arc<Mutex<LayoutRoot<WgpuBackend>>>,
        backend: Arc<Mutex<WgpuBackend>>,
        render_state: &egui_wgpu::RenderState,
        reference: LayoutReference,
    ) -> Self {
        let kind = match reference {
            LayoutReference::Root => LayoutKind::Root,
            LayoutReference::Sublayout(name) => {
                let mut tree = root.lock().instantiate_tree_from_template(name).unwrap();
                tree.setup(&mut backend.lock());
                LayoutKind::Sublayout {
                    name: name.to_string(),
                    tree: Arc::new(Mutex::new(tree)),
                }
            }
        };

        Self {
            pipeline: Arc::new(CopyTexturePipeline::new(
                &render_state.device,
                render_state.target_format,
            )),
            root,
            wgpu_backend: backend,
            editing: None,
            kind,
        }
    }

    pub fn reference(&self) -> LayoutReference<'_> {
        match &self.kind {
            LayoutKind::Root => LayoutReference::Root,
            LayoutKind::Sublayout { name, .. } => LayoutReference::Sublayout(name.as_str()),
        }
    }

    pub fn try_rename(&mut self, old: &str, new: &str) {
        match &mut self.kind {
            LayoutKind::Sublayout { name, .. } if *name == old => *name = new.to_string(),
            _ => {}
        }
    }

    pub fn reinit(&mut self) {
        let mut root = self.root.lock();
        let mut backend = self.wgpu_backend.lock();
        match &self.kind {
            LayoutKind::Root => root.sync_root_template(&mut backend),
            LayoutKind::Sublayout { name, tree } => {
                let template = root.template(name).unwrap();
                tree.lock().sync_to_template(template, &root, &mut backend);
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) -> Vec<LayoutRendererCommand> {
        let id = ui.make_persistent_id("node tree view");

        let mut commands = vec![];

        egui::SidePanel::left("tree-view").show_inside(ui, |ui| {
            let mut changed = false;
            if let LayoutKind::Sublayout { name, .. } = &mut self.kind {
                ui.horizontal(|ui| {
                    let mut new_name = name.clone();
                    if ui.text_edit_singleline(&mut new_name).changed() {
                        let root = self.root.lock();
                        if root.template(&new_name).is_none() {
                            commands.push(LayoutRendererCommand::RenameSublayout {
                                old_name: name.clone(),
                                new_name,
                            });
                        }
                    }
                });
                ui.horizontal(|ui| {
                    ui.label("Canvas Size");

                    egui::Grid::new("canvas-size").show(ui, |ui| {
                        let mut template = self.access_template();
                        changed |= ui
                            .add(egui::DragValue::new(&mut template.canvas_size[0]))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut template.canvas_size[1]))
                            .changed();
                    });
                });
            }

            let mut template = self.access_template();
            let mut new_nodes = vec![];
            let mut remove_nodes = vec![];
            let (_, actions) = egui_ltreeview::TreeView::<'_, Utf8PathBuf>::new(id)
                .with_settings(egui_ltreeview::TreeViewSettings {
                    allow_multi_select: false,
                    ..Default::default()
                })
                .show(ui, |builder| {
                    fn visit_node_recursive(
                        builder: &mut egui_ltreeview::TreeViewBuilder<'_, Utf8PathBuf>,
                        parent: &Utf8Path,
                        node: &NodeTemplate,
                        new: &mut Vec<Utf8PathBuf>,
                        remove: &mut Vec<Utf8PathBuf>,
                    ) {
                        let path = parent.join(&node.name);
                        builder.node(
                            egui_ltreeview::NodeBuilder::dir(path.clone())
                                .label(&node.name)
                                .context_menu(|ui| {
                                    if ui.button("Add New Node").clicked() {
                                        new.push(path.clone());
                                    } else if ui.button("Delete").clicked() {
                                        remove.push(path.clone());
                                    }
                                }),
                        );

                        node.visit_children(|child| {
                            visit_node_recursive(builder, &path, child, new, remove);
                        });

                        builder.close_dir();
                    }

                    builder.node(
                        egui_ltreeview::NodeBuilder::dir(Utf8PathBuf::from(""))
                            .label("Root")
                            .context_menu(|ui| {
                                if ui.button("Add New Node").clicked() {
                                    new_nodes.push(Utf8PathBuf::from(""));
                                }
                            }),
                    );
                    template.visit_roots(|root| {
                        visit_node_recursive(
                            builder,
                            Utf8Path::new(""),
                            root,
                            &mut new_nodes,
                            &mut remove_nodes,
                        );
                    });
                    builder.close_dir();
                });

            for node in new_nodes {
                let new_node = NodeTemplate {
                    name: "new_node".into(),
                    transform: NodeTransform::default(),
                    color: [255; 4],
                    children: vec![],
                    visibility: NodeVisibility::default(),
                    implementation: NodeImplTemplate::Empty,
                };
                if node.as_str().is_empty() {
                    template.add_child(new_node);
                } else {
                    let parent = template.get_node_by_path_mut(&node).unwrap();
                    assert!(parent.add_child(new_node));
                }
                changed |= true;
            }

            let mut editing = self.editing.clone();
            for node in remove_nodes {
                template.remove_node(&node);
                if editing.as_ref() == Some(&node) {
                    editing = None;
                }
                changed |= true;
            }


            for action in actions {
                use egui_ltreeview::Action;

                match action {
                    Action::SetSelected(nodes) => {
                        editing = Some(nodes[0].clone());
                    }
                    Action::Move(drag_drop) => {
                        let old_path = &drag_drop.source[0];
                        let new_path = drag_drop
                            .target
                            .join(drag_drop.source[0].file_name().unwrap());
                        template.move_node(old_path, &new_path, {
                            match &drag_drop.position {
                                DirPosition::First => MoveNodePosition::First,
                                DirPosition::Last => MoveNodePosition::Last,
                                DirPosition::Before(before) => {
                                    MoveNodePosition::Before(before.file_name().unwrap())
                                }
                                DirPosition::After(after) => {
                                    MoveNodePosition::After(after.file_name().unwrap())
                                }
                            }
                        });

                        if editing
                            .as_ref()
                            .is_some_and(|editing| *editing == *old_path)
                        {
                            editing = Some(new_path)
                        }

                        changed |= true;
                    }
                    Action::Drag(_) => {}
                    Action::Activate(_) => {}
                    Action::DragExternal(_) => {}
                    Action::MoveExternal(_) => {}
                }
            }

            drop(template);

            self.editing = editing;

            if changed {
                match &self.kind {
                    LayoutKind::Root => {
                        let mut root = self.root.lock();
                        root.sync_root_template(&mut self.wgpu_backend.lock());
                    }
                    LayoutKind::Sublayout { name, .. } => {
                        let mut layout = self.access_layout();
                        let mut root = self.root.lock();
                        let mut backend = self.wgpu_backend.lock();
                        root.sync_template(name, &mut backend);
                        let template = root.template(name).unwrap();
                        layout.sync_to_template(template, &root, &mut backend);
                        commands.push(LayoutRendererCommand::RefreshSublayout {
                            name: name.clone(),
                            path: None,
                        });
                    }
                }
            }
        });

        if let Some(editing) = self
            .editing
            .as_ref()
            .filter(|node| !node.as_str().is_empty())
        {
            let mut changed = false;
            let mut new_path = None;
            egui::SidePanel::left("editing-node").show_inside(ui, |ui| {
                let template_names = {
                    self.root
                        .lock()
                        .iter_templates()
                        .into_iter()
                        .filter(|(name, _)| !name.is_empty())
                        .map(|(name, _)| name.to_string())
                        .collect::<Vec<_>>()
                };
                let mut template = self.access_template();
                let node = template.get_node_by_path_mut(editing).unwrap();
                ui.horizontal(|ui| {
                    ui.label("Name");
                    let mut name = node.name.clone();
                    if ui.text_edit_singleline(&mut name).changed() && !name.is_empty() {
                        node.name = name;
                        new_path = Some(editing.with_file_name(&node.name));
                    }
                    ui.end_row();
                });

                egui::Grid::new("transform-editor").show(ui, |ui| {
                    ui.label("Position");
                    egui::Grid::new("position").show(ui, |ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut node.transform.position.x).speed(1.0))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut node.transform.position.y).speed(1.0))
                            .changed();
                    });
                    ui.end_row();

                    ui.label("Size");
                    egui::Grid::new("size").show(ui, |ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut node.transform.size.x).speed(1.0))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut node.transform.size.y).speed(1.0))
                            .changed();
                    });
                    ui.end_row();

                    ui.label("Scale");
                    egui::Grid::new("scale").show(ui, |ui| {
                        changed |= ui
                            .add(egui::DragValue::new(&mut node.transform.scale.x).speed(1.0))
                            .changed();
                        changed |= ui
                            .add(egui::DragValue::new(&mut node.transform.scale.y).speed(1.0))
                            .changed();
                    });
                    ui.end_row();

                    ui.label("Angle");
                    changed |= ui
                        .add(egui::DragValue::new(&mut node.transform.angle).speed(1.0))
                        .changed();
                    if node.transform.angle < 0.0 {
                        node.transform.angle += 360.0 * -(node.transform.angle / 360.0).floor();
                    }
                    node.transform.angle %= 360.0;
                    ui.end_row();

                    ui.label("Visibility");
                    egui::ComboBox::new("visibility", "")
                        .selected_text(match node.visibility {
                            NodeVisibility::Hidden => "Hidden",
                            NodeVisibility::Inherited => "Inherited",
                            NodeVisibility::Visible => "Visible"
                        })
                        .show_ui(ui, |ui| {
                            if ui.selectable_label(matches!(node.visibility, NodeVisibility::Hidden), "Hidden").clicked() {
                                changed = true;
                                node.visibility = NodeVisibility::Hidden;
                                ui.close();
                            } else if ui.selectable_label(matches!(node.visibility, NodeVisibility::Inherited), "Inherited").clicked() {
                                changed = true;
                                node.visibility = NodeVisibility::Inherited;
                                ui.close()
                            } else if ui.selectable_label(matches!(node.visibility, NodeVisibility::Visible), "Visible").clicked() {
                                changed = true;
                                node.visibility = NodeVisibility::Visible;
                                ui.close()
                            }
                        });
                    ui.end_row();

                    ui.label("Color");
                    changed |= ui
                        .color_edit_button_srgba_unmultiplied(&mut node.color)
                        .changed();
                    ui.end_row();
                });

                ui.horizontal(|ui| {
                    ui.label("Node Type");
                    let supported = ["Empty", "Image", "Text", "Sublayout"];

                    let mut current_idx = match &node.implementation {
                        NodeImplTemplate::Empty => 0,
                        NodeImplTemplate::Image(_) => 1,
                        NodeImplTemplate::Text(_) => 2,
                        NodeImplTemplate::Sublayout(_) => 3,
                    };

                    let old_idx = current_idx;

                    egui::ComboBox::new("node-picker", "").show_index(
                        ui,
                        &mut current_idx,
                        4,
                        |x| supported[x],
                    );

                    if old_idx != current_idx {
                        match current_idx {
                            0 => node.implementation = NodeImplTemplate::Empty,
                            1 => {
                                node.implementation = NodeImplTemplate::Image(ImageNodeTemplate {
                                    texture_name: "".to_string(),
                                    mask_texture_name: None,
                                    image_scaling_mode_x: Default::default(),
                                    image_scaling_mode_y: Default::default(),
                                    uv_offset: glam::Vec2::ZERO,
                                    uv_scale: glam::Vec2::ONE,
                                })
                            }
                            2 => {
                                node.implementation = NodeImplTemplate::Text(TextNodeTemplate {
                                    font_name: "".to_string(),
                                    text: "".to_string(),
                                    font_size: 32.0,
                                    line_height: 32.0,
                                })
                            }
                            3 => {
                                node.implementation =
                                    NodeImplTemplate::Sublayout(SublayoutNodeTemplate {
                                        sublayout_name: "".to_string(),
                                    })
                            }
                            _ => unimplemented!(),
                        }
                        changed |= true;
                    }
                });

                ui.separator();

                match &mut node.implementation {
                    NodeImplTemplate::Empty => {}
                    NodeImplTemplate::Image(image) => {
                        ui.horizontal(|ui| {
                            ui.label("Texture");
                            egui::ComboBox::new("texture-name", "")
                                .selected_text(&image.texture_name)
                                .show_ui(ui, |ui| {
                                    let backend = self.wgpu_backend.lock();
                                    for name in backend.iter_texture_names() {
                                        if ui
                                            .selectable_label(image.texture_name == name, name)
                                            .clicked()
                                        {
                                            image.texture_name = name.to_string();
                                            changed |= true;
                                        }
                                    }
                                });
                        });
                        ui.horizontal(|ui| {
                            ui.label("Mask Texture");
                            egui::ComboBox::new("mask-texture-name", "")
                                .selected_text(match &image.mask_texture_name {
                                    Some(name) => name.as_str(),
                                    None => "None",
                                })
                                .show_ui(ui, |ui| {
                                    let backend = self.wgpu_backend.lock();
                                    if ui
                                        .selectable_label(image.mask_texture_name.is_none(), "None")
                                        .clicked()
                                    {
                                        image.mask_texture_name = None;
                                        changed |= true;
                                        ui.close();
                                        return;
                                    }
                                    for name in backend.iter_texture_names() {
                                        if ui
                                            .selectable_label(
                                                image.mask_texture_name.as_deref() == Some(name),
                                                name,
                                            )
                                            .clicked()
                                        {
                                            image.mask_texture_name = Some(name.to_string());
                                            changed |= true;
                                            ui.close();
                                            return;
                                        }
                                    }
                                });
                        });

                        egui::Grid::new("texture-scaling")
                            .show(ui, |ui| {
                                ui.label("Texture Scale Mode X");
                                egui::ComboBox::new("x", "")
                                    .selected_text(match &image.image_scaling_mode_x {
                                        ImageScalingMode::Stretch => "Stretch",
                                        ImageScalingMode::Tiling => "Tiling"
                                    })
                                    .show_ui(ui, |ui| {
                                        if ui.selectable_label(matches!(&image.image_scaling_mode_x, ImageScalingMode::Stretch), "Stretch").clicked() {
                                            image.image_scaling_mode_x = ImageScalingMode::Stretch;
                                            changed |= true;
                                            ui.close();
                                        } else if ui.selectable_label(matches!(&image.image_scaling_mode_x, ImageScalingMode::Tiling), "Tiling").clicked() {
                                            image.image_scaling_mode_x = ImageScalingMode::Tiling;
                                            changed |= true;
                                            ui.close()
                                        }
                                    });
                                ui.end_row();
                                ui.label("Texture Scale Mode Y");
                                egui::ComboBox::new("y", "")
                                    .selected_text(match &image.image_scaling_mode_y {
                                        ImageScalingMode::Stretch => "Stretch",
                                        ImageScalingMode::Tiling => "Tiling"
                                    })
                                    .show_ui(ui, |ui| {
                                        if ui.selectable_label(matches!(&image.image_scaling_mode_y, ImageScalingMode::Stretch), "Stretch").clicked() {
                                            image.image_scaling_mode_y = ImageScalingMode::Stretch;
                                            changed |= true;
                                            ui.close();
                                        } else if ui.selectable_label(matches!(&image.image_scaling_mode_y, ImageScalingMode::Tiling), "Tiling").clicked() {
                                            image.image_scaling_mode_y = ImageScalingMode::Tiling;
                                            changed |= true;
                                            ui.close()
                                        }
                                    });
                                ui.end_row();
                            });

                        egui::Grid::new("uvs")
                            .show(ui, |ui| {
                                ui.label("UV Offset (px)");
                                egui::Grid::new("offset")
                                    .show(ui, |ui| {
                                        changed |= ui.add(egui::DragValue::new(&mut image.uv_offset.x).speed(1.0)).changed();
                                        changed |= ui.add(egui::DragValue::new(&mut image.uv_offset.y).speed(1.0)).changed();
                                    });
                                ui.end_row();
                                ui.label("UV Scale");
                                egui::Grid::new("scale")
                                    .show(ui, |ui| {
                                        changed |= ui.add(egui::DragValue::new(&mut image.uv_scale.x).speed(0.1)).changed();
                                        changed |= ui.add(egui::DragValue::new(&mut image.uv_scale.y).speed(0.1)).changed();
                                    });
                            });
                    }
                    NodeImplTemplate::Text(text) => {
                        ui.horizontal(|ui| {
                            ui.label("Font");
                            egui::ComboBox::new("font-name", "")
                                .selected_text(&text.font_name)
                                .show_ui(ui, |ui| {
                                    let backend = self.wgpu_backend.lock();
                                    for name in backend.iter_font_names() {
                                        if ui
                                            .selectable_label(name == text.font_name, name)
                                            .clicked()
                                        {
                                            text.font_name = name.to_string();
                                            changed |= true;
                                        }
                                    }
                                });
                        });

                        egui::Grid::new("text-grid").show(ui, |ui| {
                            ui.label("Font Size");
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut text.font_size)
                                        .range(1.0..=f32::INFINITY)
                                        .speed(1.0),
                                )
                                .changed();
                            ui.end_row();
                            ui.label("Line Height");
                            changed |= ui
                                .add(
                                    egui::DragValue::new(&mut text.line_height)
                                        .range(text.font_size..=f32::INFINITY)
                                        .speed(1.0)
                                        .clamp_existing_to_range(true),
                                )
                                .changed();
                        });

                        ui.horizontal(|ui| {
                            ui.label("Text");
                            changed |= ui.text_edit_multiline(&mut text.text).changed();
                        });
                    }
                    NodeImplTemplate::Sublayout(sublayout) => {
                        ui.horizontal(|ui| {
                            ui.label("Sublayout");
                            egui::ComboBox::new("sublayout-name", "")
                                .selected_text(&sublayout.sublayout_name)
                                .show_ui(ui, |ui| {
                                    for name in template_names.iter() {
                                        if ui
                                            .selectable_label(
                                                sublayout.sublayout_name == *name,
                                                name,
                                            )
                                            .clicked()
                                        {
                                            sublayout.sublayout_name = name.to_string();
                                            changed |= true;
                                        }
                                    }
                                });
                        });
                    }
                }
            });

            if let Some(new_path) = new_path {
                {
                    let mut template = self.access_template();
                    template.animations.iter_mut().for_each(|(_, animation)| {
                        animation.node_animations.iter_mut().for_each(|anim| {
                            if anim.node_path == editing.as_str() {
                                anim.node_path = new_path.to_string();
                            }
                        })
                    });
                }
                commands.push(LayoutRendererCommand::MoveNode {
                    old_path: editing.clone(),
                    new_path: new_path.clone(),
                });
                self.editing = Some(new_path);
                match &self.kind {
                    LayoutKind::Root => {
                        let mut root = self.root.lock();
                        root.sync_root_template(&mut self.wgpu_backend.lock());
                    }
                    LayoutKind::Sublayout { name, .. } => {
                        let mut layout = self.access_layout();
                        let mut root = self.root.lock();
                        let mut backend = self.wgpu_backend.lock();
                        root.sync_template(name, &mut backend);
                        let template = root.template(name).unwrap();
                        layout.sync_to_template(template, &root, &mut backend);
                        commands.push(LayoutRendererCommand::RefreshSublayout {
                            name: name.clone(),
                            path: None,
                        });
                    }
                }
            } else if changed {
                match &self.kind {
                    LayoutKind::Root => {
                        let mut root = self.root.lock();
                        root.sync_root_template_by_path(editing, &mut self.wgpu_backend.lock());
                    }
                    LayoutKind::Sublayout { name, .. } => {
                        let mut layout = self.access_layout();
                        let mut root = self.root.lock();
                        let mut backend = self.wgpu_backend.lock();
                        root.sync_template_by_path(name, editing, &mut backend);
                        root.sync_root_template(&mut backend);
                        let template = root.template(name).unwrap();
                        layout.sync_to_template(template, &root, &mut backend);
                        commands.push(LayoutRendererCommand::RefreshSublayout {
                            name: name.clone(),
                            path: Some(editing.clone()),
                        });
                    }
                }
            }
        }

        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let (rect, _response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::drag());

            ui.painter()
                .with_clip_rect(rect)
                .add(egui_wgpu::Callback::new_paint_callback(
                    rect,
                    LayoutPaintCallback {
                        reference: match &self.kind {
                            LayoutKind::Root => PaintingReference::Root(self.root.clone()),
                            LayoutKind::Sublayout { tree, .. } => {
                                PaintingReference::Tree(tree.clone())
                            }
                        },
                        pipeline: self.pipeline.clone(),
                        backend: self.wgpu_backend.clone(),
                    },
                ));
        });

        commands
    }
}

pub enum PaintingReference {
    Root(Arc<Mutex<LayoutRoot<WgpuBackend>>>),
    Tree(Arc<Mutex<LayoutTree<WgpuBackend>>>),
}

impl PaintingReference {
    fn get_layout(&self) -> parking_lot::MappedMutexGuard<'_, LayoutTree<WgpuBackend>> {
        match self {
            Self::Root(root) => {
                parking_lot::MutexGuard::map(root.lock(), |root| root.as_layout_mut())
            }
            Self::Tree(tree) => parking_lot::MutexGuard::map(tree.lock(), |tree| tree),
        }
    }
}

pub(super) struct LayoutPaintCallback {
    pub reference: PaintingReference,
    pub pipeline: Arc<CopyTexturePipeline>,
    pub backend: Arc<Mutex<WgpuBackend>>,
}

impl CallbackTrait for LayoutPaintCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        egui_encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let mut layout = self.reference.get_layout();

        layout.update_animations();
        layout.update();
        layout.propagate();

        let mut backend = self.backend.lock();
        layout.prepare(&mut backend);
        backend.update();

        self.pipeline.render_to_texture(egui_encoder, |pass| {
            backend.prep_render(pass);
            layout.render(&backend, pass);
        });

        vec![]
    }

    fn paint(
        &self,
        info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
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
        self.pipeline.render_texture(render_pass);
    }
}

fn fit_aspect_in_viewport(target_w: f32, target_h: f32, viewport: ViewportInPixels) -> egui::Rect {
    let w_frac = viewport.width_px as f32 / target_w;
    let h_frac = viewport.height_px as f32 / target_h;
    let scale = w_frac.min(h_frac);
    let scaled_w = target_w * scale;
    let scaled_h = target_h * scale;

    let x_offset = (viewport.width_px as f32 - scaled_w) / 2.0;
    let y_offset = (viewport.height_px as f32 - scaled_h) / 2.0;

    egui::Rect::from_min_size(
        egui::Pos2::new(viewport.left_px as f32, viewport.top_px as f32)
            + egui::Vec2::new(x_offset, y_offset),
        egui::Vec2::new(scaled_w, scaled_h),
    )
}
