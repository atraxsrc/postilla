// SPDX-License-Identifier: GPL-3.0

use crate::app::app_menu::MenuAction;
use crate::app::context_page::ContextPage;
use crate::app::core::editor::EditorState;
use crate::app::core::preview::MarkdownPreview;
use crate::app::core::project::ProjectNode;
use crate::app::core::utils::search::SearchAction;
use crate::app::core::utils::stats::DocStats;
use crate::app::core::utils::vault_search::{VaultSearchAction, VaultSearchState};
use crate::app::core::utils::{self, CedillaToast, Image};
use crate::app::dialogs::{DialogPage, DialogState};
use crate::config::{
    AUTOSAVE_DELAYS, AppTheme, BoolState, CONFIG_VERSION, CedillaConfig, ConfigInput,
    FrostedGlass, MIN_WINDOW_OPACITY, ShowState, ViewMode, autosave_labels,
};
use crate::key_binds::key_binds;
use crate::syntax_theme::ThemeRegistry;
use crate::{fl, icons};
use cosmic::app::context_drawer;
use cosmic::cosmic_config::Update;
use cosmic::cosmic_theme::{self, ThemeMode};
use cosmic::iced::core::keyboard::{Key, Modifiers};
use cosmic::iced::core::window;
use cosmic::iced::widget::{center, column, row, scrollable, tooltip};
use cosmic::iced::{Alignment, Color, Event, Font, Length, Padding, Subscription};
use cosmic::widget::space::horizontal;
use cosmic::widget::{self, about::About, menu};
use cosmic::widget::{
    ToastId, Toasts, button, container, nav_bar, pane_grid, responsive, segmented_button, text,
    text_input, toaster,
};
use cosmic::{cosmic_config, prelude::*, surface, theme};
use frostmark::{MarkWidget, UpdateMsg};
use slotmap::Key as SlotMapKey;
use std::any::TypeId;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use widgets::highlighter::{self as hl, ThemeRef};
use widgets::{TextEditor, text_editor};

pub mod app_menu;
mod context_page;
mod core;
mod dialogs;
mod update;

const REPOSITORY: &str = env!("CARGO_PKG_REPOSITORY");

/// Unique identifier in RDNN (reverse domain name notation) format.
///
/// This fork uses its own id so it can be installed and run next to the
/// upstream (Flathub) Cedilla: separate config, vault, themes folder,
/// single-instance D-Bus name and launcher entry. Keep it in sync with the
/// `appid` in the `justfile`, `resources/app.desktop`,
/// `resources/app.metainfo.xml` and the Flatpak manifest.
pub const APP_ID: &str = "dev.mariinkys.Cedilla.Fork";

/// The application model stores app-specific state used to describe its interface and
/// drive its logic.
pub struct AppModel {
    /// Application toasts
    toasts: Toasts<Message>,
    /// Application state which is managed by the COSMIC runtime.
    core: cosmic::Core,
    /// Application navbar
    nav_model: segmented_button::SingleSelectModel,
    /// Needed for navbar context menu func
    nav_bar_context_id: segmented_button::Entity,
    /// Dialog Pages of the Application
    dialog_pages: VecDeque<DialogPage>,
    /// Holds the state of the application dialogs
    dialog_state: DialogState,
    /// Display a context drawer with the designated page if defined.
    context_page: ContextPage,
    /// The about page for this app.
    about: About,
    /// Key bindings for the application's menu bar.
    key_binds: HashMap<menu::KeyBind, MenuAction>,
    /// Application Keyboard Modifiers
    modifiers: Modifiers,
    /// Application configuration handler
    config_handler: Option<cosmic::cosmic_config::Config>,
    /// Configuration data that persists between application runs.
    config: CedillaConfig,
    // Application Themes
    app_themes: Vec<String>,
    // Fonts available in the system
    system_fonts: Vec<String>,
    /// Editor and preview fonts
    cedilla_font: Font,
    /// Syntax highlighting themes (built-in + user themes folder)
    syntax_themes: ThemeRegistry,
    /// Display names of the syntax themes, in registry order (for dropdowns)
    syntax_theme_names: Vec<String>,
    /// Currently selected path on the navbar (i need these for accurate file creadtion deletion...)
    selected_nav_path: Option<PathBuf>,
    /// Model of the view mode switcher in the header bar
    view_mode_model: segmented_button::SingleSelectModel,
    /// Distraction free mode (hides every bar around the editor)
    zen_mode: bool,
    /// Nav bar state to restore when leaving zen mode
    zen_restore_nav: bool,
    /// Incremented every time an autosave is scheduled so stale timers are ignored
    autosave_generation: u64,
    /// The next save result comes from an autosave: don't show a toast for it
    silent_save: bool,
    /// Line to jump to after the next file is opened (vault search results)
    pending_jump_line: Option<usize>,
    /// State of the "search in vault" context page
    vault_search: VaultSearchState,
    /// System clipboard (kept alive so copied text outlives the copy action)
    clipboard: Option<arboard::Clipboard>,
    /// Application State
    state: State,
}

/// Represents the Application State
#[allow(clippy::large_enum_variant)]
enum State {
    Loading,
    Ready {
        /// Holds the state for editor related fields
        editor: EditorState,
        /// Holds the state for preview related fields
        preview: MarkdownPreview,
        /// Pane grid state
        panes: pane_grid::State<PaneContent>,
        /// Layout of the main view (editor / split / preview)
        view_mode: ViewMode,
    },
}

/// Content type for each pane
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaneContent {
    Editor,
    Preview,
}

/// Possible actions after asking to discard changes in the warning [`DialogPage`]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscardChangesAction {
    CloseApp,
    OpenFile(PathBuf),
}

/// Clipboard operations on the text editor
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardAction {
    Cut,
    Copy,
    Paste,
    SelectAll,
}

/// Messages emitted by the application and its widgets.
#[derive(Debug, Clone)]
pub enum Message {
    /// Callback for closing a toast
    CloseToast(ToastId),
    /// Ask to add new toast
    AddToast(CedillaToast),
    /// Opens the given URL in the browser
    LaunchUrl(String),
    /// Copies some data to the system clipboard
    CopyToClipboard(String),
    /// Opens (or closes if already open) the given [`ContextPage`]
    ToggleContextPage(ContextPage),
    /// Needed for responsive menu bar
    Surface(surface::Action<Message>),
    /// Executes the appropiate cosmic binding on keyboard shortcut
    Key(Modifiers, Key),
    /// Updates the current state of keyboard modifiers
    Modifiers(Modifiers),

    /// Callback after clicking something in the app menu
    MenuAction(app_menu::MenuAction),

    /// Asks to execute various actions related to the application dialogs
    DialogAction(dialogs::DialogAction),
    /// Right click on a NavBar Item
    NavBarContext(segmented_button::Entity),
    /// Fired when a menu item is chosen
    NavMenuAction(NavMenuAction),

    /// Startup Message, checks config and set's the state as needed
    Startup(Option<PathBuf>),
    /// Creates a new empty file (no path)
    NewFile,
    /// Creates a new markdown file in the vault
    NewVaultFile(String),
    /// Creates a new folder in the vault
    NewVaultFolder(String),
    /// Save the current file
    SaveFile,
    /// Callback after opening a new file
    OpenFile(Result<(PathBuf, Arc<String>), anywho::Error>),
    /// Callback after saving the current file
    FileSaved(Option<Result<PathBuf, anywho::Error>>),
    /// Callback after asking to close a file discarding changes
    DiscardChanges(DiscardChangesAction),
    /// Fired when the watcher detects an external change to the open file
    ExternalFileChanged(PathBuf),
    /// Autosave timer fired (generation of the timer)
    AutosaveTick(u64),

    /// Deletes the given node entity of the navbar folder or file
    DeleteNode(cosmic::widget::segmented_button::Entity),
    /// Renames the given node entity of the navbar folder or file
    RenameNode(cosmic::widget::segmented_button::Entity, String),
    /// Move one node (to another one)
    MoveNode(cosmic::widget::segmented_button::Entity, PathBuf),
    /// Ask to move the main vault to another path
    MoveVault,
    /// Ask to use an existing folder as the vault (nothing is moved)
    OpenVault,
    /// Callback after asking to move the vault
    VaultMoved(Result<PathBuf, anywho::Error>),

    /// Callback after some action is performed on the text editor
    Edit(text_editor::Action),
    /// Apply formatting to selected text
    ApplyFormatting(utils::SelectionAction),
    /// Undo requested
    Undo,
    /// Redo requested
    Redo,
    /// Search related action requested
    Search(SearchAction),
    /// Search across the vault
    VaultSearch(VaultSearchAction),
    /// Smart paste (pastes images as Markdown or standard text)
    SmartPaste,
    /// Text read from the system clipboard (fallback paste path)
    PastedText(Option<String>),
    /// Cut / copy / paste / select all on the editor
    Clipboard(ClipboardAction),
    /// A checkbox in the preview was clicked (index in the document, new value)
    ToggleCheckbox(usize, bool),
    /// Move the cursor to the given (zero based) line and scroll to it
    JumpToLine(usize),

    /// Update the HTML renderer state
    UpdateMarkState(UpdateMsg),
    /// Callback after a Markdown image has been downloaded
    ImageDownloaded(Result<Image, anywho::Error>),
    /// Set's the layout of the main view
    SetViewMode(ViewMode),
    /// A view mode was picked on the header bar switcher
    ViewModeSelected(segmented_button::Entity),
    /// Toggle the distraction free mode
    ToggleZenMode,
    /// Pane grid resized callback
    PaneResized(pane_grid::ResizeEvent),
    /// Pane grid dragged callback
    PaneDragged(pane_grid::DragEvent),
    /// Scrollable position changed
    ScrollChanged(widget::Id, scrollable::Viewport),

    /// Callback after input on the Config [`ContextPage`]
    ConfigInput(ConfigInput),

    /// Open the given path in the default file explorer
    OpenInFileExplorer(PathBuf),

    /// Export current document to PDF
    ExportPDF,
    /// Export current document to a standalone HTML file
    ExportHtml,
    /// Copy the current document as HTML
    CopyHtml,
    /// Callback after using asks to close the app
    AppCloseRequested,
    /// Something happened on the tray icon
    Tray(crate::tray::TrayEvent),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[allow(clippy::enum_variant_names)]
pub enum NavMenuAction {
    DeleteNode(segmented_button::Entity),
    RenameNode(segmented_button::Entity),
    MoveNode(segmented_button::Entity),
    OpenNodeFileManager(segmented_button::Entity),
    OpenFolderCreationDialog(segmented_button::Entity),
    OpenVaultFileCreationDialog(segmented_button::Entity),
}

impl cosmic::widget::menu::Action for NavMenuAction {
    type Message = cosmic::Action<Message>;
    fn message(&self) -> Self::Message {
        cosmic::Action::App(Message::NavMenuAction(*self))
    }
}

/// Create a COSMIC application from the app model
impl cosmic::Application for AppModel {
    /// The async executor that will be used to run your application's commands.
    type Executor = cosmic::executor::Default;

    /// Data that your application receives to its init method.
    type Flags = crate::flags::Flags;

    /// Messages which the application and its widgets will emit.
    type Message = Message;

    /// Unique identifier in RDNN (reverse domain name notation) format.
    const APP_ID: &'static str = APP_ID;

    fn core(&self) -> &cosmic::Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut cosmic::Core {
        &mut self.core
    }

    /// Initializes the application with any given flags and startup commands.
    fn init(core: cosmic::Core, flags: Self::Flags) -> (Self, Task<cosmic::Action<Self::Message>>) {
        // Create the about widget
        let about = About::default()
            .name("Cedilla")
            .icon(widget::icon::from_name(Self::APP_ID))
            .version(env!("CARGO_PKG_VERSION"))
            .links([
                (fl!("repository"), REPOSITORY),
                (fl!("support"), &format!("{}/issues", REPOSITORY)),
                (fl!("donations"), "https://buymeacoffee.com/mariinkys"),
            ])
            .license(env!("CARGO_PKG_LICENSE"))
            .author("mariinkys")
            .developers([("mariinkys", "kysdev.owjga@aleeas.com")])
            .comments("\"Pop Icons\" by System76 is licensed under CC-SA-4.0");

        let font = if let Some(ref name) = flags.config.selected_font_family {
            let static_name: &'static str = Box::leak(name.clone().into_boxed_str());
            Font {
                family: cosmic::iced::font::Family::Name(static_name),
                weight: cosmic::iced::font::Weight::Normal,
                stretch: cosmic::iced::font::Stretch::Normal,
                style: cosmic::iced::font::Style::Normal,
            }
        } else {
            Font::DEFAULT
        };

        let mut font_options = vec![fl!("default-font")];
        font_options.extend(flags.system_fonts.iter().cloned());

        let syntax_themes = ThemeRegistry::load();
        let syntax_theme_names = syntax_themes.names();

        let view_mode_model = view_mode_model(flags.config.last_view_mode);

        let clipboard = match arboard::Clipboard::new() {
            Ok(clipboard) => Some(clipboard),
            Err(err) => {
                eprintln!("System clipboard unavailable, falling back to the toolkit clipboard: {err}");
                None
            }
        };

        // Construct the app model with the runtime's core.
        let mut app = AppModel {
            toasts: Toasts::new(Message::CloseToast),
            core,
            nav_model: nav_bar::Model::builder().build(),
            nav_bar_context_id: segmented_button::Entity::null(),
            context_page: ContextPage::default(),
            dialog_pages: VecDeque::default(),
            dialog_state: DialogState::default(),
            about,
            key_binds: key_binds(),
            modifiers: Modifiers::empty(),
            config_handler: flags.config_handler,
            config: flags.config,
            app_themes: vec![fl!("match-desktop"), fl!("dark"), fl!("light")],
            system_fonts: font_options,
            cedilla_font: font,
            syntax_themes,
            syntax_theme_names,
            selected_nav_path: None,
            view_mode_model,
            zen_mode: false,
            zen_restore_nav: true,
            autosave_generation: 0,
            silent_save: false,
            pending_jump_line: None,
            vault_search: VaultSearchState::default(),
            clipboard,
            state: State::Loading,
        };

        // restore last navbar app state
        match app.config.last_navbar_showstate {
            ShowState::Show => app.core.nav_bar_set_toggled(true),
            ShowState::Hide => app.core.nav_bar_set_toggled(false),
        }

        // load vault
        let vault_path = PathBuf::from(&app.config.vault_path);
        if vault_path.exists() && vault_path.is_dir() {
            app.open_vault_folder(&vault_path);
        } else {
            eprintln!("Vault not found, trying to fallback to default path");
            let default_vault_path = PathBuf::from(CedillaConfig::default().vault_path);

            #[allow(clippy::collapsible_if)]
            if let Some(handler) = &app.config_handler {
                if let Err(err) = app
                    .config
                    .set_vault_path(handler, default_vault_path.to_string_lossy().to_string())
                {
                    eprintln!("{err}");
                }
            }

            app.config.vault_path = default_vault_path.to_string_lossy().to_string();
            app.open_vault_folder(&default_vault_path);
        }

        // Startup tasks.
        let theme_task = app.apply_theme();
        let tasks = vec![
            app.update_title(),
            theme_task,
            Task::done(cosmic::action::app(Message::Startup(flags.open_with_file))),
        ];

        (app, Task::batch(tasks))
    }

    /// Elements to pack at the start of the header bar.
    fn header_start(&self) -> Vec<Element<'_, Self::Message>> {
        let view_mode = match &self.state {
            State::Ready { view_mode, .. } => *view_mode,
            State::Loading => self.config.last_view_mode,
        };

        vec![app_menu::menu_bar(
            &self.key_binds,
            app_menu::ViewState {
                view_mode,
                zen_mode: self.zen_mode,
                sidebar: if self.zen_mode {
                    self.zen_restore_nav
                } else {
                    self.core.nav_bar_active()
                },
                help_bar: self.config.show_helper_header_bar.is_shown(),
                status_bar: self.config.show_status_bar.is_shown(),
            },
        )]
    }

    /// Elements to pack at the end of the header bar.
    fn header_end(&self) -> Vec<Element<'_, Self::Message>> {
        let State::Ready { .. } = &self.state else {
            return vec![];
        };

        let view_switcher = tooltip(
            widget::segmented_control::horizontal(&self.view_mode_model)
                .button_height(28)
                .width(Length::Shrink)
                .on_activate(Message::ViewModeSelected),
            container(text(format!("{} · Ctrl+1 / Ctrl+2 / Ctrl+3", fl!("view"))))
                .class(cosmic::theme::Container::Background)
                .padding(8.),
            tooltip::Position::Bottom,
        );

        let zen_button = tooltip(
            widget::button::icon(icons::get_handle("zen-symbolic", 18))
                .on_press(Message::ToggleZenMode)
                .class(if self.zen_mode {
                    theme::Button::Suggested
                } else {
                    theme::Button::Icon
                }),
            container(text(format!("{} · F11", fl!("zen-mode"))))
                .class(cosmic::theme::Container::Background)
                .padding(8.),
            tooltip::Position::Bottom,
        );

        let settings_button = tooltip(
            widget::button::icon(icons::get_handle("settings-symbolic", 18))
                .on_press(Message::ToggleContextPage(ContextPage::Settings))
                .class(theme::Button::Icon),
            container(text(format!("{} · Ctrl+,", fl!("settings"))))
                .class(cosmic::theme::Container::Background)
                .padding(8.),
            tooltip::Position::Bottom,
        );

        let file_manager_button = tooltip(
            widget::button::icon(icons::get_handle("folder-open-symbolic", 18))
                .on_press(Message::OpenInFileExplorer(self.config.vault_path()))
                .class(theme::Button::Icon),
            container(text(fl!("open-file-manager")))
                .class(cosmic::theme::Container::Background)
                .padding(8.),
            tooltip::Position::Bottom,
        );

        vec![
            row![view_switcher, zen_button, file_manager_button, settings_button]
                .spacing(cosmic::theme::spacing().space_xxs)
                .align_y(Alignment::Center)
                .into(),
        ]
    }

    fn nav_bar(&self) -> Option<Element<'_, cosmic::action::Action<Message>>> {
        if !self.core().nav_bar_active() || self.zen_mode {
            return None;
        }

        let nav_model = self.nav_model()?;

        // if no valid items exist, render nothing
        nav_model.iter().next()?;

        let cosmic::cosmic_theme::Spacing {
            space_none,
            space_s,
            space_xxxs,
            ..
        } = self.core().system_theme().cosmic().spacing;

        let mut nav = segmented_button::vertical(nav_model)
            .button_height(space_xxxs + 20 /* line height */ + space_xxxs)
            .button_padding([space_s, space_xxxs, space_s, space_xxxs])
            .button_spacing(space_xxxs)
            .on_activate(|entity| cosmic::action::cosmic(cosmic::app::Action::NavBar(entity)))
            .on_context(|entity| cosmic::Action::App(Message::NavBarContext(entity)))
            .context_menu(self.nav_context_menu())
            .spacing(space_none)
            .style(theme::SegmentedButton::FileNav)
            .apply(widget::container)
            .padding(space_s)
            .width(Length::Shrink);

        if !self.core().is_condensed() {
            nav = nav.max_width(280);
            nav = nav.width(Length::Fixed(280.)); // remove if it ever get's fixed (needed right now in iced-rebase branch)
        }

        Some(
            nav.apply(widget::scrollable)
                .apply(widget::container)
                .height(Length::Fill)
                .class(theme::Container::custom(nav_bar::nav_bar_style))
                .into(),
        )
    }

    fn nav_model(&self) -> Option<&nav_bar::Model> {
        Some(&self.nav_model)
    }

    fn nav_context_menu(
        &self
    ) -> Option<Vec<widget::menu::Tree<cosmic::Action<Message>>>> {
        // All of the weird code with the ButtonDisabled stuff is because there seems to be many issues
        // with menu_inner crashing the app if there are more items in some context menus so all menus
        // must have the same number of items
        Some(cosmic::widget::menu::nav_context(
            &std::collections::HashMap::new(),
            self.nav_model
                .iter()
                .map(|e| {
                    // only one entry for root node
                    if self.nav_model.indent(e).unwrap_or(0) == 0 {
                        vec![
                            cosmic::widget::menu::Item::ButtonDisabled(
                                fl!("delete"),
                                None,
                                NavMenuAction::DeleteNode(e),
                            ),
                            cosmic::widget::menu::Item::ButtonDisabled(
                                fl!("rename"),
                                None,
                                NavMenuAction::RenameNode(e),
                            ),
                            cosmic::widget::menu::Item::ButtonDisabled(
                                fl!("move-to"),
                                None,
                                NavMenuAction::MoveNode(e),
                            ),
                            cosmic::widget::menu::Item::Button(
                                fl!("open-file-manager"),
                                None,
                                NavMenuAction::OpenNodeFileManager(e),
                            ),
                            cosmic::widget::menu::Item::Button(
                                fl!("new-folder"),
                                None,
                                NavMenuAction::OpenFolderCreationDialog(e),
                            ),
                            cosmic::widget::menu::Item::Button(
                                fl!("new-vault-file"),
                                None,
                                NavMenuAction::OpenVaultFileCreationDialog(e),
                            ),
                        ]
                    } else {
                        vec![
                            cosmic::widget::menu::Item::Button(
                                fl!("delete"),
                                None,
                                NavMenuAction::DeleteNode(e),
                            ),
                            cosmic::widget::menu::Item::Button(
                                fl!("rename"),
                                None,
                                NavMenuAction::RenameNode(e),
                            ),
                            cosmic::widget::menu::Item::Button(
                                fl!("move-to"),
                                None,
                                NavMenuAction::MoveNode(e),
                            ),
                            cosmic::widget::menu::Item::Button(
                                fl!("open-file-manager"),
                                None,
                                NavMenuAction::OpenNodeFileManager(e),
                            ),
                            // If the entity is a file both creation dialogs will store the new folder/file in the parent folder
                            cosmic::widget::menu::Item::Button(
                                fl!("new-folder"),
                                None,
                                NavMenuAction::OpenFolderCreationDialog(e),
                            ),
                            cosmic::widget::menu::Item::Button(
                                fl!("new-vault-file"),
                                None,
                                NavMenuAction::OpenVaultFileCreationDialog(e),
                            ),
                        ]
                    }
                })
                .collect(),
        ))
    }

    fn on_nav_select(&mut self, id: nav_bar::Id) -> Task<cosmic::Action<Message>> {
        let State::Ready { editor, .. } = &self.state else {
            return Task::none();
        };

        let node_opt = match self.nav_model.data_mut::<ProjectNode>(id) {
            Some(node) => {
                if let ProjectNode::Folder { open, .. } = node {
                    *open = !*open;
                }
                Some(node.clone())
            }
            None => None,
        };

        match node_opt {
            Some(node) => {
                self.nav_model.icon_set(id, node.icon(18));

                match node {
                    ProjectNode::Folder { path, open, .. } => {
                        // store selected directory
                        self.selected_nav_path = Some(path.clone());

                        let position = self.nav_model.position(id).unwrap_or(0);
                        let indent = self.nav_model.indent(id).unwrap_or(0);

                        if open {
                            self.open_folder(&path, position + 1, indent + 1);
                        } else {
                            while let Some(child_id) = self.nav_model.entity_at(position + 1) {
                                if self.nav_model.indent(child_id).unwrap_or(0) > indent {
                                    self.nav_model.remove(child_id);
                                } else {
                                    break;
                                }
                            }
                        }
                        Task::none()
                    }
                    ProjectNode::File { path, .. } => {
                        if editor.path.as_ref() == Some(&path) {
                            return Task::none();
                        }

                        if editor.is_dirty {
                            if editor.needs_confirmation() {
                                Task::done(cosmic::action::app(Message::DialogAction(
                                    dialogs::DialogAction::OpenConfirmCloseFileDialog(
                                        DiscardChangesAction::OpenFile(path),
                                    ),
                                )))
                            } else {
                                Task::perform(utils::files::load_file(path), |res| {
                                    cosmic::action::app(Message::OpenFile(res))
                                })
                            }
                        } else {
                            Task::perform(utils::files::load_file(path), |res| {
                                cosmic::action::app(Message::OpenFile(res))
                            })
                        }
                    }
                }
            }
            None => Task::none(),
        }
    }

    /// Display a dialog if requested.
    fn dialog(&self) -> Option<Element<'_, Message>> {
        let dialog_page = self.dialog_pages.front()?;
        dialog_page.display(&self.dialog_state)
    }

    /// Display a context drawer if the context page is requested.
    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Self::Message>> {
        if !self.core.window.show_context {
            return None;
        }

        self.context_page.display(self)
    }

    /// Describes the interface based on the current state of the application model.
    fn view(&self) -> Element<'_, Self::Message> {
        let content: Element<_> = match &self.state {
            State::Loading => center(text(fl!("loading"))).into(),
            State::Ready {
                editor,
                preview,
                panes,
                view_mode,
            } => cedilla_main_view(MainViewParams {
                app_config: &self.config,
                editor,
                preview,
                panes,
                view_mode: *view_mode,
                zen_mode: self.zen_mode,
                font: self.cedilla_font,
                syntax_theme: self.active_syntax_theme(),
                theme_colors: self.config.highlighter_theme_colors.is_yes(),
                key_binds: &self.key_binds,
            }),
        };

        toaster(&self.toasts, container(content).center(Length::Fill))
    }

    /// Register subscriptions for this application.
    fn subscription(&self) -> Subscription<Self::Message> {
        struct ConfigSubscription;
        struct ThemeSubscription;

        let watched_path = match &self.state {
            State::Ready { editor, .. } => editor.path.clone(),
            _ => None,
        };

        // Add subscriptions which are always active.
        let subscriptions = vec![
            // Watch for key_bind inputs
            cosmic::iced::event::listen_with(|event, status, _| match event {
                Event::Keyboard(cosmic::iced::keyboard::Event::KeyPressed {
                    key,
                    modifiers,
                    ..
                }) => match status {
                    cosmic::iced::event::Status::Ignored => Some(Message::Key(modifiers, key)),
                    cosmic::iced::event::Status::Captured => None,
                },
                Event::Keyboard(cosmic::iced::keyboard::Event::ModifiersChanged(modifiers)) => {
                    Some(Message::Modifiers(modifiers))
                }
                Event::Window(window::Event::CloseRequested) => Some(Message::AppCloseRequested),
                _ => None,
            }),
            cosmic_config::config_subscription(
                TypeId::of::<ConfigSubscription>(),
                Self::APP_ID.into(),
                CONFIG_VERSION,
            )
            .map(|update: Update<ThemeMode>| {
                if !update.errors.is_empty() {
                    eprintln!(
                        "errors loading config {:?}: {:?}",
                        update.keys, update.errors
                    );
                }
                Message::ConfigInput(ConfigInput::SystemThemeModeChange)
            }),
            cosmic_config::config_subscription::<_, cosmic_theme::ThemeMode>(
                TypeId::of::<ThemeSubscription>(),
                cosmic_theme::THEME_MODE_ID.into(),
                cosmic_theme::ThemeMode::version(),
            )
            .map(|update: Update<ThemeMode>| {
                if !update.errors.is_empty() {
                    eprintln!(
                        "errors loading theme mode {:?}: {:?}",
                        update.keys, update.errors
                    );
                }
                Message::ConfigInput(ConfigInput::SystemThemeModeChange)
            }),
            // Watch for external file changes
            file_watch_subscription(watched_path),
            // Tray icon on the panel
            crate::tray::subscription().map(Message::Tray),
        ];

        Subscription::batch(subscriptions)
    }

    fn on_app_exit(&mut self) -> Option<Self::Message> {
        Some(Message::AppCloseRequested)
    }

    /// Handles messages emitted by the application and its widgets.
    fn update(&mut self, message: Self::Message) -> Task<cosmic::Action<Self::Message>> {
        match message {
            // UI
            Message::CloseToast(id) => self.handle_close_toast(id),
            Message::AddToast(toast) => self.handle_add_toast(toast),
            Message::LaunchUrl(url) => self.handle_launch_url(url),
            Message::CopyToClipboard(content) => self.handle_copy_to_clipboard(content),
            Message::ToggleContextPage(page) => self.handle_toggle_context_page(page),
            Message::Surface(a) => self.handle_surface(a),
            Message::Key(modifiers, key) => self.handle_key(modifiers, key),
            Message::Modifiers(modifiers) => self.handle_modifiers(modifiers),

            // Menu
            Message::MenuAction(action) => self.handle_menu_action(action),

            // Dialog & NavBar
            Message::DialogAction(action) => self.handle_dialog_action(action),
            Message::NavBarContext(entity) => self.handle_nav_bar_context(entity),
            Message::NavMenuAction(action) => self.handle_nav_menu_action(action),

            // File
            Message::Startup(open_with_file) => self.handle_startup(open_with_file),
            Message::NewFile => self.handle_new_file(),
            Message::NewVaultFile(name) => self.handle_new_vault_file(name),
            Message::NewVaultFolder(name) => self.handle_new_vault_folder(name),
            Message::SaveFile => self.handle_save_file(),
            Message::OpenFile(result) => self.handle_open_file(result),
            Message::FileSaved(result) => self.handle_file_saved(result),
            Message::DiscardChanges(action) => self.handle_discard_changes(action),
            Message::ExternalFileChanged(path) => self.handle_external_file_changed(path),
            Message::AutosaveTick(generation) => self.handle_autosave_tick(generation),

            // Vault / Node
            Message::DeleteNode(entity) => self.handle_delete_node(entity),
            Message::RenameNode(entity, name) => self.handle_rename_node(entity, name),
            Message::MoveNode(entity, path) => self.handle_move_node(entity, path),
            Message::MoveVault => self.handle_move_vault(),
            Message::OpenVault => self.handle_open_vault(),
            Message::VaultMoved(result) => self.handle_vault_moved(result),

            // Editor
            Message::Edit(action) => self.handle_edit(action),
            Message::ApplyFormatting(action) => self.handle_apply_formatting(action),
            Message::Undo => self.handle_undo(),
            Message::Redo => self.handle_redo(),
            Message::Search(action) => self.handle_search(action),
            Message::VaultSearch(action) => self.handle_vault_search(action),
            Message::SmartPaste => self.handle_smart_paste(),
            Message::PastedText(text) => self.handle_pasted_text(text),
            Message::Clipboard(action) => self.handle_clipboard(action),
            Message::ToggleCheckbox(index, value) => self.handle_toggle_checkbox(index, value),
            Message::JumpToLine(line) => self.handle_jump_to_line(line),

            // Preview / Pane
            Message::UpdateMarkState(msg) => self.handle_update_mark_state(msg),
            Message::ImageDownloaded(result) => self.handle_image_downloaded(result),
            Message::SetViewMode(mode) => self.handle_set_view_mode(mode),
            Message::ViewModeSelected(entity) => self.handle_view_mode_selected(entity),
            Message::ToggleZenMode => self.handle_toggle_zen_mode(),
            Message::PaneResized(event) => self.handle_pane_resized(event),
            Message::PaneDragged(event) => self.handle_pane_dragged(event),
            Message::ScrollChanged(id, viewport) => self.handle_scroll_changed(id, viewport),

            // Config
            Message::ConfigInput(input) => self.handle_config_input(input),

            // Others
            Message::OpenInFileExplorer(path) => self.handle_open_in_file_explorer(&path),
            Message::ExportPDF => self.handle_export_pdf(),
            Message::ExportHtml => self.handle_export_html(),
            Message::CopyHtml => self.handle_copy_html(),
            Message::AppCloseRequested => self.handle_app_close_requested(),
            Message::Tray(event) => self.handle_tray(event),
        }
    }
}

impl AppModel {
    /// Updates the header and window titles.
    pub fn update_title(&mut self) -> Task<cosmic::Action<Message>> {
        let window_title = String::from("Cedilla");

        if let Some(id) = self.core.main_window_id() {
            self.set_window_title(window_title, id)
        } else {
            Task::none()
        }
    }

    /// Whether the application is currently drawn with a dark theme
    pub fn is_dark_theme(&self) -> bool {
        match self.config.app_theme {
            AppTheme::Dark => true,
            AppTheme::Light => false,
            AppTheme::System => theme::is_dark(),
        }
    }

    /// The syntax highlighting theme matching the current application theme
    pub fn active_syntax_theme(&self) -> ThemeRef {
        let dark = self.is_dark_theme();
        let id = if dark {
            self.config.dark_highlighter_theme.as_str()
        } else {
            self.config.light_highlighter_theme.as_str()
        };
        self.syntax_themes.resolve(id, dark)
    }

    /// Settings context page
    pub fn settings(&self) -> Element<'_, Message> {
        let app_theme_selected = match self.config.app_theme {
            AppTheme::Dark => 1,
            AppTheme::Light => 2,
            AppTheme::System => 0,
        };

        let light_theme_selected = self
            .syntax_themes
            .index_of(self.config.light_highlighter_theme.as_str());

        let dark_theme_selected = self
            .syntax_themes
            .index_of(self.config.dark_highlighter_theme.as_str());

        let location_label = if let (Ok(vault), Ok(data_dir)) = (
            self.config.vault_path().canonicalize(),
            dirs::data_dir().unwrap().canonicalize(),
        ) {
            if vault.starts_with(data_dir) {
                fl!("vault-default-location")
            } else {
                self.config.vault_path.to_string()
            }
        } else {
            self.config.vault_path.to_string()
        };

        let font_selected = self
            .config
            .selected_font_family
            .as_ref()
            .and_then(|name| self.system_fonts.iter().position(|f| f == name))
            .map(|i| i + 1) // offset by 1 because "Default" is at index 0
            .unwrap_or(0);

        let autosave_selected = AUTOSAVE_DELAYS
            .iter()
            .position(|d| *d == self.config.autosave_delay)
            .unwrap_or(0);

        let spacing = cosmic::theme::spacing();

        let info_tooltip = |content: String| {
            widget::tooltip(
                widget::icon(icons::get_handle("dialog-information-symbolic", 18)),
                container(text(content)).padding(6.).max_width(280.),
                tooltip::Position::Left,
            )
        };

        widget::settings::view_column(vec![
            widget::settings::section()
                .title(fl!("appearance"))
                .add(
                    widget::settings::item::builder(fl!("theme")).control(widget::dropdown(
                        &self.app_themes,
                        Some(app_theme_selected),
                        |t| Message::ConfigInput(ConfigInput::UpdateTheme(t)),
                    )),
                )
                .add(
                    widget::settings::item::builder(fl!("light-highlighter")).control(
                        widget::dropdown(&self.syntax_theme_names, light_theme_selected, |index| {
                            Message::ConfigInput(ConfigInput::UpdateLightHighlighterTheme(index))
                        }),
                    ),
                )
                .add(
                    widget::settings::item::builder(fl!("dark-highlighter")).control(
                        widget::dropdown(&self.syntax_theme_names, dark_theme_selected, |index| {
                            Message::ConfigInput(ConfigInput::UpdateDarkHighlighterTheme(index))
                        }),
                    ),
                )
                .add(
                    widget::settings::item::builder(fl!("highlighter-theme-colors"))
                        .description(fl!("highlighter-theme-colors-description"))
                        .control(widget::dropdown(
                            BoolState::all_labels(),
                            Some(self.config.highlighter_theme_colors.to_index()),
                            |index| {
                                Message::ConfigInput(ConfigInput::HighlighterThemeColors(
                                    BoolState::from_index(index),
                                ))
                            },
                        )),
                )
                .add(
                    cosmic::widget::column::with_children(vec![
                        row![
                            text::body(fl!("custom-themes")),
                            horizontal(),
                            info_tooltip(fl!("custom-themes-info")),
                        ]
                        .align_y(Alignment::Center)
                        .into(),
                        row![
                            button::standard(fl!("import-theme"))
                                .leading_icon(icons::get_handle("import-symbolic", 16))
                                .on_press(Message::ConfigInput(ConfigInput::ImportSyntaxTheme)),
                            button::standard(fl!("open-themes-folder"))
                                .leading_icon(icons::get_handle("folder-open-symbolic", 16))
                                .on_press(Message::ConfigInput(ConfigInput::OpenThemesFolder)),
                            horizontal(),
                            tooltip(
                                button::icon(icons::get_handle("refresh-symbolic", 16))
                                    .on_press(Message::ConfigInput(ConfigInput::ReloadSyntaxThemes)),
                                container(text(fl!("reload-themes")))
                                    .class(cosmic::theme::Container::Background)
                                    .padding(8.),
                                tooltip::Position::Left
                            ),
                        ]
                        .spacing(spacing.space_xxs)
                        .align_y(Alignment::Center)
                        .into(),
                    ])
                    .spacing(spacing.space_xxs),
                )
                .add(
                    widget::settings::item::builder(fl!("text-size"))
                        .description(format!("{}px", self.config.text_size))
                        .control(
                            widget::slider(6..=30, self.config.text_size as u16, |v| {
                                Message::ConfigInput(ConfigInput::UpdateTextSize(v))
                            })
                            .step(1u16),
                        ),
                )
                .add(
                    widget::settings::item::builder(fl!("selected-font")).control(
                        row![
                            widget::dropdown(&self.system_fonts, Some(font_selected), |index| {
                                if index == 0 {
                                    Message::ConfigInput(ConfigInput::ResetFont)
                                } else {
                                    Message::ConfigInput(ConfigInput::UpdateFont(index - 1))
                                }
                            }),
                            info_tooltip(fl!("font-selection-info")),
                        ]
                        .align_y(Alignment::Center)
                        .spacing(spacing.space_xxs),
                    ),
                )
                .into(),
            widget::settings::section()
                .title(fl!("window"))
                .add(
                    widget::settings::item::builder(fl!("frosted-glass"))
                        .description(fl!("frosted-glass-description"))
                        .control(
                            row![
                                widget::dropdown(
                                    FrostedGlass::all_labels(),
                                    Some(self.config.frosted_glass.to_index()),
                                    |index| Message::ConfigInput(ConfigInput::FrostedGlass(index)),
                                ),
                                info_tooltip(fl!("frosted-glass-info")),
                            ]
                            .align_y(Alignment::Center)
                            .spacing(spacing.space_xxs),
                        ),
                )
                .add(
                    widget::settings::item::builder(fl!("window-opacity"))
                        .description(format!("{}%", self.config.window_opacity))
                        .control(
                            widget::slider(
                                MIN_WINDOW_OPACITY..=100,
                                self.config.window_opacity,
                                |v| Message::ConfigInput(ConfigInput::WindowOpacity(v)),
                            )
                            .step(5u8),
                        ),
                )
                .add(
                    widget::settings::item::builder(fl!("window-background"))
                        .description(fl!("window-background-description"))
                        .control(
                            text_input("#1e1e2e", &self.config.window_background)
                                .on_input(|v| {
                                    Message::ConfigInput(ConfigInput::WindowBackground(v))
                                })
                                .width(Length::Fixed(140.)),
                        ),
                )
                .into(),
            widget::settings::section()
                .title(fl!("general"))
                .add(
                    cosmic::widget::column::with_children(vec![
                        row![
                            text::body(fl!("move-vault")),
                            horizontal(),
                            widget::tooltip(
                                widget::icon(icons::get_handle("dialog-information-symbolic", 18)),
                                container(
                                    column![
                                        text(fl!("flatpak-permissions")),
                                        text(fl!("flatpak-permissions-note"))
                                    ]
                                    .spacing(10.)
                                )
                                .padding(6.)
                                .max_width(250.),
                                tooltip::Position::Left
                            ),
                        ]
                        .align_y(Alignment::Center)
                        .into(),
                        row![
                            column![
                                text::body(fl!("current-location")),
                                button::custom(
                                    text(location_label)
                                        .wrapping(cosmic::iced::core::text::Wrapping::Glyph)
                                        .width(Length::Fill)
                                )
                                .on_press(Message::OpenInFileExplorer(PathBuf::from(
                                    self.config.vault_path.clone()
                                )))
                                .class(theme::Button::Link)
                            ]
                            .width(Length::FillPortion(1)),
                            widget::button::destructive(fl!("move-vault"))
                                .on_press(Message::MoveVault)
                                .width(Length::Shrink)
                        ]
                        .align_y(Alignment::Center)
                        .into(),
                        row![
                            button::standard(fl!("open-vault"))
                                .leading_icon(icons::get_handle("folder-open-symbolic", 16))
                                .on_press(Message::OpenVault),
                            horizontal(),
                            info_tooltip(fl!("open-vault-info")),
                        ]
                        .align_y(Alignment::Center)
                        .spacing(spacing.space_xxs)
                        .into(),
                    ])
                    .spacing(spacing.space_xxs),
                )
                .add(
                    widget::settings::item::builder(fl!("autosave"))
                        .description(fl!("autosave-description"))
                        .control(widget::dropdown(
                            autosave_labels(),
                            Some(autosave_selected),
                            |index| Message::ConfigInput(ConfigInput::Autosave(index)),
                        )),
                )
                .add(
                    widget::settings::item::builder(fl!("last-file")).control(widget::dropdown(
                        BoolState::all_labels(),
                        Some(self.config.open_last_file.to_index()),
                        |index| {
                            Message::ConfigInput(ConfigInput::OpenLastFile(BoolState::from_index(
                                index,
                            )))
                        },
                    )),
                )
                .add(
                    widget::settings::item::builder(fl!("scrollbar-sync")).control(
                        widget::dropdown(
                            BoolState::all_labels(),
                            Some(self.config.scrollbar_sync.to_index()),
                            |index| {
                                Message::ConfigInput(ConfigInput::ScrollbarSync(
                                    BoolState::from_index(index),
                                ))
                            },
                        ),
                    ),
                )
                .into(),
            widget::settings::section()
                .title(fl!("view"))
                .add(
                    widget::settings::item::builder(fl!("help-bar")).control(widget::dropdown(
                        ShowState::all_labels(),
                        Some(self.config.show_helper_header_bar.to_index()),
                        |index| {
                            Message::ConfigInput(ConfigInput::HelperHeaderBarShowState(
                                ShowState::from_index(index),
                            ))
                        },
                    )),
                )
                .add(
                    widget::settings::item::builder(fl!("status-bar")).control(widget::dropdown(
                        ShowState::all_labels(),
                        Some(self.config.show_status_bar.to_index()),
                        |index| {
                            Message::ConfigInput(ConfigInput::StatusBarShowState(
                                ShowState::from_index(index),
                            ))
                        },
                    )),
                )
                .into(),
        ])
        .into()
    }

    /// Outline (document headings) context page
    pub fn outline_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();

        let State::Ready { editor, .. } = &self.state else {
            return text(fl!("loading")).into();
        };

        let headings = utils::outline::headings(&editor.content.text());
        if headings.is_empty() {
            return container(text::body(fl!("no-headings")))
                .padding(spacing.space_s)
                .into();
        }

        let current_line = editor.content.cursor().position.line;
        // the heading the cursor is currently under
        let active = headings
            .iter()
            .rposition(|h| h.line <= current_line);

        let items: Vec<Element<Message>> = headings
            .into_iter()
            .enumerate()
            .map(|(i, heading)| {
                let indent = f32::from(heading.level.saturating_sub(1)) * 14.0;
                let label = if heading.text.is_empty() {
                    format!("{} {}", "#".repeat(usize::from(heading.level)), fl!("untitled"))
                } else {
                    heading.text
                };

                let label = if heading.level <= 2 {
                    text::body(label)
                } else {
                    text::caption(label)
                };

                button::custom(
                    row![horizontal().width(indent), label]
                        .align_y(Alignment::Center)
                        .spacing(spacing.space_xxs),
                )
                .on_press(Message::JumpToLine(heading.line))
                .class(if active == Some(i) {
                    theme::Button::Suggested
                } else {
                    theme::Button::MenuItem
                })
                .width(Length::Fill)
                .into()
            })
            .collect();

        widget::column::with_children(items)
            .spacing(spacing.space_xxxs)
            .width(Length::Fill)
            .into()
    }

    /// Vault search context page
    pub fn vault_search_view(&self) -> Element<'_, Message> {
        let spacing = cosmic::theme::spacing();
        let search = &self.vault_search;

        let status: String = if let Some(err) = &search.error {
            err.clone()
        } else if search.searching {
            fl!("searching")
        } else if search.query.trim().is_empty() {
            fl!("search-vault-hint")
        } else {
            fl!("vault-results", count = search.results.len())
        };

        let input_row = row![
            text_input(fl!("search-vault-placeholder"), &search.query)
                .id(vault_search_input_id())
                .on_input(|v| Message::VaultSearch(VaultSearchAction::UpdateQuery(v)))
                .width(Length::Fill),
            button::icon(icons::get_handle("regex-symbolic", 18))
                .on_press(Message::VaultSearch(VaultSearchAction::ToggleRegex))
                .class(if search.use_regex {
                    theme::Button::Suggested
                } else {
                    theme::Button::Standard
                }),
        ]
        .spacing(spacing.space_xxs)
        .align_y(Alignment::Center);

        let vault_root = self.config.vault_path();

        let results: Vec<Element<Message>> = search
            .results
            .iter()
            .map(|result| {
                let relative = result
                    .path
                    .strip_prefix(&vault_root)
                    .unwrap_or(&result.path)
                    .to_string_lossy()
                    .into_owned();

                button::custom(
                    column![
                        text::caption(format!("{relative}:{}", result.line + 1)),
                        text::body(result.snippet.clone())
                            .wrapping(cosmic::iced::core::text::Wrapping::WordOrGlyph),
                    ]
                    .spacing(spacing.space_xxxs)
                    .width(Length::Fill),
                )
                .on_press(Message::VaultSearch(VaultSearchAction::Open(
                    result.path.clone(),
                    result.line,
                )))
                .class(theme::Button::MenuItem)
                .width(Length::Fill)
                .into()
            })
            .collect();

        column![
            input_row,
            text::caption(status),
            widget::column::with_children(results)
                .spacing(spacing.space_xxxs)
                .width(Length::Fill),
        ]
        .spacing(spacing.space_xs)
        .width(Length::Fill)
        .into()
    }
}

/// Builds the header bar view mode switcher model
fn view_mode_model(active: ViewMode) -> segmented_button::SingleSelectModel {
    let mut model = segmented_button::Model::builder();
    for mode in ViewMode::ALL {
        model = model.insert(|b| {
            b.icon(icons::get_icon(mode.icon_name(), 16))
                .data::<ViewMode>(*mode)
        });
    }
    let mut model = model.build();
    if let Some(position) = ViewMode::ALL.iter().position(|m| *m == active) {
        model.activate_position(position as u16);
    }
    model
}

//
// VIEWS
//

/// Everything the main view needs from the application model
struct MainViewParams<'a> {
    app_config: &'a CedillaConfig,
    editor: &'a EditorState,
    preview: &'a MarkdownPreview,
    panes: &'a pane_grid::State<PaneContent>,
    view_mode: ViewMode,
    zen_mode: bool,
    font: Font,
    syntax_theme: ThemeRef,
    theme_colors: bool,
    key_binds: &'a HashMap<menu::KeyBind, MenuAction>,
}

/// A card container, optionally painted with the syntax theme colors
fn card_class<'a>(colors: Option<(Option<Color>, Option<Color>)>) -> theme::Container<'a> {
    match colors {
        Some((background, foreground)) => theme::Container::custom(move |theme| {
            let cosmic = theme.cosmic();
            let mut style = <theme::Theme as cosmic::iced::widget::container::Catalog>::style(
                theme,
                &theme::Container::Card,
            );
            if let Some(bg) = background {
                style.background = Some(cosmic::iced::Background::Color(bg));
            }
            if let Some(fg) = foreground {
                style.text_color = Some(fg);
                style.icon_color = Some(fg);
            }
            style.border.radius = cosmic.corner_radii.radius_s.into();
            style
        }),
        None => theme::Container::Card,
    }
}

/// View of the header of this screen
fn cedilla_main_view<'a>(params: MainViewParams<'a>) -> Element<'a, Message> {
    let MainViewParams {
        app_config,
        editor,
        preview,
        panes,
        view_mode,
        zen_mode,
        font,
        syntax_theme,
        theme_colors,
        key_binds,
    } = params;

    let spacing = theme::active().cosmic().spacing;

    // colors of the syntax theme, used when the user wants them for the editor
    let theme_bg = theme_colors
        .then(|| hl::theme_background(syntax_theme))
        .flatten();
    let theme_fg = theme_colors
        .then(|| hl::theme_foreground(syntax_theme))
        .flatten();
    let editor_colors = theme_colors.then_some((theme_bg, theme_fg));

    let create_editor = move || {
        container(responsive(move |size| {
            let text_editor = TextEditor::new(&editor.content)
                .id(text_editor_id())
                .key_binding(text_editor_key_bindings)
                .size(app_config.text_size)
                .font(font)
                .padding(0)
                .retain_focus_on_external_click(!editor.search.show_search_box)
                .on_action(Message::Edit);

            let text_editor = if theme_colors {
                text_editor.style(move |theme, status, is_code_block| {
                    let mut style = text_editor::default(theme, status, is_code_block);
                    if let Some(bg) = theme_bg {
                        style.background = cosmic::iced::Background::Color(bg);
                    }
                    if let Some(fg) = theme_fg {
                        style.value = fg;
                        style.placeholder = fg;
                    }
                    style
                })
            } else {
                text_editor
            };

            let text_editor = text_editor.highlight_with::<hl::Highlighter>(
                hl::Settings {
                    theme: syntax_theme,
                    token: editor
                        .path
                        .as_ref()
                        .and_then(|path| path.extension()?.to_str())
                        .unwrap_or("md")
                        .to_string(),
                    font,
                },
                |highlight, _theme| highlight.to_format(),
            );

            let scrollable_editor: Element<'a, Message> = scrollable(text_editor)
                .id(editor_scrollable_id())
                .on_scroll(|vp| Message::ScrollChanged(editor_scrollable_id(), vp))
                .height(Length::Fixed(size.height - 5.)) // This is a bit of a workaround but it works,
                .into();

            widget::context_menu(
                scrollable_editor,
                Some(app_menu::editor_context_menu(key_binds)),
            )
            .into()
        }))
        .padding([5, spacing.space_xxs])
        .width(Length::Fill)
        .height(Length::Fill)
    };

    let create_preview = move || {
        container(
            scrollable(
                MarkWidget::new(&preview.markstate)
                    .on_updating_state(Message::UpdateMarkState)
                    .on_clicking_link(Message::LaunchUrl)
                    .on_copying_code(Message::CopyToClipboard)
                    .on_toggling_checkbox(Message::ToggleCheckbox)
                    .font(font)
                    .text_size(app_config.text_size)
                    .code_highlight_theme(syntax_theme)
                    .code_block_theme_colors(theme_colors)
                    .on_drawing_image(|info| {
                        if let Some(image) = preview.images.get(info.url).cloned() {
                            let mut img = widget::image(image);
                            if let Some(w) = info.width {
                                img = img.width(w);
                            }
                            if let Some(h) = info.height {
                                img = img.height(h);
                            }
                            img.into()
                        } else if let Some(svg_f) = preview.svgs.get(info.url).cloned() {
                            let mut svg = widget::svg(svg_f);
                            if let Some(w) = info.width {
                                svg = svg.width(w);
                            }
                            if let Some(h) = info.height {
                                svg = svg.height(h);
                            }
                            svg.into()
                        } else {
                            text(fl!("loading-image")).into()
                        }
                    }),
            )
            .id(preview_scrollable_id())
            .on_scroll(|vp| Message::ScrollChanged(preview_scrollable_id(), vp))
            .width(Length::Fill),
        )
        .padding(spacing.space_xxs)
        .width(Length::Fill)
        .height(Length::Fill)
    };

    let create_title = |icon_name: &str, title: String| {
        row![
            icons::get_icon_owned(icon_name.to_string(), 18).size(16),
            text(title.to_uppercase()).font(Font::MONOSPACE).size(14),
        ]
        .spacing(spacing.space_xxs)
        .padding([5, spacing.space_xxs])
        .align_y(Alignment::Center)
    };

    let main_content: Element<'a, Message> = match view_mode {
        ViewMode::Editor => {
            // Editor only view
            let mut col = column![];
            if !zen_mode {
                col = col.push(
                    container(create_title("edit-symbolic", fl!("editor")))
                        .padding(3.)
                        .width(Length::Fill)
                        .class(theme::Container::Card),
                );
            }
            container(col.push(create_editor()))
                .width(Length::Fill)
                .height(Length::Fill)
                .class(card_class(editor_colors))
                .into()
        }
        ViewMode::Preview => {
            // Preview only view
            let mut col = column![];
            if !zen_mode {
                col = col.push(
                    container(create_title("show-symbolic", fl!("preview")))
                        .padding(3.)
                        .width(Length::Fill)
                        .class(theme::Container::Card),
                );
            }
            container(col.push(create_preview()))
                .width(Length::Fill)
                .height(Length::Fill)
                .class(theme::Container::Card)
                .into()
        }
        ViewMode::Split => {
            // Pane grid with editor and preview
            pane_grid::PaneGrid::new(panes, |_pane, content, _is_focused| {
                let (title, icon_name) = match content {
                    PaneContent::Editor => (fl!("editor"), "edit-symbolic"),
                    PaneContent::Preview => (fl!("preview"), "show-symbolic"),
                };

                let (pane_content, class): (Element<'a, Message>, theme::Container<'a>) =
                    match content {
                        PaneContent::Editor => (create_editor().into(), card_class(editor_colors)),
                        PaneContent::Preview => (create_preview().into(), theme::Container::Card),
                    };

                let content = pane_grid::Content::new(pane_content).class(class);
                if zen_mode {
                    content
                } else {
                    content.title_bar(
                        pane_grid::TitleBar::new(create_title(icon_name, title)).padding(3.),
                    )
                }
            })
            .on_drag(Message::PaneDragged)
            .on_resize(10, Message::PaneResized)
            .spacing(spacing.space_xxs)
            .into()
        }
    };

    let status_bar: Element<Message> = {
        let file_path = match editor.path.as_deref().and_then(Path::to_str) {
            Some(path) => {
                if path.starts_with(&app_config.vault_path) {
                    let relative = path
                        .trim_start_matches(&*app_config.vault_path)
                        .trim_start_matches('/');
                    text(format!("Cedilla Vault/{relative}")).size(12)
                } else {
                    text(path).size(12)
                }
            }
            None => text(fl!("new-file")).size(12),
        };

        let dirty_indicator = if editor.is_dirty {
            text("•").size(12)
        } else {
            text("").size(12)
        };

        let stats = DocStats::compute(&editor.content.text());
        let stats_text = text(format!(
            "{} · {} · {}",
            fl!("words", count = stats.words),
            fl!("characters", count = stats.chars),
            fl!("reading-time", minutes = stats.reading_minutes),
        ))
        .size(12);

        let position = {
            let cursor = editor.content.cursor();
            text(format!(
                "{}:{}",
                cursor.position.line + 1,
                cursor.position.column + 1
            ))
            .size(12)
        };

        container(
            row![
                file_path,
                dirty_indicator,
                horizontal(),
                stats_text,
                horizontal().width(spacing.space_m),
                position
            ]
            .padding(spacing.space_xxs)
            .spacing(spacing.space_xxs)
            .align_y(Alignment::Center),
        )
        .width(Length::Fill)
        .class(theme::Container::Card)
        .into()
    };

    let helper_header_bar = {
        let format_button = |icon: &'static str, action: utils::SelectionAction| {
            button::icon(icons::get_handle(icon, 18)).on_press(Message::ApplyFormatting(action))
        };

        container(
            row![
                format_button("helperbar/bold-symbolic", utils::SelectionAction::Bold),
                format_button("helperbar/italic-symbolic", utils::SelectionAction::Italic),
                format_button(
                    "helperbar/strikethrough-symbolic",
                    utils::SelectionAction::Strikethrough
                ),
                horizontal().width(18.),
                format_button("helperbar/link-symbolic", utils::SelectionAction::Hyperlink),
                format_button("helperbar/code-symbolic", utils::SelectionAction::Code),
                format_button("helperbar/math-symbolic", utils::SelectionAction::Math),
                format_button("helperbar/image-symbolic", utils::SelectionAction::Image),
                horizontal().width(18.),
                format_button(
                    "helperbar/bulleted-list-symbolic",
                    utils::SelectionAction::BulletedList
                ),
                format_button(
                    "helperbar/numbered-list-symbolic",
                    utils::SelectionAction::NumberedList
                ),
                format_button(
                    "helperbar/checked-list-symbolic",
                    utils::SelectionAction::CheckboxList
                ),
                format_button("helperbar/heading-symbolic", utils::SelectionAction::Heading1),
                format_button("helperbar/quote-symbolic", utils::SelectionAction::Blockquote),
                format_button("helperbar/table-symbolic", utils::SelectionAction::Table),
                format_button("helperbar/rule-symbolic", utils::SelectionAction::Rule),
                horizontal().width(18.),
                format_button("helperbar/datetime-symbolic", utils::SelectionAction::DateTime),
                horizontal(),
                button::icon(icons::get_handle("outline-symbolic", 18))
                    .on_press(Message::ToggleContextPage(ContextPage::Outline)),
                button::icon(icons::get_handle("search-vault-symbolic", 18))
                    .on_press(Message::ToggleContextPage(ContextPage::VaultSearch)),
                button::icon(icons::get_handle("helperbar/pdf-symbolic", 18))
                    .on_press(Message::ExportPDF)
            ]
            .padding(spacing.space_xxs)
            .spacing(spacing.space_xxs),
        )
        .width(Length::Fill)
        .class(theme::Container::Card)
    };

    let show_help_bar = app_config.show_helper_header_bar.is_shown() && !zen_mode;
    let show_status_bar = app_config.show_status_bar.is_shown() && !zen_mode;

    let content_column = if show_help_bar {
        column![helper_header_bar, main_content]
    } else {
        column![main_content]
    }
    .spacing(spacing.space_xxxs);

    let content_column = if show_status_bar {
        content_column.extend([status_bar])
    } else {
        content_column
    };

    let base: Element<Message> = container(content_column)
        .padding(
            Padding::new(spacing.space_xxs as f32)
                .left(0.)
                .right(0.)
                .top(0.),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .into();

    if !editor.search.show_search_box {
        base
    } else {
        let match_status: String = match editor.search.current_match_index {
            _ if editor.search.regex_error.is_some() => editor.search.regex_error.clone().unwrap(),
            Some(i) => format!("{}/{}", i + 1, editor.search.matches.len()),
            None if editor.search.search_value.is_empty() => String::new(),
            None => fl!("no-results"),
        };

        let search_box = container(
            row![
                text_input(fl!("search"), &editor.search.search_value)
                    .id(search_input_id())
                    .on_focus(Message::Search(SearchAction::FocusSearchField))
                    .on_input(|v| Message::Search(SearchAction::UpdateSearchValue(v)))
                    .on_submit(|_v| Message::Search(SearchAction::NextResult))
                    .width(Length::Fixed(200.)),
                button::icon(icons::get_handle("go-up-symbolic", 16))
                    .on_press(Message::Search(SearchAction::PrevResult))
                    .class(theme::Button::Icon),
                button::icon(icons::get_handle("go-down-symbolic", 16))
                    .on_press(Message::Search(SearchAction::NextResult))
                    .class(theme::Button::Icon),
                button::icon(icons::get_handle("regex-symbolic", 18))
                    .on_press(Message::Search(SearchAction::ToggleRegex))
                    .class(if editor.search.use_regex {
                        theme::Button::Suggested
                    } else {
                        theme::Button::Standard
                    }),
                text(match_status).size(12),
                button::icon(icons::get_handle("window-close-symbolic", 16))
                    .on_press(Message::Search(SearchAction::ToggleSearch))
                    .class(theme::Button::Icon),
            ]
            .spacing(spacing.space_xxs)
            .align_y(Alignment::Center)
            .padding(spacing.space_xs),
        )
        .class(theme::Container::Background)
        .width(Length::Shrink);

        let overlay = container(search_box)
            .align_right(Length::Fill)
            .align_top(Length::Fill)
            .padding(
                Padding::new(0.)
                    .top(spacing.space_xs as f32)
                    .right(spacing.space_s as f32),
            );

        cosmic::iced::widget::stack!(base, overlay).into()
    }
}

// Watches for external changes on the currently open file
fn file_watch_subscription(path: Option<PathBuf>) -> Subscription<Message> {
    use cosmic::iced::futures::SinkExt;
    use cosmic::iced::futures::channel::mpsc as iced_mpsc;
    use notify::{EventKind, RecursiveMode, Watcher, recommended_watcher};

    let Some(path) = path else {
        return Subscription::none();
    };

    Subscription::run_with(path, |path| {
        let path_owned = path.clone();

        cosmic::iced::stream::channel(
            16,
            move |mut output: iced_mpsc::Sender<Message>| async move {
                let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<PathBuf>();

                let mut watcher =
                    match recommended_watcher(move |res: notify::Result<notify::Event>| {
                        if let Ok(event) = res {
                            let is_relevant =
                                matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_));
                            if is_relevant {
                                for p in event.paths {
                                    let _ = tx.send(p);
                                }
                            }
                        }
                    }) {
                        Ok(w) => w,
                        Err(e) => {
                            eprintln!("file_watcher: failed to create watcher: {e}");
                            return;
                        }
                    };

                if let Err(e) = watcher.watch(&path_owned, RecursiveMode::NonRecursive) {
                    eprintln!("file_watcher: failed to watch {path_owned:?}: {e}");
                    return;
                }

                // keep the watcher alive
                let _watcher = watcher;

                loop {
                    let changed_path = match rx.recv().await {
                        Some(p) => p,
                        None => return, // channel closed, exit the stream
                    };

                    if changed_path == path_owned {
                        let _ = output
                            .send(Message::ExternalFileChanged(changed_path.clone()))
                            .await;

                        tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        while rx.try_recv().is_ok() {}
                    }
                }
            },
        )
    })
}

/// Custom Text Editor Key Bindings
fn text_editor_key_bindings(key_press: text_editor::KeyPress) -> Option<text_editor::Binding<Message>> {
    use cosmic::iced::keyboard::{key::Named, Key};
    use text_editor::{Binding, Motion, Status};

    let text_editor::KeyPress {
        key,
        modified_key,
        physical_key,
        modifiers,
        status,
        ..
    } = &key_press;

    // from_key_press() bails out unless focused we better do the same here
    if !matches!(status, Status::Focused { .. }) {
        return None;
    }

    if modifiers.command() && !modifiers.alt() {
        match key.to_latin(*physical_key) {
            // Ctrl+P paste image
            Some('p') => {
                return Some(Binding::Custom(Message::MenuAction(MenuAction::PasteImage)));
            }
            // Ctrl+V smart paste (image detection or text)
            Some('v') => return Some(Binding::Custom(Message::SmartPaste)),
            // Ctrl+C / Ctrl+X go through the application so the system clipboard is used
            Some('c') => {
                return Some(Binding::Custom(Message::Clipboard(ClipboardAction::Copy)));
            }
            Some('x') => {
                return Some(Binding::Custom(Message::Clipboard(ClipboardAction::Cut)));
            }
            Some('a') => return Some(Binding::SelectAll),
            // Any other Ctrl+<character> is an application shortcut: never insert the
            // character, let the event bubble up to the application key binds.
            Some(_) if matches!(key, Key::Character(_)) => return None,
            _ => {}
        }
    }

    // Ctrl+Backspace delete the previous word.
    // Ctrl+Shift+Backspace delete the line
    if matches!(modified_key, Key::Named(Named::Backspace)) && modifiers.jump() {
        let motion = if modifiers.shift() {
            Motion::Home
        } else {
            Motion::Left.widen()
        };

        return Some(Binding::Sequence(vec![
            Binding::Select(motion),
            Binding::Backspace,
        ]));
    }

    // everything else we fall back to the default handling
    Binding::from_key_press(key_press)
}

/// Returns the text editor scrollable Id
fn editor_scrollable_id() -> widget::Id {
    widget::Id::new("editor_scroll")
}

/// Returns the cotnent preview scrollable Id
fn preview_scrollable_id() -> widget::Id {
    widget::Id::new("preview_scroll")
}

/// Returns the text editor id
fn text_editor_id() -> widgets::text_editor::Id {
    widgets::text_editor::Id::from(widget::Id::new("text_editor"))
}

/// Returns the search input id
pub fn search_input_id() -> widget::Id {
    widget::Id::new("search_input")
}

/// Returns the vault search input id
pub fn vault_search_input_id() -> widget::Id {
    widget::Id::new("vault_search_input")
}

/// Creates the default panes for the app
fn create_default_panes() -> pane_grid::State<PaneContent> {
    let (mut panes, first_pane) = pane_grid::State::new(PaneContent::Editor);
    panes.split(pane_grid::Axis::Vertical, first_pane, PaneContent::Preview);
    panes
}
