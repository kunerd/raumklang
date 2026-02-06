use iced::{
    Alignment::Center,
    Element, Font,
    Length::{Fill, Shrink},
    Task,
    advanced::graphics::core::font,
    alignment::Vertical::Bottom,
    widget::{button, checkbox, column, container, pick_list, right, row, rule, text},
};

use std::path::{Path, PathBuf};

use crate::data::project::{self, Operation};

#[derive(Debug, Clone)]
pub struct View {
    base_path: PathBuf,
    file_path_str: String,
    create_subdir: bool,
    measurement_operation: Operation,
    export_from_memory: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFileDialog,
    ChangeProjectPath(String),
    ToggleCreateSubdir(bool),

    ChangeOperation(Operation),
    ToggleExportFromMemory(bool),

    Cancel,
    Save,
}

pub enum Action {
    None,
    Cancel,
    Task(Task<Message>),
    Save(PathBuf, project::Operation, bool),
}

impl View {
    pub fn new(measurement_operation: Operation) -> Self {
        Self {
            base_path: PathBuf::new(),
            file_path_str: String::new(),
            create_subdir: true,
            measurement_operation,
            export_from_memory: true,
        }
    }

    pub fn update(&mut self, msg: Message) -> Action {
        match msg {
            Message::OpenFileDialog => {
                let task = Task::future(pick_file()).and_then(|path| {
                    Task::done(Message::ChangeProjectPath(
                        path.to_string_lossy().to_string(),
                    ))
                });

                Action::Task(task)
            }
            Message::ChangeProjectPath(path) => {
                self.base_path = PathBuf::from(path);

                let new_path = self.compute_final_path(self.create_subdir);
                self.file_path_str = new_path.to_string_lossy().to_string();

                Action::None
            }
            Message::ToggleCreateSubdir(state) => {
                self.create_subdir = state;

                let new_path = self.compute_final_path(state);
                self.file_path_str = new_path.to_string_lossy().to_string();

                Action::None
            }
            Message::ChangeOperation(operation) => {
                self.measurement_operation = operation;
                Action::None
            }
            Message::ToggleExportFromMemory(state) => {
                self.export_from_memory = state;
                Action::None
            }
            Message::Cancel => Action::Cancel,
            Message::Save => Action::Save(
                PathBuf::from(&self.file_path_str),
                self.measurement_operation,
                self.export_from_memory,
            ),
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let bold = |s| {
            let mut font = Font::DEFAULT;
            font.weight = font::Weight::Bold;

            text(s).font(font)
        };

        let header = column![bold("Save Project").size(18), rule::horizontal(2)].spacing(8);

        let content = {
            let file_path_picker = {
                let subdir_checkbox = checkbox(self.create_subdir)
                    .label("Create subdirectory for project")
                    .on_toggle(Message::ToggleCreateSubdir);

                column![
                    text("Project file path:"),
                    row![
                        container(text(&self.file_path_str).align_y(Bottom))
                            .padding(8)
                            .align_y(Center)
                            .height(Shrink)
                            .width(Fill)
                            .style(|theme| {
                                let base = container::bordered_box(theme);
                                let palette = theme.extended_palette();

                                base.background(palette.background.base.color)
                            }),
                        button("...")
                            .style(button::secondary)
                            .on_press(Message::OpenFileDialog)
                    ]
                    .height(Shrink)
                    .spacing(5)
                    .align_y(Bottom),
                    subdir_checkbox,
                ]
                .spacing(5)
            };

            let measurement_settings = {
                let measurement_file_operation = {
                    column![
                        text("Choose how imported measurements should be handled"),
                        pick_list(
                            Operation::ALL,
                            Some(&self.measurement_operation),
                            Message::ChangeOperation
                        )
                    ]
                    .spacing(10)
                };

                let export_in_memory_measurements = checkbox(self.export_from_memory)
                    .label("Export in-memory measurements.")
                    .on_toggle(Message::ToggleExportFromMemory);

                column![measurement_file_operation, export_in_memory_measurements].spacing(5)
            };

            column![file_path_picker, measurement_settings].spacing(20)
        };

        let controls = {
            let cancel = button("Cancel")
                .style(button::secondary)
                .on_press(Message::Cancel);

            let is_not_empty = !self.file_path_str.is_empty();
            let save = button("Save")
                .style(button::success)
                .on_press_maybe(is_not_empty.then_some(Message::Save));

            right(row![save, cancel].spacing(8))
        };

        container(column![header, content, rule::horizontal(1), controls].spacing(20))
            .padding(20)
            .width(600)
            .style(container::bordered_box)
            .into()
    }

    fn compute_final_path(&mut self, subdir: bool) -> PathBuf {
        let new_path = if subdir {
            let mut subdir_path = self
                .base_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_default();

            subdir_path.push(self.base_path.file_stem().unwrap_or_default());
            subdir_path.push(self.base_path.file_name().unwrap_or_default());

            subdir_path
        } else {
            self.base_path.to_path_buf()
        };
        new_path
    }
}

async fn pick_file() -> Option<PathBuf> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Save project file ...")
        .save_file()
        .await?;

    Some(handle.path().to_path_buf())
}
