//! iced user interface.

use std::process::ExitCode;

use iced::Element;
use iced::widget::text;
use imaginer_core::APP_NAME;

#[derive(Default)]
struct App;

#[derive(Clone, Debug)]
enum Message {}

impl App {
    fn update(&mut self, message: Message) {
        match message {}
    }

    fn view(&self) -> Element<'_, Message> {
        text(APP_NAME).into()
    }
}

pub fn run() -> ExitCode {
    let result = iced::application(App::default, App::update, App::view)
        .title(APP_NAME)
        .window_size((1024.0, 768.0))
        .run();
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
