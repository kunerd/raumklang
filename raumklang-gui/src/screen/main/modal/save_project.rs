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
    pub base_path: PathBuf,
    pub file_path_str: String,
    pub create_subdir: bool,
    pub measurment_operation: Operation,
}

#[derive(Debug, Clone)]
pub enum Message {
    OpenFileDialog,
    ChangeOperation(Operation),
    Cancel,
    Save,
    ChangeProjectPath(String),
    ToggleCreateSubdir(bool),
}

pub enum Action {
    None,
    Cancel,
    Task(Task<Message>),
    Save(PathBuf, project::SaveSettings),
}

impl View {
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
                self.measurment_operation = operation;
                Action::None
            }
            Message::Cancel => Action::Cancel,
            Message::Save => Action::Save(
                PathBuf::from(&self.file_path_str),
                project::SaveSettings {
                    create_subdir: self.create_subdir,
                    measurement_operation: self.measurment_operation,
                },
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
                    .spacing(5)
                    .align_y(Bottom)
                ]
                .spacing(10)
            };

            let subdir_checkbox = checkbox(self.create_subdir)
                .label("Create subdirectory for project")
                .on_toggle(Message::ToggleCreateSubdir);

            let measurement_file_operation = {
                column![
                    text("Choose how imported measurements should be handled"),
                    pick_list(
                        Operation::ALL,
                        Some(&self.measurment_operation),
                        Message::ChangeOperation
                    )
                ]
                .spacing(10)
            };

            column![
                file_path_picker,
                subdir_checkbox,
                measurement_file_operation
            ]
            .spacing(20)
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
