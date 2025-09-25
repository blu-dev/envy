use std::{
    path::{Path, PathBuf},
    sync::{Arc, atomic::AtomicU64},
};

use envy::{LayoutRoot, LayoutTemplate, NodeImplTemplate};
use envy_wgpu::WgpuBackend;
use parking_lot::Mutex;

use crate::widgets::{
    animator::AnimatorWidget,
    layout_renderer::{LayoutReference, LayoutRenderer, LayoutRendererCommand},
    resource_viewer::{ResourceKind, ResourceViewer, ResourceViewerCommand},
};

mod widgets;

pub struct AppTabWrapper {
    id: u64,
    inner: AppTab,
}

impl AppTabWrapper {
    pub fn new(tab: AppTab) -> Self {
        static ATOMIC: AtomicU64 = AtomicU64::new(0);
        Self {
            id: ATOMIC.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            inner: tab,
        }
    }
}

pub enum AppTab {
    Animator(AnimatorWidget),
    Layout(LayoutRenderer),
    Resource(ResourceViewer),
}

pub struct NewTab {
    tab: AppTab,
    surface: egui_dock::SurfaceIndex,
    node: egui_dock::NodeIndex,
}

pub struct AppTabViewer<'a> {
    root: Arc<Mutex<LayoutRoot<WgpuBackend>>>,
    backend: Arc<Mutex<WgpuBackend>>,
    new_tab: Option<NewTab>,
    layout_commands: Vec<LayoutRendererCommand>,
    resource_commands: Vec<ResourceViewerCommand>,
    render_state: &'a egui_wgpu::RenderState,
}

impl egui_dock::TabViewer for AppTabViewer<'_> {
    type Tab = AppTabWrapper;

    fn id(&mut self, tab: &mut Self::Tab) -> egui::Id {
        egui::Id::new(tab.id)
    }

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        match &mut tab.inner {
            AppTab::Animator(_) => "Animator".into(),
            AppTab::Layout(layout) => match layout.reference() {
                LayoutReference::Root => "Root Layout".into(),
                LayoutReference::Sublayout(name) => name.into(),
            },
            AppTab::Resource(viewer) => match viewer.kind() {
                ResourceKind::Images => "Image Resources".into(),
                ResourceKind::Fonts => "Font Resources".into(),
            },
        }
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        [false; 2]
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        match &mut tab.inner {
            AppTab::Animator(animator) => animator.show(ui),
            AppTab::Layout(layout) => self.layout_commands.extend(layout.show(ui)),
            AppTab::Resource(viewer) => self
                .resource_commands
                .extend(viewer.show(ui, self.render_state)),
        }
    }

    fn add_popup(
        &mut self,
        ui: &mut egui::Ui,
        surface: egui_dock::SurfaceIndex,
        node: egui_dock::NodeIndex,
    ) {
        ui.set_min_width(120.0);
        ui.style_mut().visuals.button_frame = false;
        let root = self.root.lock();
        let templates = root
            .templates()
            .into_iter()
            .map(|(template, _)| template.to_string())
            .collect::<Vec<_>>();
        drop(root);

        ui.menu_button("Open", |ui| {
            ui.style_mut().visuals.button_frame = false;

            if ui.button("Root Layout").clicked() {
                self.new_tab = Some(NewTab {
                    tab: AppTab::Layout(LayoutRenderer::new(
                        self.root.clone(),
                        self.backend.clone(),
                        self.render_state,
                        LayoutReference::Root,
                    )),
                    surface,
                    node,
                });
                ui.close();
                return;
            }

            if ui.button("Animator").clicked() {
                self.new_tab = Some(NewTab {
                    tab: AppTab::Animator(AnimatorWidget::new(
                        self.root.clone(),
                        self.backend.clone(),
                        self.render_state,
                    )),
                    surface,
                    node,
                });
                ui.close();
                return;
            }

            for template in templates
                .into_iter()
                .filter(|template| !template.is_empty())
            {
                if ui.button(&template).clicked() {
                    self.new_tab = Some(NewTab {
                        tab: AppTab::Layout(LayoutRenderer::new(
                            self.root.clone(),
                            self.backend.clone(),
                            self.render_state,
                            LayoutReference::Sublayout(template.as_str()),
                        )),
                        surface,
                        node,
                    });
                    ui.close();
                    break;
                }
            }
        });

        ui.menu_button("Resources", |ui| {
            ui.style_mut().visuals.button_frame = false;

            if ui.button("Images").clicked() {
                self.new_tab = Some(NewTab {
                    tab: AppTab::Resource(ResourceViewer::new(
                        self.backend.clone(),
                        ResourceKind::Images,
                    )),
                    surface,
                    node,
                });
                ui.close();
                return;
            }

            if ui.button("Fonts").clicked() {
                self.new_tab = Some(NewTab {
                    tab: AppTab::Resource(ResourceViewer::new(
                        self.backend.clone(),
                        ResourceKind::Fonts,
                    )),
                    surface,
                    node,
                });
                ui.close();
            }
        });

        if !ui.should_close() && ui.button("New Sublayout").clicked() {
            {
                self.root
                    .lock()
                    .add_template("new_template", LayoutTemplate::default());
            }
            self.new_tab = Some(NewTab {
                tab: AppTab::Layout(LayoutRenderer::new(
                    self.root.clone(),
                    self.backend.clone(),
                    self.render_state,
                    LayoutReference::Sublayout("new_template"),
                )),
                surface,
                node,
            });
            ui.close();
        }
    }
}

pub struct Application {
    dock_state: egui_dock::DockState<AppTabWrapper>,
    backend: Arc<Mutex<WgpuBackend>>,
    root: Arc<Mutex<LayoutRoot<WgpuBackend>>>,
    file_path: Option<PathBuf>,
}

impl Application {
    pub fn open(wgpu_render_state: &egui_wgpu::RenderState, path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();
        let mut backend = WgpuBackend::new(
            wgpu_render_state.device.clone(),
            wgpu_render_state.queue.clone(),
            wgpu_render_state.target_format,
            widgets::layout_renderer::SAMPLE_COUNT as usize,
        );

        let file_data = std::fs::read(path).unwrap();
        let mut root = envy::asset::deserialize(&mut backend, &file_data);
        root.setup(&mut backend);

        let templates = root
            .templates()
            .into_iter()
            .map(|(name, _)| name)
            .filter(|name| !name.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>();

        let backend = Arc::new(Mutex::new(backend));
        let root = Arc::new(Mutex::new(root));

        let mut tabs = vec![AppTabWrapper::new(AppTab::Layout(LayoutRenderer::new(
            root.clone(),
            backend.clone(),
            wgpu_render_state,
            LayoutReference::Root,
        )))];

        for template in templates {
            tabs.push(AppTabWrapper::new(AppTab::Layout(LayoutRenderer::new(
                root.clone(),
                backend.clone(),
                wgpu_render_state,
                LayoutReference::Sublayout(template.as_str()),
            ))));
        }

        let dock_state = egui_dock::DockState::new(tabs);

        Self {
            dock_state,
            backend,
            root,
            file_path: Some(path.to_path_buf()),
        }
    }

    pub fn new(wgpu_render_state: &egui_wgpu::RenderState) -> Self {
        let mut backend = WgpuBackend::new(
            wgpu_render_state.device.clone(),
            wgpu_render_state.queue.clone(),
            wgpu_render_state.target_format,
            widgets::layout_renderer::SAMPLE_COUNT as usize,
        );

        let mut root = LayoutRoot::new();
        root.setup(&mut backend);

        let backend = Arc::new(Mutex::new(backend));
        let root = Arc::new(Mutex::new(root));

        let tabs = vec![AppTabWrapper::new(AppTab::Layout(LayoutRenderer::new(
            root.clone(),
            backend.clone(),
            wgpu_render_state,
            LayoutReference::Root,
        )))];

        let dock_state = egui_dock::DockState::new(tabs);

        Self {
            dock_state,
            backend,
            root,
            file_path: None,
        }
    }
}

impl eframe::App for Application {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        ctx.request_repaint();
        egui::TopBottomPanel::top("file-bar").show(ctx, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("New").clicked() {
                    *self = Application::new(frame.wgpu_render_state().unwrap());
                    ui.close();
                } else if ui.button("Open").clicked() {
                    let file = rfd::FileDialog::new()
                        .set_title("Open Envy Layout File")
                        .add_filter("ENVY Layout File", &["envy"])
                        .pick_file();

                    if let Some(file) = file {
                        *self = Application::open(frame.wgpu_render_state().unwrap(), file);
                    }

                    ui.close();
                } else if ui.button("Save").clicked() {
                    if let Some(path) = self.file_path.as_ref() {
                        let root = self.root.lock();
                        let bytes = envy::asset::serialize(&root, &self.backend.lock());
                        std::fs::write(path, &bytes).unwrap();
                    } else {
                        let file = rfd::FileDialog::new()
                            .set_title("Open Envy Layout File")
                            .add_filter("ENVY Layout File", &["envy"])
                            .save_file();

                        if let Some(file) = file {
                            self.file_path = Some(file.clone());
                            let root = self.root.lock();
                            let bytes = envy::asset::serialize(&root, &self.backend.lock());
                            std::fs::write(&file, &bytes).unwrap();
                        }
                    }
                    ui.close();
                } else if ui.button("Save As").clicked() {
                    let file = rfd::FileDialog::new()
                        .set_title("Open Envy Layout File")
                        .add_filter("ENVY Layout File", &["envy"])
                        .save_file();

                    if let Some(file) = file {
                        self.file_path = Some(file.clone());
                        let root = self.root.lock();
                        let bytes = envy::asset::serialize(&root, &self.backend.lock());
                        std::fs::write(&file, &bytes).unwrap();
                    }
                    ui.close();
                }
            });
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            let mut viewer = AppTabViewer {
                root: self.root.clone(),
                backend: self.backend.clone(),
                new_tab: None,
                layout_commands: vec![],
                resource_commands: vec![],
                render_state: frame.wgpu_render_state().unwrap(),
            };

            egui_dock::DockArea::new(&mut self.dock_state)
                .style(egui_dock::Style::from_egui(ui.style().as_ref()))
                .show_add_buttons(true)
                .show_add_popup(true)
                .show_leaf_collapse_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_inside(ui, &mut viewer);

            for command in viewer.layout_commands {
                match command {
                    LayoutRendererCommand::RenameSublayout { old_name, new_name } => {
                        {
                            self.root
                                .lock()
                                .rename_template(old_name.as_str(), new_name.as_str());
                        }

                        for (_, tab) in self.dock_state.iter_all_tabs_mut() {
                            if let AppTab::Layout(renderer) = &mut tab.inner {
                                renderer.try_rename(old_name.as_str(), new_name.as_str());
                                renderer.reinit();
                            }
                        }
                    }
                    LayoutRendererCommand::MoveNode { old_path, new_path } => {
                        for (_, tab) in self.dock_state.iter_all_tabs_mut() {
                            if let AppTab::Animator(animator) = &mut tab.inner {
                                animator.try_move_node(&old_path, &new_path);
                            }
                        }
                    }
                    LayoutRendererCommand::RefreshSublayout { name, .. } => {
                        for (_, tab) in self.dock_state.iter_all_tabs_mut() {
                            if let AppTab::Layout(renderer) = &mut tab.inner
                                && let LayoutReference::Sublayout(reference) = renderer.reference()
                                && reference == name.as_str()
                            {
                                renderer.reinit();
                            }
                        }
                    }
                }
            }

            let mut reinit = false;
            for command in viewer.resource_commands {
                match command {
                    ResourceViewerCommand::Add { kind } => match kind {
                        ResourceKind::Fonts => {
                            if let Some(file) = rfd::FileDialog::new()
                                .set_title("Add Font Resource")
                                .add_filter("Font File", &["ttf", "otf"])
                                .pick_file()
                            {
                                let stem = file.file_stem().unwrap().to_string_lossy();
                                let file_data = std::fs::read(&file).unwrap();
                                self.backend.lock().add_font(stem, file_data);
                            }
                        }
                        ResourceKind::Images => {
                            if let Some(file) = rfd::FileDialog::new()
                                .set_title("Add Texture")
                                .add_filter("PNG File", &["png"])
                                .pick_file()
                            {
                                let stem = file.file_stem().unwrap().to_string_lossy().to_string();
                                let file_data = std::fs::read(file).unwrap();
                                self.backend.lock().add_texture(stem, &file_data);
                            }
                        }
                    },
                    ResourceViewerCommand::Rename {
                        kind,
                        old_name,
                        new_name,
                    } => {
                        let mut root = self.root.lock();
                        let mut backend = self.backend.lock();
                        let templates = root
                            .templates()
                            .into_iter()
                            .filter(|(name, _)| !name.is_empty())
                            .map(|(name, _)| name.to_string())
                            .collect::<Vec<_>>();
                        match kind {
                            ResourceKind::Fonts => {
                                backend.rename_font(&old_name, &new_name);
                                root.root_template_mut().walk_tree_mut(|node| {
                                    if let NodeImplTemplate::Text(text) = &mut node.implementation {
                                        if text.font_name == old_name {
                                            text.font_name = new_name.clone();
                                        }
                                    }
                                });

                                for template in templates {
                                    root.template_mut(&template).unwrap().walk_tree_mut(|node| {
                                        if let NodeImplTemplate::Text(text) =
                                            &mut node.implementation
                                        {
                                            if text.font_name == old_name {
                                                text.font_name = new_name.clone();
                                            }
                                        }
                                    });
                                }

                                root.sync_root_template(&mut backend);
                                reinit = true;
                            }
                            ResourceKind::Images => {
                                backend.rename_texture(&old_name, new_name.clone());

                                root.root_template_mut().walk_tree_mut(|node| {
                                    if let NodeImplTemplate::Image(image) = &mut node.implementation
                                    {
                                        if image.texture_name == old_name {
                                            image.texture_name = new_name.clone();
                                        }
                                    }
                                });

                                for template in templates {
                                    root.template_mut(&template).unwrap().walk_tree_mut(|node| {
                                        if let NodeImplTemplate::Image(image) =
                                            &mut node.implementation
                                        {
                                            if image.texture_name == old_name {
                                                image.texture_name = new_name.clone();
                                            }
                                        }
                                    });
                                }

                                root.sync_root_template(&mut backend);
                                reinit = true;
                            }
                        }
                    }
                    ResourceViewerCommand::Remove { kind, name } => {
                        let mut root = self.root.lock();
                        let mut backend = self.backend.lock();
                        let templates = root
                            .templates()
                            .into_iter()
                            .filter(|(name, _)| !name.is_empty())
                            .map(|(name, _)| name.to_string())
                            .collect::<Vec<_>>();
                        match kind {
                            ResourceKind::Fonts => {
                                backend.remove_font(&name);
                                root.root_template_mut().walk_tree_mut(|node| {
                                    if let NodeImplTemplate::Text(text) = &mut node.implementation {
                                        if text.font_name == name {
                                            text.font_name = "".to_string();
                                        }
                                    }
                                });

                                for template in templates {
                                    root.template_mut(&template).unwrap().walk_tree_mut(|node| {
                                        if let NodeImplTemplate::Text(text) =
                                            &mut node.implementation
                                        {
                                            if text.font_name == name {
                                                text.font_name = "".to_string();
                                            }
                                        }
                                    });
                                }

                                root.sync_root_template(&mut backend);
                                reinit = true;
                            }
                            ResourceKind::Images => {
                                backend.remove_texture(&name);
                                root.root_template_mut().walk_tree_mut(|node| {
                                    if let NodeImplTemplate::Image(image) = &mut node.implementation
                                    {
                                        if image.texture_name == name {
                                            image.texture_name = "".to_string();
                                        }
                                    }
                                });

                                for template in templates {
                                    root.template_mut(&template).unwrap().walk_tree_mut(|node| {
                                        if let NodeImplTemplate::Image(image) =
                                            &mut node.implementation
                                        {
                                            if image.texture_name == name {
                                                image.texture_name = "".to_string();
                                            }
                                        }
                                    });
                                }

                                root.sync_root_template(&mut backend);
                                reinit = true;
                            }
                        }
                    }
                    ResourceViewerCommand::Replace { kind, name } => match kind {
                        ResourceKind::Fonts => {
                            if let Some(file) = rfd::FileDialog::new()
                                .set_title("Add Font Resource")
                                .add_filter("Font File", &["ttf", "otf"])
                                .pick_file()
                            {
                                let file_data = std::fs::read(&file).unwrap();
                                let mut backend = self.backend.lock();
                                backend.add_font(name, file_data);
                                let mut root = self.root.lock();
                                root.sync_root_template(&mut backend);
                                reinit = true;
                            }
                        }
                        ResourceKind::Images => {
                            if let Some(file) = rfd::FileDialog::new()
                                .set_title("Add Texture")
                                .add_filter("PNG File", &["png"])
                                .pick_file()
                            {
                                let file_data = std::fs::read(file).unwrap();
                                let mut backend = self.backend.lock();
                                backend.add_texture(name, &file_data);
                                let mut root = self.root.lock();
                                root.sync_root_template(&mut backend);
                                reinit = true;
                            }
                        }
                    },
                }
            }

            if reinit {
                for (_, tab) in self.dock_state.iter_all_tabs_mut() {
                    if let AppTab::Layout(renderer) = &mut tab.inner {
                        renderer.reinit();
                    }
                }
            }

            if let Some(tab) = viewer.new_tab {
                self.dock_state.get_surface_mut(tab.surface).unwrap()[tab.node]
                    .append_tab(AppTabWrapper::new(tab.tab));
            }
        });
    }
}

fn main() {
    env_logger::init();

    eframe::run_native(
        "Envy Designer",
        eframe::NativeOptions::default(),
        Box::new(|ctx| {
            Ok(Box::new(Application::new(
                ctx.wgpu_render_state.as_ref().unwrap(),
            )))
        }),
    )
    .unwrap();
}
