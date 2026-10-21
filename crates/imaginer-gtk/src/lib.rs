//! GTK 4 user interface.

use std::process::ExitCode;

use gtk4::prelude::*;
use gtk4::{Application, ApplicationWindow};
use imaginer_core::{APP_ID, APP_NAME};

pub fn run() -> ExitCode {
    let app = Application::builder().application_id(APP_ID).build();
    app.connect_activate(build_window);
    ExitCode::from(app.run().get())
}

fn build_window(app: &Application) {
    ApplicationWindow::builder()
        .application(app)
        .title(APP_NAME)
        .default_width(1024)
        .default_height(768)
        .build()
        .present();
}
