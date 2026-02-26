pub mod pending_window;
pub mod save_project;
pub mod spectral_decay_config;
pub mod spectrogram_config;

use iced::{
    Element, Font,
    Length::Fill,
    font,
    widget::{button, column, container, scrollable, text},
};
pub use pending_window::pending_window;
pub use spectral_decay_config::SpectralDecayConfig;
pub use spectrogram_config::SpectrogramConfig;

use crate::screen::main::{recording::Recording, tab};

#[allow(clippy::large_enum_variant)]
#[derive(Default, Debug)]
pub enum Modal {
    #[default]
    None,
    PendingWindow {
        goto_tab: tab::Id,
    },
    SpectralDecayConfig(SpectralDecayConfig),
    SpectrogramConfig(SpectrogramConfig),
    // TODO move recording into mod modal
    Recording(Recording),
    SaveProjectDialog(save_project::View),
    OpenRecentProject,
}

pub fn load_recent_project<'a, Message>(
    recent_projects: &'a crate::data::RecentProjects,
    msg: impl Fn(usize) -> Message + Clone,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let bold = |label| {
        let mut font = Font::default();
        font.weight = font::Weight::Bold;

        text(label).font(font)
    };

    let recent_project_entries = recent_projects
        .iter()
        .enumerate()
        .filter_map(|(i, p)| p.file_name().map(|f| (i, f)))
        .filter_map(|(i, p)| p.to_str().map(|f| (i, f)))
        .map(|(i, n)| {
            button(n)
                .on_press(msg(i))
                .width(Fill)
                .style(button::subtle)
                .into()
        });

    column!(
        container(bold("Recent projects"))
            .padding(5)
            .style(container::rounded_box)
            .width(Fill),
        container(scrollable(
            column(recent_project_entries).spacing(2).padding(1)
        ))
        .style(container::bordered_box)
    )
    .width(400)
    .into()
}
