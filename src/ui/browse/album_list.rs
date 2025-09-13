use relm4::WidgetTemplate;
use relm4::adw::gtk;
use relm4::adw::prelude::*;
use relm4::adw::gtk::{Orientation, Align};
use crate::icon_names;

#[relm4::widget_template(pub)]
impl WidgetTemplate for AlbumList {

    view! {
        gtk::Box {
            set_orientation: Orientation::Vertical,

            gtk::CenterBox {
                set_orientation: Orientation::Horizontal,
                set_halign: Align::Fill,
                set_hexpand: true,
                set_hexpand_set: true,

                #[wrap(Some)]
                #[name = "top_label"]
                set_start_widget = &gtk::Label {
                    add_css_class: "t0",
                    add_css_class: "bold",
                },

                #[wrap(Some)]
                set_end_widget = &gtk::Box {
                    set_orientation: Orientation::Horizontal,
                    set_spacing: 5,
                    #[name = "back_btn"]
                    gtk::Button {
                        set_icon_name: icon_names::LEFT,
                        add_css_class: "no-bg",
                        add_css_class: "pill",
                    },
                    #[name = "forward_btn"]
                    gtk::Button {
                        set_icon_name: icon_names::RIGHT,
                        add_css_class: "no-bg",
                        add_css_class: "pill",
                    }
                }
            },
            #[name = "scroll"]
            gtk::ScrolledWindow {
                set_vscrollbar_policy: gtk::PolicyType::Never,
                set_hscrollbar_policy: gtk::PolicyType::Always,
                set_hexpand: true,
                set_hexpand_set: true,
                set_halign: Align::Fill,
                #[name = "list"]
                gtk::ListView {
                    set_orientation: Orientation::Horizontal,
                    add_css_class: "no-bg",
                    add_css_class: "card",
                    set_single_click_activate: true,
                }
            }
        }
    }
}
