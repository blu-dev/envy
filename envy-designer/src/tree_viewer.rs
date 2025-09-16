use std::hash::Hash;

use camino::{Utf8Path, Utf8PathBuf};
use egui::{
    Align2, Color32, CornerRadius, DragAndDrop, FontId, Id, ImageSource, Pos2, Rect, Sense, Stroke,
    Ui, Vec2, WidgetText,
};
use envy::{EnvyBackend, ImageNode, LayoutTemplate, LayoutTree, NodeImplTemplate, NodeItem, NodeTemplate, TextNode};

use crate::Icons;

struct ItemTreeNode<T> {
    universal_id: T,
    icon: Option<ImageSource<'static>>,
    label: WidgetText,
    children: Vec<ItemTreeNode<T>>,
}

pub struct ItemTreeBuilder<'a, T> {
    icon: &'a mut Option<ImageSource<'static>>,
    children: &'a mut Vec<ItemTreeNode<T>>,
}

impl<T> ItemTreeBuilder<'_, T> {
    pub fn set_icon(&mut self, icon: impl Into<ImageSource<'static>>) -> &mut Self {
        *self.icon = Some(icon.into());
        self
    }

    pub fn child(
        &mut self,
        id: T,
        label: impl Into<WidgetText>,
        f: impl FnOnce(ItemTreeBuilder<'_, T>),
    ) -> &mut Self {
        let mut node = ItemTreeNode {
            icon: None,
            label: label.into(),
            children: vec![],
            universal_id: id,
        };

        let builder = ItemTreeBuilder {
            icon: &mut node.icon,
            children: &mut node.children,
        };

        f(builder);

        self.children.push(node);

        self
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum ItemTreeCommand<T, U = ()> {
    RenameItem { id: T, new_name: String },
    DeleteItem(T),
    OpenItem(T),
    NewItem { parent: T, new_id: T },
    ExpandItemChildren(T),
    CollapseItemChildren(T),
    MoveItem { id: T, new_parent: T },
    MoveItemWithinParent { id: T, before: T },
    UserCommand { id: T, command: U },
}

pub struct ItemTree {
    id: Id,
}

impl ItemTree {
    pub fn new(id_source: impl Into<Id>) -> Self {
        Self {
            id: id_source.into(),
        }
    }
}

#[derive(Clone)]
enum DragPayload<T> {
    MoveToNewParent { guid: T, id: Id },
    MoveWithinParent { guid: T, parent_id: Id, id: Id },
}

impl<T> DragPayload<T> {
    fn id(&self) -> Id {
        match self {
            Self::MoveToNewParent { id, .. } => *id,
            Self::MoveWithinParent { id, .. } => *id,
        }
    }
}

impl ItemTree {
    fn show_recursive<T: Hash + Clone + Send + Sync + 'static, U>(
        ui: &mut Ui,
        parent_id: Id,
        children: Vec<ItemTreeNode<T>>,
        depth: usize,
        open_toggle_buttons: &mut Vec<(Rect, Id, T)>,
        commands: &mut Vec<ItemTreeCommand<T, U>>,
        ctx: &mut impl FnMut(&T, &mut Ui, Id) -> Option<ItemTreeCommand<T, U>>,
    ) {
        const DEPTH: f32 = 12.0;
        if children.is_empty() {
            return;
        }

        let mut lowest_middle = 0.0;
        let current = ui.cursor().min;
        let midpoint_x = current.x + DEPTH * (depth as f32 + 0.5);
        let painter = ui.painter().clone();
        for child in children {
            let id = parent_id.with(Id::new(&child.universal_id));

            let rect = ui
                .horizontal(|ui| {
                    ui.add_space(DEPTH * (depth + 1) as f32 + DEPTH / 2.0);

                    let renaming = id.with("renaming");
                    let rename_suffix = id.with("rename-suffix");

                    let rename_state = ui.data_mut(|data| data.remove_temp::<String>(renaming));
                    if let Some(mut rename_state) = rename_state {
                        let suffix = ui
                            .data_mut(|data| data.remove_temp::<String>(rename_suffix))
                            .unwrap_or_default();
                        rename_state = rename_state
                            .strip_suffix(&suffix)
                            .map(ToString::to_string)
                            .unwrap_or(rename_state);
                        let resp = ui.text_edit_singleline(&mut rename_state);
                        rename_state.push_str(&suffix);
                        if ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            commands.push(ItemTreeCommand::RenameItem {
                                id: child.universal_id.clone(),
                                new_name: rename_state,
                            })
                        } else if !(resp.lost_focus())
                            || ui.input(|i| i.key_pressed(egui::Key::Escape))
                        {
                            ui.data_mut(|data| data.insert_temp(renaming, rename_state));
                            ui.data_mut(|data| data.insert_temp(rename_suffix, suffix));
                        }
                        resp.request_focus();
                    } else {
                        let resp = match DragAndDrop::payload::<DragPayload<T>>(ui.ctx()) {
                            Some(payload) => {
                                if payload.id() == id {
                                    ui.dnd_drag_source(id, (*payload).clone(), |ui| {
                                        if let Some(icon) = child.icon {
                                            ui.image(icon);
                                        }
                                        ui.selectable_label(false, child.label.clone())
                                    })
                                    .inner
                                } else {
                                    match &*payload {
                                        DragPayload::MoveToNewParent { guid, .. } => {
                                            let (resp, payload) = ui
                                                .dnd_drop_zone::<DragPayload<T>, _>(
                                                    egui::Frame::default(),
                                                    |ui| {
                                                        if let Some(icon) = child.icon {
                                                            ui.image(icon);
                                                        }
                                                        ui.selectable_label(
                                                            false,
                                                            child.label.clone(),
                                                        )
                                                    },
                                                );

                                            if payload.is_some() {
                                                commands.push(ItemTreeCommand::MoveItem {
                                                    id: guid.clone(),
                                                    new_parent: child.universal_id.clone(),
                                                });
                                            }

                                            resp.inner
                                        }
                                        DragPayload::MoveWithinParent {
                                            parent_id: drag_parent_id,
                                            guid,
                                            ..
                                        } => {
                                            let (resp, payload) = if *drag_parent_id == parent_id {
                                                let (resp, payload) = ui
                                                    .dnd_drop_zone::<DragPayload<T>, _>(
                                                        egui::Frame::default(),
                                                        |ui| {
                                                            if let Some(icon) = child.icon {
                                                                ui.image(icon);
                                                            }
                                                            ui.selectable_label(
                                                                false,
                                                                child.label.clone(),
                                                            )
                                                        },
                                                    );

                                                (resp.inner, payload)
                                            } else {
                                                if let Some(icon) = child.icon {
                                                    ui.image(icon);
                                                }
                                                (
                                                    ui.selectable_label(false, child.label.clone()),
                                                    None,
                                                )
                                            };

                                            if payload.is_some() {
                                                commands.push(
                                                    ItemTreeCommand::MoveItemWithinParent {
                                                        id: guid.clone(),
                                                        before: child.universal_id.clone(),
                                                    },
                                                );
                                            }

                                            resp
                                        }
                                    }
                                }
                            }
                            None => {
                                if ui.input(|i| i.modifiers.command) {
                                    ui.dnd_drag_source(
                                        id,
                                        DragPayload::MoveToNewParent {
                                            guid: child.universal_id.clone(),
                                            id,
                                        },
                                        |ui| {
                                            if let Some(icon) = child.icon {
                                                ui.image(icon);
                                            }
                                            ui.selectable_label(false, child.label.clone())
                                        },
                                    )
                                    .inner
                                } else if ui.input(|i| i.modifiers.shift) {
                                    ui.dnd_drag_source(
                                        id,
                                        DragPayload::MoveWithinParent {
                                            guid: child.universal_id.clone(),
                                            parent_id,
                                            id,
                                        },
                                        |ui| {
                                            if let Some(icon) = child.icon {
                                                ui.image(icon);
                                            }
                                            ui.selectable_label(false, child.label.clone())
                                        },
                                    )
                                    .inner
                                } else {
                                    if let Some(icon) = child.icon {
                                        ui.image(icon);
                                    }
                                    ui.selectable_label(false, child.label.clone())
                                }
                            }
                        };

                        if resp.double_clicked() {
                            ui.data_mut(|data| {
                                data.insert_temp(renaming, child.label.text().to_string());
                            });
                        } else if resp.clicked() {
                            commands.push(ItemTreeCommand::OpenItem(child.universal_id.clone()));
                        }

                        resp.context_menu(|ui| {
                            if let Some(command) = ctx(&child.universal_id, ui, id) {
                                commands.push(command);
                                ui.close();
                            }
                        });
                    }
                })
                .response
                .rect;

            let y = rect.center().y;
            lowest_middle = y.max(lowest_middle);
            painter.line_segment(
                [
                    Pos2::new(midpoint_x, y),
                    Pos2::new(midpoint_x + DEPTH / 2.0, y),
                ],
                Stroke::new(1.0, Color32::DARK_GRAY),
            );

            if !child.children.is_empty() {
                let open_toggle_rect =
                    Rect::from_center_size(Pos2::new(midpoint_x, y), Vec2::new(12.0, 12.0));
                open_toggle_buttons.push((open_toggle_rect, id, child.universal_id));

                let is_open = id.with("is-open");
                if ui
                    .ctx()
                    .data(|data| data.get_temp::<bool>(is_open).unwrap_or_default())
                {
                    Self::show_recursive(
                        ui,
                        id,
                        child.children,
                        depth + 1,
                        open_toggle_buttons,
                        commands,
                        ctx,
                    );
                }
            }
        }

        if depth > 0 {
            painter.line_segment(
                [
                    Pos2::new(midpoint_x, current.y),
                    Pos2::new(midpoint_x, lowest_middle),
                ],
                Stroke::new(1.0, Color32::DARK_GRAY),
            );
        }
    }

    /// Shows this item tree
    ///
    /// # Arguments
    /// * `ui` - The UI to render this tree inside of
    /// * `generate_id` - Function to generate a new id if a new child is to be added to the tree
    /// * `f` - Function used to build the tree
    ///
    /// # Return
    /// Returns a list of commands that were emitted by the tree UI
    pub fn show<T: Hash + Clone + Send + Sync + 'static, U>(
        self,
        ui: &mut Ui,
        f: impl FnOnce(ItemTreeBuilder<'_, T>),
        mut ctx: impl FnMut(&T, &mut Ui, Id) -> Option<ItemTreeCommand<T, U>>,
    ) -> Vec<ItemTreeCommand<T, U>> {
        let mut icon = None;
        let mut root_children = vec![];

        let id = ui.id().with(self.id);

        f(ItemTreeBuilder {
            icon: &mut icon,
            children: &mut root_children,
        });

        let mut commands = vec![];
        let mut toggle_buttons = vec![];

        ui.vertical(|ui| {
            Self::show_recursive(
                ui,
                id,
                root_children,
                0,
                &mut toggle_buttons,
                &mut commands,
                &mut ctx,
            )
        });

        let painter = ui.painter().clone();
        for (open_toggle_rect, id, universal_id) in toggle_buttons {
            let open_toggle_resp = ui.allocate_rect(open_toggle_rect, Sense::click());
            let visuals = ui.style().interact(&open_toggle_resp);
            let is_open = id.with("is-open");
            painter.rect(
                open_toggle_rect,
                CornerRadius::same(1),
                visuals.bg_fill,
                visuals.bg_stroke,
                egui::StrokeKind::Middle,
            );

            let text = if ui
                .ctx()
                .data_mut(|data| *data.get_temp_mut_or_default::<bool>(is_open))
            {
                '-'
            } else {
                '+'
            };

            painter.text(
                open_toggle_rect.center(),
                Align2::CENTER_CENTER,
                text,
                FontId::monospace(12.0),
                visuals.text_color(),
            );

            if open_toggle_resp.clicked() {
                let command = ui.ctx().data_mut(|data| {
                    let flag = data.get_temp_mut_or_default::<bool>(is_open);
                    *flag = !*flag;
                    if *flag {
                        ItemTreeCommand::ExpandItemChildren(universal_id)
                    } else {
                        ItemTreeCommand::CollapseItemChildren(universal_id)
                    }
                });

                commands.push(command);
            }
        }

        commands
    }
}

fn visit_node(
    node: &NodeTemplate,
    icons: &Icons,
    mut builder: ItemTreeBuilder<'_, Utf8PathBuf>,
    current_path: &Utf8Path,
) {
    match &node.implementation {
        NodeImplTemplate::Empty => {
            builder.set_icon(icons.empty.clone());
        },
        NodeImplTemplate::Image(_) => {
            builder.set_icon(icons.texture.clone());
        },
        NodeImplTemplate::Text(_) => {
            builder.set_icon(icons.text.clone());
        },
        _ => {}
    }

    node.visit_children(move |node| {
        let path = current_path.join(&node.name);
        builder.child(path.clone(), &node.name, move |builder| {
            visit_node(node, icons, builder, &path);
        });
    });
}

pub enum TreeViewerCommand {
    MoveBackward,
    MoveForward,
}

pub fn show_tree_viewer(
    ui: &mut egui::Ui,
    icons: &super::Icons,
    tree: &LayoutTemplate,
) -> Vec<ItemTreeCommand<Utf8PathBuf, TreeViewerCommand>> {
    let item_tree = ItemTree::new(Id::new("asset_tree"));

    ui.push_id("sub-ui", |ui| {
        ui.spacing_mut().item_spacing.x /= 4.0;

        item_tree.show(
            ui,
            |mut builder| {
                tree.visit_roots(move |node| {
                    let path = Utf8Path::new(&node.name);
                    builder.child(path.to_path_buf(), &node.name, move |builder| {
                        visit_node(node, icons, builder, path);
                    });
                });
            },
            |id, ui, ui_id| {
                if ui.button("Remove").clicked() {
                    return Some(ItemTreeCommand::DeleteItem(id.clone()));
                } else if ui.button("Add Child Node").clicked() {
                    return Some(ItemTreeCommand::NewItem {
                        parent: id.clone(),
                        new_id: id.join("new_node"),
                    });
                } else if ui.button("Rename").clicked() {
                    ui.data_mut(|data| {
                        data.insert_temp(ui_id.with("renaming"), String::new());
                    });
                    ui.close();
                } else if ui.button("Move Back").clicked() {
                    return Some(ItemTreeCommand::UserCommand {
                        id: id.clone(),
                        command: TreeViewerCommand::MoveBackward,
                    });
                } else if ui.button("Move Forward").clicked() {
                    return Some(ItemTreeCommand::UserCommand {
                        id: id.clone(),
                        command: TreeViewerCommand::MoveForward,
                    });
                }

                None
            },
        )
    })
    .inner

    // for node in nodes.iter() {
    //     let _ = ui.selectable_label(false, &node.name);
    // }
}
