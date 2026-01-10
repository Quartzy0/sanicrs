use relm4::adw::{self, gtk};
use relm4::adw::gio;
use relm4::WidgetTemplate;
use crate::icon_names;

#[relm4::widget_template(pub)]
impl WidgetTemplate for HeaderBar {
    view! {
        #[name = "header_bar"]
        adw::HeaderBar {
            set_show_end_title_buttons: true,
            set_show_back_button: true,
            pack_end = &gtk::MenuButton {
                set_icon_name: icon_names::shipped::MENU,

                #[wrap(Some)]
                set_menu_model = &gio::Menu {
                    append_item = &gio::MenuItem::new(Some("Preferences"), Some("win.preferences")),
                    append_item = &gio::MenuItem::new(Some("About"), Some("win.about")),
                    append_item = &gio::MenuItem::new(Some("Quit"), Some("win.quit")),
                }
            }
        }
    }
}
