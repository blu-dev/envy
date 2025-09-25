use std::{sync::Arc, time::Instant};

use camino::{Utf8Path, Utf8PathBuf};
use egui_ltreeview::Action;
use envy::{Animation, AnimationChannel, LayoutRoot, LayoutTree, NodeAnimation, NodeTemplate};
use envy_wgpu::WgpuBackend;
use parking_lot::Mutex;

use crate::widgets::layout_renderer::{
    LayoutPaintCallback, PaintingReference, pipeline::CopyTexturePipeline,
};

pub enum LayoutReference {
    Root,
    Sublayout(String),
}

impl LayoutReference {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Root => "Root",
            Self::Sublayout(sub) => sub.as_str(),
        }
    }
}

pub struct AnimatorWidget {
    root: Arc<Mutex<LayoutRoot<WgpuBackend>>>,
    layout_reference: LayoutReference,
    tree: Arc<Mutex<LayoutTree<WgpuBackend>>>,
    backend: Arc<Mutex<WgpuBackend>>,
    pipeline: Arc<CopyTexturePipeline>,
    animation: String,
    editing_node: Option<Utf8PathBuf>,
    current_keyframe: usize,
    playback_start: Option<Instant>,
}

impl AnimatorWidget {
    pub fn new(
        root: Arc<Mutex<LayoutRoot<WgpuBackend>>>,
        backend: Arc<Mutex<WgpuBackend>>,
        state: &egui_wgpu::RenderState,
    ) -> Self {
        let root_lock = root.lock();
        let template = root_lock.root_template();
        let mut tree = LayoutTree::from_template(template, &root_lock);
        tree.setup(&mut backend.lock());
        drop(root_lock);
        let pipeline = Arc::new(CopyTexturePipeline::new(&state.device, state.target_format));
        Self {
            root,
            layout_reference: LayoutReference::Root,
            tree: Arc::new(Mutex::new(tree)),
            backend,
            pipeline,
            animation: String::new(),
            editing_node: None,
            current_keyframe: 0usize,
            playback_start: None,
        }
    }

    pub fn try_move_node(&mut self, old: &Utf8Path, new: &Utf8Path) {
        if self.editing_node.as_ref().is_some_and(|node| *node == *old) {
            self.editing_node = Some(new.to_path_buf());
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        let mut root = self.root.lock();
        egui::SidePanel::left("animation-controls").show_inside(ui, |ui| {
            ui.horizontal(|ui| {
                ui.label("Layout");
                egui::ComboBox::new("layout-picker", "")
                    .selected_text(self.layout_reference.as_str())
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(
                                matches!(&self.layout_reference, LayoutReference::Root),
                                "Root",
                            )
                            .clicked()
                        {
                            self.layout_reference = LayoutReference::Root;
                            self.editing_node = None;
                            self.animation.clear();
                        }

                        for (template, _) in root.templates() {
                            if template.is_empty() {
                                continue;
                            }

                            if ui
                                .selectable_label(
                                    self.layout_reference.as_str() == template,
                                    template,
                                )
                                .clicked()
                            {
                                self.layout_reference =
                                    LayoutReference::Sublayout(template.to_string());
                                self.editing_node = None;
                                self.animation.clear();
                                ui.close();
                                break;
                            }
                        }
                    });
            });

            let template = match &self.layout_reference {
                LayoutReference::Root => root.root_template_mut(),
                LayoutReference::Sublayout(reference) => root.template_mut(reference).unwrap(),
            };

            ui.horizontal(|ui| {
                ui.label("Animation");
                egui::ComboBox::new("animation-picker", "")
                    .selected_text(self.animation.as_str())
                    .show_ui(ui, |ui| {
                        for (animation, _) in template.animations.iter() {
                            if ui
                                .selectable_label(self.animation == *animation, animation)
                                .clicked()
                            {
                                self.animation = animation.clone();
                                self.editing_node = None;
                                ui.close();
                                break;
                            }
                        }
                    });

                if ui.button("New Animation").clicked() {
                    self.animation = "New Animation".to_string();
                    self.editing_node = None;
                    template.animations.push((
                        "New Animation".to_string(),
                        Animation {
                            total_duration: 0,
                            node_animations: vec![],
                        },
                    ));
                }
            });

            if self.animation.is_empty() {
                return;
            }

            let mut animation_name = self.animation.clone();
            if ui.text_edit_singleline(&mut animation_name).changed()
                && !template
                    .animations
                    .iter()
                    .any(|(name, _)| *name == animation_name)
                && !animation_name.is_empty()
            {
                let pos = template
                    .animations
                    .iter()
                    .position(|(name, _)| *name == self.animation)
                    .unwrap();
                let (_, anim) = template.animations.remove(pos);
                template
                    .animations
                    .insert(pos, (animation_name.clone(), anim));
                self.animation = animation_name;
            }

            let animation = &mut template
                .animations
                .iter_mut()
                .find(|(animation, _)| *animation == self.animation)
                .unwrap()
                .1;

            ui.horizontal(|ui| {
                ui.label("Duration");
                ui.add(egui::DragValue::new(&mut animation.total_duration));
            });

            if ui.button("Play Animation").clicked() {
                self.playback_start = Some(Instant::now());
            }

            ui.add_enabled_ui(self.playback_start.is_none(), |ui| {
                let enabled_color = ui.style().visuals.text_color();
                let disabled_color = ui.style().visuals.weak_text_color();

                let id = ui.make_persistent_id("animation-tree-view");
                let (_, commands) = egui_ltreeview::TreeView::new(id)
                    .with_settings(egui_ltreeview::TreeViewSettings {
                        allow_multi_select: false,
                        allow_drag_and_drop: false,
                        ..Default::default()
                    })
                    .show(ui, |builder| {
                        fn recursive(
                            enabled_color: egui::Color32,
                            disabled_color: egui::Color32,
                            node: &NodeTemplate,
                            parent: &Utf8Path,
                            animation: &Animation,
                            builder: &mut egui_ltreeview::TreeViewBuilder<'_, Utf8PathBuf>,
                        ) {
                            let path = parent.join(&node.name);

                            let is_animated = animation
                                .node_animations
                                .iter()
                                .any(|node| node.node_path == path);
                            let label: egui::WidgetText = if is_animated {
                                egui::RichText::new(&node.name).color(enabled_color).into()
                            } else {
                                egui::RichText::new(&node.name).color(disabled_color).into()
                            };

                            if node.children.is_empty() {
                                builder.leaf(path, label);
                            } else {
                                builder.dir(path.clone(), label);
                                node.visit_children(|child| {
                                    recursive(
                                        enabled_color,
                                        disabled_color,
                                        child,
                                        &path,
                                        animation,
                                        builder,
                                    );
                                });
                                builder.close_dir();
                            }
                        }

                        builder.dir(Utf8PathBuf::new(), "Root");
                        for root in template.root_nodes.iter() {
                            recursive(
                                enabled_color,
                                disabled_color,
                                root,
                                Utf8Path::new(""),
                                animation,
                                builder,
                            );
                        }
                        builder.close_dir();
                    });

                for action in commands {
                    let Action::SetSelected(nodes) = action else {
                        continue;
                    };

                    self.editing_node = Some(nodes[0].clone());
                }
            });
        });

        let Some(node) = self.editing_node.as_ref() else {
            return;
        };

        let template = match &self.layout_reference {
            LayoutReference::Root => root.root_template_mut(),
            LayoutReference::Sublayout(reference) => root.template_mut(reference).unwrap(),
        };

        // This could technically fail due to tab render order if we rename the node in a different
        // tab while this one is visible
        let Some(default_node) = template.get_node_by_path(node).map(|node| node.clone()) else {
            return;
        };

        let animation = &mut template
            .animations
            .iter_mut()
            .find(|(animation, _)| *animation == self.animation)
            .unwrap()
            .1;

        let node_animation = if let Some(animation) = animation
            .node_animations
            .iter_mut()
            .find(|anim| anim.node_path.as_str() == node.as_str())
        {
            animation
        } else {
            animation.node_animations.push(NodeAnimation {
                node_path: node.to_string(),
                angle_channel: None,
                position_channel: None,
                scale_channel: None,
                size_channel: None,
                color_channel: None,
            });
            animation.node_animations.last_mut().unwrap()
        };

        self.current_keyframe = self.current_keyframe.min(animation.total_duration);

        ui.add_enabled_ui(self.playback_start.is_none(), |ui| {
            egui::TopBottomPanel::top("node-animation-controls").show_inside(ui, |ui| {
                ui.heading(format!("Animating {}", node_animation.node_path));
                ui.style_mut().spacing.slider_width = ui.available_width() * 0.8;
                ui.add(egui::Slider::new(
                    &mut self.current_keyframe,
                    0..=animation.total_duration,
                ));

                ui.horizontal(|ui| {
                    let mut has_angle_channel = node_animation.angle_channel.is_some();
                    if ui
                        .checkbox(&mut has_angle_channel, "Animate Angle Channel?")
                        .changed()
                    {
                        if has_angle_channel {
                            node_animation.angle_channel = Some(AnimationChannel {
                                start: default_node.transform.angle,
                                transforms: vec![],
                            });
                        } else {
                            node_animation.angle_channel = None;
                        }
                    }

                    let mut has_position_channel = node_animation.position_channel.is_some();
                    if ui
                        .checkbox(&mut has_position_channel, "Animate Position Channel?")
                        .changed()
                    {
                        if has_position_channel {
                            node_animation.position_channel = Some(AnimationChannel {
                                start: default_node.transform.position,
                                transforms: vec![],
                            });
                        } else {
                            node_animation.position_channel = None;
                        }
                    }

                    let mut has_size_channel = node_animation.size_channel.is_some();
                    if ui
                        .checkbox(&mut has_size_channel, "Animate Size Channel?")
                        .changed()
                    {
                        if has_size_channel {
                            node_animation.size_channel = Some(AnimationChannel {
                                start: default_node.transform.size,
                                transforms: vec![],
                            });
                        } else {
                            node_animation.size_channel = None;
                        }
                    }

                    let mut has_scale_channel = node_animation.scale_channel.is_some();
                    if ui
                        .checkbox(&mut has_scale_channel, "Animate Scale Channel?")
                        .changed()
                    {
                        if has_scale_channel {
                            node_animation.scale_channel = Some(AnimationChannel {
                                start: default_node.transform.scale,
                                transforms: vec![],
                            });
                        } else {
                            node_animation.scale_channel = None;
                        }
                    }

                    let mut has_color_channel = node_animation.color_channel.is_some();
                    if ui
                        .checkbox(&mut has_color_channel, "Animate Color Channel?")
                        .changed()
                    {
                        if has_color_channel {
                            node_animation.color_channel = Some(AnimationChannel {
                                start: default_node.color,
                                transforms: vec![],
                            });
                        } else {
                            node_animation.color_channel = None;
                        }
                    }
                });

                if let Some(channel) = node_animation.angle_channel.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Angle");

                        let keyframe = channel.keyframe_mut(self.current_keyframe);
                        let mut has_keyframe = keyframe.is_some();
                        if ui.checkbox(&mut has_keyframe, "").changed() {
                            if has_keyframe {
                                channel.insert_keyframe(self.current_keyframe);
                            } else {
                                channel.remove_keyframe(self.current_keyframe);
                            }
                        }

                        if let Some(value) = channel.keyframe_mut(self.current_keyframe) {
                            ui.add(egui::DragValue::new(value).speed(0.1));
                        } else {
                            let mut value = channel.value_for_frame(self.current_keyframe);
                            ui.add_enabled(false, egui::DragValue::new(&mut value));
                        }

                        if ui.button("<").clicked() {
                            self.current_keyframe =
                                channel.get_prev_keyframe_idx(self.current_keyframe);
                        } else if ui.button(">").clicked() {
                            self.current_keyframe =
                                channel.get_next_keyframe_idx(self.current_keyframe);
                        }
                    });
                }

                if let Some(channel) = node_animation.position_channel.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Position");

                        let keyframe = channel.keyframe_mut(self.current_keyframe);
                        let mut has_keyframe = keyframe.is_some();
                        if ui.checkbox(&mut has_keyframe, "").changed() {
                            if has_keyframe {
                                channel.insert_keyframe(self.current_keyframe);
                            } else {
                                channel.remove_keyframe(self.current_keyframe);
                            }
                        }

                        if let Some(value) = channel.keyframe_mut(self.current_keyframe) {
                            egui::Grid::new("pos-grid").show(ui, |ui| {
                                ui.add(egui::DragValue::new(&mut value.x));
                                ui.add(egui::DragValue::new(&mut value.y));
                            });
                        } else {
                            let mut value = channel.value_for_frame(self.current_keyframe);
                            egui::Grid::new("pos-grid").show(ui, |ui| {
                                ui.add_enabled(false, egui::DragValue::new(&mut value.x));
                                ui.add_enabled(false, egui::DragValue::new(&mut value.y));
                            });
                        }

                        if ui.button("<").clicked() {
                            self.current_keyframe =
                                channel.get_prev_keyframe_idx(self.current_keyframe);
                        } else if ui.button(">").clicked() {
                            self.current_keyframe =
                                channel.get_next_keyframe_idx(self.current_keyframe);
                        }
                    });
                }

                if let Some(channel) = node_animation.size_channel.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Size");

                        let keyframe = channel.keyframe_mut(self.current_keyframe);
                        let mut has_keyframe = keyframe.is_some();
                        if ui.checkbox(&mut has_keyframe, "").changed() {
                            if has_keyframe {
                                channel.insert_keyframe(self.current_keyframe);
                            } else {
                                channel.remove_keyframe(self.current_keyframe);
                            }
                        }

                        if let Some(value) = channel.keyframe_mut(self.current_keyframe) {
                            egui::Grid::new("size-grid").show(ui, |ui| {
                                ui.add(egui::DragValue::new(&mut value.x));
                                ui.add(egui::DragValue::new(&mut value.y));
                            });
                        } else {
                            let mut value = channel.value_for_frame(self.current_keyframe);
                            egui::Grid::new("size-grid").show(ui, |ui| {
                                ui.add_enabled(false, egui::DragValue::new(&mut value.x));
                                ui.add_enabled(false, egui::DragValue::new(&mut value.y));
                            });
                        }

                        if ui.button("<").clicked() {
                            self.current_keyframe =
                                channel.get_prev_keyframe_idx(self.current_keyframe);
                        } else if ui.button(">").clicked() {
                            self.current_keyframe =
                                channel.get_next_keyframe_idx(self.current_keyframe);
                        }
                    });
                }

                if let Some(channel) = node_animation.scale_channel.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Scale");

                        let keyframe = channel.keyframe_mut(self.current_keyframe);
                        let mut has_keyframe = keyframe.is_some();
                        if ui.checkbox(&mut has_keyframe, "").changed() {
                            if has_keyframe {
                                channel.insert_keyframe(self.current_keyframe);
                            } else {
                                channel.remove_keyframe(self.current_keyframe);
                            }
                        }

                        if let Some(value) = channel.keyframe_mut(self.current_keyframe) {
                            egui::Grid::new("scale-grid").show(ui, |ui| {
                                ui.add(egui::DragValue::new(&mut value.x));
                                ui.add(egui::DragValue::new(&mut value.y));
                            });
                        } else {
                            let mut value = channel.value_for_frame(self.current_keyframe);
                            egui::Grid::new("scale-grid").show(ui, |ui| {
                                ui.add_enabled(false, egui::DragValue::new(&mut value.x));
                                ui.add_enabled(false, egui::DragValue::new(&mut value.y));
                            });
                        }

                        if ui.button("<").clicked() {
                            self.current_keyframe =
                                channel.get_prev_keyframe_idx(self.current_keyframe);
                        } else if ui.button(">").clicked() {
                            self.current_keyframe =
                                channel.get_next_keyframe_idx(self.current_keyframe);
                        }
                    });
                }

                if let Some(channel) = node_animation.color_channel.as_mut() {
                    ui.horizontal(|ui| {
                        ui.label("Color");

                        let keyframe = channel.keyframe_mut(self.current_keyframe);
                        let mut has_keyframe = keyframe.is_some();
                        if ui.checkbox(&mut has_keyframe, "").changed() {
                            if has_keyframe {
                                channel.insert_keyframe(self.current_keyframe);
                            } else {
                                channel.remove_keyframe(self.current_keyframe);
                            }
                        }

                        if let Some(value) = channel.keyframe_mut(self.current_keyframe) {
                            ui.color_edit_button_srgba_unmultiplied(value);
                        } else {
                            let mut value = channel.value_for_frame(self.current_keyframe);
                            ui.color_edit_button_srgba_unmultiplied(&mut value);
                        }

                        if ui.button("<").clicked() {
                            self.current_keyframe =
                                channel.get_prev_keyframe_idx(self.current_keyframe);
                        } else if ui.button(">").clicked() {
                            self.current_keyframe =
                                channel.get_next_keyframe_idx(self.current_keyframe);
                        }
                    });
                }
            });
        });

        let template = match &self.layout_reference {
            LayoutReference::Root => root.root_template(),
            LayoutReference::Sublayout(reference) => root.template(reference).unwrap(),
        };

        let preview_frame = if let Some(start) = self.playback_start {
            let frame = (start.elapsed().as_secs_f32() * 60.0).floor() as usize;
            let animation_duration = template
                .animations
                .iter()
                .find(|(name, _)| *name == self.animation)
                .unwrap()
                .1
                .total_duration;
            if frame >= animation_duration {
                self.playback_start = None;
            }

            frame
        } else {
            self.current_keyframe
        };

        let mut tree = self.tree.lock();
        tree.sync_to_template(template, &root, &mut self.backend.lock());
        tree.sync_to_animation_keyframe(&self.animation, preview_frame);

        egui::Frame::canvas(ui.style()).show(ui, |ui| {
            let (rect, _response) =
                ui.allocate_exact_size(ui.available_size(), egui::Sense::drag());

            ui.painter()
                .with_clip_rect(rect)
                .add(egui_wgpu::Callback::new_paint_callback(
                    rect,
                    LayoutPaintCallback {
                        reference: PaintingReference::Tree(self.tree.clone()),
                        pipeline: self.pipeline.clone(),
                        backend: self.backend.clone(),
                    },
                ));
        });
    }
}
