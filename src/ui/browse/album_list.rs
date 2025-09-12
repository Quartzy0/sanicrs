use relm4::WidgetTemplate;
use relm4::adw::gtk;
use relm4::adw::prelude::*;
use relm4::adw::gtk::{Orientation, Align};

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
                        set_label: "<",
                        add_css_class: "no-bg",
                        add_css_class: "bold",
                        // connect_clicked => BrowseMsg::ScrollNewest(-100)
                    },
                    #[name = "forward_btn"]
                    gtk::Button {
                        set_label: ">",
                        add_css_class: "no-bg",
                        add_css_class: "bold",
                        // connect_clicked => BrowseMsg::ScrollNewest(100)
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
                    add_css_class: "album-list",
                    // set_factory: Some(&model.album_factory),
                    set_single_click_activate: true,
                    /*connect_activate[sender] => move |view, index| {
                        let model = view.model();
                        if let Some(model) = model {
                            let album: AlbumObject = model.item(index)
                                .expect("Item at index clicked expected to exist")
                                .downcast::<AlbumObject>()
                                .expect("Item expected to be AlbumObject");
                            sender.input(BrowseMsg::ViewAlbum(album));
                        }
                    }*/
                }
            }
        }
    }
}
