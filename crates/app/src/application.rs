//! Main Libadwaita application setup.

use crate::client_view::ClientView;
use crate::host_view::HostView;
use gtk4::prelude::*;
use libadwaita::prelude::*;

pub const APP_ID: &str = "org.linux.screenextend";

pub struct ScreenExtendApp {
    app: libadwaita::Application,
}

impl ScreenExtendApp {
    pub fn new() -> Self {
        let app = libadwaita::Application::builder()
            .application_id(APP_ID)
            .build();

        app.connect_activate(Self::on_activate);

        Self { app }
    }

    pub fn run(&self) -> gtk4::glib::ExitCode {
        self.app.run()
    }

    fn on_activate(app: &libadwaita::Application) {
        let window = libadwaita::ApplicationWindow::builder()
            .application(app)
            .title("ScreenExtend")
            .default_width(900)
            .default_height(650)
            .build();

        let header = libadwaita::HeaderBar::new();

        let main_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .build();
        main_box.append(&header);

        // View switcher and view stack
        let stack = libadwaita::ViewStack::new();

        let host_view = HostView::new();
        let host_page = stack.add_titled(host_view.widget(), Some("host"), "Host Mode");
        host_page.set_icon_name(Some("video-display-symbolic"));

        let client_view = ClientView::new();
        let client_page = stack.add_titled(client_view.widget(), Some("client"), "Client Mode");
        client_page.set_icon_name(Some("network-transmit-receive-symbolic"));

        let switcher_title = libadwaita::ViewSwitcherTitle::builder()
            .stack(&stack)
            .title("ScreenExtend")
            .build();
        header.set_title_widget(Some(&switcher_title));

        let view_switcher_bar = libadwaita::ViewSwitcherBar::builder()
            .stack(&stack)
            .build();

        main_box.append(&stack);
        main_box.append(&view_switcher_bar);

        window.set_content(Some(&main_box));
        window.present();
    }
}

impl Default for ScreenExtendApp {
    fn default() -> Self {
        Self::new()
    }
}
