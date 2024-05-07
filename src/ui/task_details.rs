// rusttimetrack - Track your time without being tracked
// Copyright (C) 2022  Ricky Kresslein <rk@lakoliu.com>
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <https://www.gnu.org/licenses/>.

use adw::subclass::prelude::*;
use chrono::{offset::TimeZone, DateTime, Local, NaiveDateTime, ParseError, Duration};
use gettextrs::*;
use glib::clone;
use gtk::{glib, prelude::*, CompositeTemplate};
use itertools::Itertools;

use crate::database;
use crate::settings_manager;
use crate::ui::rusttimetrackWindow;
use crate::rusttimetrackApplication;

mod imp {
    use super::*;
    use glib::subclass;
    use std::cell::RefCell;

    #[derive(Debug, Default, CompositeTemplate)]
    #[template(resource = "/com/lakoliu/rusttimetrack/gtk/task_details.ui")]
    pub struct FurTaskDetails {
        #[template_child]
        pub headerbar: TemplateChild<gtk::HeaderBar>,
        #[template_child]
        pub dialog_title: TemplateChild<adw::WindowTitle>,
        #[template_child]
        pub scrolled_window: TemplateChild<gtk::ScrolledWindow>,

        #[template_child]
        pub task_name_label: TemplateChild<gtk::Label>,
        #[template_child]
        pub edit_task_names_btn: TemplateChild<gtk::Button>,

        #[template_child]
        pub main_box: TemplateChild<gtk::Box>,

        #[template_child]
        pub add_similar_btn: TemplateChild<gtk::Button>,
        #[template_child]
        pub delete_all_btn: TemplateChild<gtk::Button>,

        pub all_boxes: RefCell<Vec<gtk::Box>>,
        pub all_task_ids: RefCell<Vec<i32>>,
        pub this_task_name: RefCell<String>,
        pub this_task_tags: RefCell<String>,
        pub this_day: RefCell<String>,
        pub orig_tags: RefCell<String>,
        pub orig_name_with_tags: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for FurTaskDetails {
        const NAME: &'static str = "FurTaskDetails";
        type ParentType = adw::Window;
        type Type = super::FurTaskDetails;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }

        fn instance_init(obj: &subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for FurTaskDetails {
        fn constructed(&self) {
            let obj = self.obj();
            obj.setup_signals();
            obj.setup_delete_all();
            obj.setup_add_similar();
            self.parent_constructed();
        }
    }

    impl WidgetImpl for FurTaskDetails {}

    impl WindowImpl for FurTaskDetails {}

    impl AdwWindowImpl for FurTaskDetails {}
}

glib::wrapper! {
    pub struct FurTaskDetails(ObjectSubclass<imp::FurTaskDetails>)
        @extends gtk::Widget, gtk::Window, adw::Window;
}

impl FurTaskDetails {
    pub fn new() -> Self {
        let dialog: Self = glib::Object::new::<FurTaskDetails>();

        let window = rusttimetrackWindow::default();
        dialog.set_transient_for(Some(&window));

        let app = rusttimetrackApplication::default();
        app.add_window(&window);

        dialog
    }

    pub fn setup_widgets(&self, mut task_group: Vec<database::Task>) {
        let imp = imp::FurTaskDetails::from_obj(self);

        imp.task_name_label.set_text(&task_group[0].task_name);
        let this_day_str = DateTime::parse_from_rfc3339(&task_group[0].start_time).unwrap();
        *imp.this_task_name.borrow_mut() = task_group[0].task_name.clone();
        *imp.this_task_tags.borrow_mut() = task_group[0].tags.clone();
        *imp.this_day.borrow_mut() = this_day_str.format("%F").to_string();
        *imp.orig_tags.borrow_mut() = task_group[0].tags.clone();
        *imp.orig_name_with_tags.borrow_mut() = task_group[0].task_name.clone() + " #" + &task_group[0].tags.clone();

        for task in task_group.clone() {
            imp.all_task_ids.borrow_mut().push(task.id);
        }

        // Using ISO 8601 format until a localized option is possible
        let time_formatter = "%F %H:%M:%S";
        let time_formatter_no_secs = "%F %H:%M";
        let task_group_len = task_group.len();
        task_group.reverse();
        for task in task_group {
            let task_box = gtk::Box::new(gtk::Orientation::Horizontal, 5);
            task_box.set_homogeneous(true);

            let start_time = DateTime::parse_from_rfc3339(&task.start_time).unwrap();
            let mut start_time_str = start_time.format("%H:%M:%S").to_string();
            if !settings_manager::get_bool("show-seconds") {
                start_time_str = start_time.format("%H:%M").to_string();
            }
            let start = gtk::Button::new();
            start.set_label(&start_time_str);
            task_box.append(&start);

            let stop_time = DateTime::parse_from_rfc3339(&task.stop_time).unwrap();
            let mut stop_time_str = stop_time.format("%H:%M:%S").to_string();
            if !settings_manager::get_bool("show-seconds") {
                stop_time_str = stop_time.format("%H:%M").to_string();
            }
            let stop = gtk::Button::new();
            stop.set_label(&stop_time_str);
            task_box.append(&stop);

            let total_time = stop_time - start_time;
            let total_time = total_time.num_seconds();
            let h = total_time / 3600;
            let m = total_time % 3600 / 60;
            let s = total_time % 60;
            let mut total_time_str = format!("{:02}:{:02}:{:02}", h, m, s);
            if !settings_manager::get_bool("show-seconds") {
                total_time_str = format!("{:02}:{:02}", h, m);
            }
            let total = gtk::Button::new();
            total.set_label(&total_time_str);
            total.add_css_class("inactive-button");
            total.set_hexpand(false);
            task_box.append(&total);

            imp.main_box.append(&task_box);
            imp.all_boxes.borrow_mut().push(task_box);

            start.connect_clicked(clone!(@weak self as this => move |_|{
                let dialog = gtk::MessageDialog::new(
                    Some(&this),
                    gtk::DialogFlags::MODAL,
                    gtk::MessageType::Question,
                    gtk::ButtonsType::OkCancel,
                    &format!("<span size='x-large' weight='bold'>{}</span>", &gettext("Edit Task")),
                );
                dialog.set_use_markup(true);

                let message_area = dialog.message_area().downcast::<gtk::Box>().unwrap();
                let vert_box = gtk::Box::new(gtk::Orientation::Vertical, 5);
                let task_name_edit = gtk::Entry::new();
                task_name_edit.set_text(&task.task_name);
                let task_tags_edit = gtk::Entry::new();
                let tags_placeholder = format!("#{}", &gettext("Tags"));
                task_tags_edit.set_placeholder_text(Some(&tags_placeholder));
                let mut task_tags: String = task.tags.clone();
                if !task.tags.trim().is_empty() {
                    task_tags = format!("#{}", task.tags);
                    task_tags_edit.set_text(&task_tags);
                }
                let labels_box = gtk::Box::new(gtk::Orientation::Horizontal, 5);
                labels_box.set_homogeneous(true);
                let start_label = gtk::Label::new(Some(&gettext("Start")));
                start_label.add_css_class("title-4");
                let stop_label = gtk::Label::new(Some(&gettext("Stop")));
                stop_label.add_css_class("title-4");
                let times_box = gtk::Box::new(gtk::Orientation::Horizontal, 5);
                times_box.set_homogeneous(true);

                let mut start_time_w_year = start_time.format(time_formatter).to_string();
                if !settings_manager::get_bool("show-seconds") {
                    start_time_w_year = start_time.format(time_formatter_no_secs).to_string();
                }
                let mut stop_time_w_year = stop_time.format(time_formatter).to_string();
                if !settings_manager::get_bool("show-seconds") {
                    stop_time_w_year = stop_time.format(time_formatter_no_secs).to_string();
                }
                let start_time_edit = gtk::Entry::new();
                start_time_edit.set_text(&start_time_w_year);
                let stop_time_edit = gtk::Entry::new();
                stop_time_edit.set_text(&stop_time_w_year);

                let instructions = gtk::Label::new(Some(
                    &gettext("*Use the format YYYY-MM-DD HH:MM:SS")));
                if !settings_manager::get_bool("show-seconds") {
                    instructions.set_text(&gettext("*Use the format YYYY-MM-DD HH:MM"));
                }
                instructions.set_visible(false);
                instructions.add_css_class("error_message");

                let time_error = gtk::Label::new(Some(
                    &gettext("*Start time cannot be later than stop time.")));
                time_error.set_visible(false);
                time_error.add_css_class("error_message");

                let future_error = gtk::Label::new(Some(
                    &gettext("*Time cannot be in the future.")));
                future_error.set_visible(false);
                future_error.add_css_class("error_message");

                let delete_task_btn = gtk::Button::new();
                delete_task_btn.set_icon_name("user-trash-symbolic");
                delete_task_btn.set_tooltip_text(Some(&gettext("Delete task")));
                delete_task_btn.set_hexpand(false);
                delete_task_btn.set_vexpand(false);
                delete_task_btn.set_halign(gtk::Align::End);

                vert_box.append(&task_name_edit);
                vert_box.append(&task_tags_edit);
                labels_box.append(&start_label);
                labels_box.append(&stop_label);
                times_box.append(&start_time_edit);
                times_box.append(&stop_time_edit);
                vert_box.append(&labels_box);
                vert_box.append(&times_box);
                vert_box.append(&instructions);
                vert_box.append(&time_error);
                vert_box.append(&future_error);
                message_area.append(&delete_task_btn);
                message_area.append(&vert_box);

                delete_task_btn.connect_clicked(clone!(
                    @strong task, @strong dialog, @weak this => move |_| {

                    let delete_confirmation = gtk::MessageDialog::with_markup(
                        Some(&dialog),
                        gtk::DialogFlags::MODAL,
                        gtk::MessageType::Question,
                        gtk::ButtonsType::OkCancel,
                        Some(&format!("<span size='x-large' weight='bold'>{}</span>", &gettext("Delete task?"))),
                    );
                    let delete_btn = delete_confirmation.widget_for_response(gtk::ResponseType::Ok).unwrap();
                    delete_btn.add_css_class("destructive-action");

                    delete_confirmation.connect_response(clone!(
                        @strong dialog,
                        @strong delete_confirmation => move |_, resp| {
                        if resp == gtk::ResponseType::Ok {
                            let _ = database::delete_by_id(task.id);
                            if task_group_len == 1 {
                                delete_confirmation.close();
                                dialog.close();
                                this.close();
                                let window = rusttimetrackWindow::default();
                                window.reset_history_box();
                            } else {
                                delete_confirmation.close();
                                this.clear_task_list();
                                dialog.close();
                            }
                        } else {
                            delete_confirmation.close();
                        }
                    }));

                    if settings_manager::get_bool("delete-confirmation") {
                        delete_confirmation.show();
                    } else {
                        delete_confirmation.response(gtk::ResponseType::Ok);
                    }
                }));


                dialog.connect_response(
                    clone!(@strong dialog,
                        @strong task.task_name as name,
                        @strong task.start_time as start_time,
                        @strong task.stop_time as stop_time,
                        @strong task.tags as tags => move |_ , resp| {
                        if resp == gtk::ResponseType::Ok {
                            instructions.set_visible(false);
                            time_error.set_visible(false);
                            future_error.set_visible(false);
                            let mut start_successful = false;
                            let mut stop_successful = false;
                            let mut do_not_close = false;
                            let mut new_start_time_edited: String = "".to_string();
                            let mut new_start_time_local = Local::now();
                            let new_stop_time_edited: String;
                            if start_time_edit.text() != start_time_w_year {
                                let new_start_time_str = start_time_edit.text();
                                let new_start_time: Result<NaiveDateTime, ParseError>;
                                if settings_manager::get_bool("show-seconds") {
                                    new_start_time = NaiveDateTime::parse_from_str(
                                                        &new_start_time_str,
                                                        time_formatter);
                                } else {
                                    new_start_time = NaiveDateTime::parse_from_str(
                                                            &new_start_time_str,
                                                            time_formatter_no_secs);
                                }

                                if let Err(_) = new_start_time {
                                    instructions.set_visible(true);
                                    do_not_close = true;
                                } else if (Local::now() - Local.from_local_datetime(&new_start_time.unwrap()).unwrap()).num_seconds() < 0 {
                                    future_error.set_visible(true);
                                    do_not_close = true;
                                }else {
                                    new_start_time_local = Local.from_local_datetime(&new_start_time.unwrap()).unwrap();
                                    new_start_time_edited = new_start_time_local.to_rfc3339();
                                    start_successful = true;
                                }
                            }
                            if stop_time_edit.text() != stop_time_w_year {
                                let new_stop_time_str = stop_time_edit.text();
                                let new_stop_time: Result<NaiveDateTime, ParseError>;
                                if settings_manager::get_bool("show-seconds") {
                                    new_stop_time = NaiveDateTime::parse_from_str(
                                                        &new_stop_time_str,
                                                        time_formatter);
                                } else {
                                    new_stop_time = NaiveDateTime::parse_from_str(
                                                            &new_stop_time_str,
                                                            time_formatter_no_secs);
                                }
                                if let Err(_) = new_stop_time {
                                    instructions.set_visible(true);
                                    do_not_close = true;
                                } else {
                                    let new_stop_time = Local.from_local_datetime(&new_stop_time.unwrap()).unwrap();
                                    new_stop_time_edited = new_stop_time.to_rfc3339();
                                    if start_successful {
                                        if (new_stop_time - new_start_time_local).num_seconds() >= 0 {
                                            database::update_stop_time(task.id, new_stop_time_edited.clone())
                                                .expect("Failed to update stop time.");
                                            database::update_start_time(task.id, new_start_time_edited.clone())
                                                .expect("Failed to update start time.");
                                        }
                                    } else {
                                        let old_start_time = DateTime::parse_from_rfc3339(&start_time);
                                        let old_start_time = old_start_time.unwrap().with_timezone(&Local);
                                        if (Local::now() - new_stop_time).num_seconds() < 0 {
                                            future_error.set_visible(true);
                                            do_not_close = true;
                                        } else if (new_stop_time - old_start_time).num_seconds() >= 0 {
                                            database::update_stop_time(task.id, new_stop_time_edited)
                                                .expect("Failed to update stop time.");
                                        } else {
                                            time_error.set_visible(true);
                                            do_not_close = true;
                                        }
                                    }
                                    stop_successful = true;
                                }
                            }
                            if task_name_edit.text() != name {
                                database::update_task_name(task.id, task_name_edit.text().to_string())
                                    .expect("Failed to update task name.");
                            }

                            if task_tags_edit.text() != task_tags {
                                let new_tags = task_tags_edit.text();
                                let mut split_tags: Vec<&str> = new_tags.trim().split("#").collect();
	                            split_tags = split_tags.iter().map(|x| x.trim()).collect();
	                            // Don't allow empty tags
	                            split_tags.retain(|&x| !x.trim().is_empty());
	                            // Handle duplicate tags before they are saved
	                            split_tags = split_tags.into_iter().unique().collect();
	                            // Lowercase tags
	                            let lower_tags: Vec<String> = split_tags.iter().map(|x| x.to_lowercase()).collect();
	                            let new_tag_list = lower_tags.join(" #");
                                database::update_tags(task.id, new_tag_list)
                                    .expect("Failed to update tags.");
                            }

                            if start_successful && !stop_successful {
                                let old_stop_time = DateTime::parse_from_rfc3339(&stop_time);
                                let old_stop_time = old_stop_time.unwrap().with_timezone(&Local);
                                if (old_stop_time - new_start_time_local).num_seconds() >= 0 {
                                    database::update_start_time(task.id, new_start_time_edited)
                                        .expect("Failed to update start time.");
                                } else {
                                    time_error.set_visible(true);
                                    do_not_close = true;
                                }
                            }

                            if !do_not_close {
                                this.clear_task_list();
                                dialog.close();
                            }

                        } else {
                            // If Cancel, close dialog and do nothing.
                            dialog.close();
                        }
                    }),
                );

                dialog.show();
            }));

            stop.connect_clicked(move |_| {
                start.emit_clicked();
            });
        }
    }

    fn clear_task_list(&self) {
        let imp = imp::FurTaskDetails::from_obj(&self);

        for task_box in &*imp.all_boxes.borrow() {
            imp.main_box.remove(task_box);
        }

        imp.all_boxes.borrow_mut().clear();
        // Get list from database by a vec of IDs
        let updated_list = database::get_list_by_id(imp.all_task_ids.clone().borrow().to_vec());
        let mut updated_list = updated_list.unwrap();
        // Check if dates in all_task_ids list match this date
        // and if not, delete them.
        updated_list.retain(|task| {
            let delete = {
                let start_time = DateTime::parse_from_rfc3339(&task.start_time).unwrap();
                let start_time_str = start_time.format("%F").to_string();
                if imp.this_day.borrow().to_string() != start_time_str
                    || imp.task_name_label.text() != task.task_name
                    || imp.orig_tags.borrow().to_string() != task.tags
                {
                    false
                } else {
                    true
                }
            };
            delete
        });

        imp.all_task_ids.borrow_mut().clear();
        let window = rusttimetrackWindow::default();
        window.reset_history_box();
        if updated_list.len() > 0 {
            self.setup_widgets(updated_list);
        } else {
            self.close();
        }
    }

    fn setup_signals(&self) {
        let imp = imp::FurTaskDetails::from_obj(self);

        // Add headerbar to dialog when scrolled far
        imp.scrolled_window.vadjustment().connect_value_notify(
            clone!(@weak self as this => move |adj|{
                let imp = imp::FurTaskDetails::from_obj(&this);
                if adj.value() < 120.0 {
                    imp.headerbar.add_css_class("hidden");
                    imp.dialog_title.set_visible(false);
                }else {
                    imp.headerbar.remove_css_class("hidden");
                    imp.dialog_title.set_visible(true);
                }
            }),
        );

        // Make dialog header smaller if the name is long
        imp.task_name_label.connect_label_notify(|label| {
            let large_title = !(label.text().len() > 25);

            if large_title {
                label.remove_css_class("title-2");
                label.add_css_class("title-1");
            } else {
                label.remove_css_class("title-1");
                label.add_css_class("title-2");
            }
        });

        // Change all task names at once
        imp.edit_task_names_btn.connect_clicked(clone!(@weak self as this => move|_| {
            let dialog = gtk::MessageDialog::new(
                Some(&this),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Question,
                gtk::ButtonsType::OkCancel,
                &format!("<span size='x-large' weight='bold'>{}</span>", &gettext("Edit Task")),
            );
            dialog.set_use_markup(true);

            let message_area = dialog.message_area().downcast::<gtk::Box>().unwrap();
            let new_name_entry = gtk::Entry::new();
            new_name_entry.set_placeholder_text(Some(&gettext("New Name #tags")));
            let imp3 = imp::FurTaskDetails::from_obj(&this);
            new_name_entry.set_text(&imp3.orig_name_with_tags.borrow().to_string());
            let cant_be_empty = gtk::Label::new(Some(&gettext("Task name cannot be empty.")));
            cant_be_empty.add_css_class("error_message");
            cant_be_empty.hide();
            message_area.append(&new_name_entry);
            message_area.append(&cant_be_empty);

            dialog.connect_response(move |dialog, resp| {
                let window = rusttimetrackWindow::default();
                cant_be_empty.hide();
                if resp == gtk::ResponseType::Ok {
                    let new_name_text = new_name_entry.text();
                    let mut split_tags: Vec<&str> = new_name_text.trim().split("#").collect();
                    // Remove task name from tags list
                    let new_task_name = *split_tags.first().unwrap();
                    split_tags.remove(0);
                    // Trim whitespace around each tag
                    split_tags = split_tags.iter().map(|x| x.trim()).collect();
                    // Don't allow empty tags
                    split_tags.retain(|&x| !x.trim().is_empty());
                    // Handle duplicate tags before they are ever saved
                    split_tags = split_tags.into_iter().unique().collect();
                    // Lowercase tags
                    let lower_tags: Vec<String> = split_tags.iter().map(|x| x.to_lowercase()).collect();
                    let tag_list = lower_tags.join(" #");

                    if !new_task_name.is_empty() {
                        // Change all task names & tags
                        let imp2 = imp::FurTaskDetails::from_obj(&this);
                        for id in &*imp2.all_task_ids.borrow() {
                            database::update_task_name(*id, new_task_name.trim().to_string())
                                .expect("Could not update group of task names");
                            database::update_tags(*id, tag_list.clone())
                                .expect("Could not update tags of group");
                        }
                        imp2.all_task_ids.borrow_mut().clear();
                        window.reset_history_box();
                        dialog.close();
                        this.close();
                    } else {
                        // Clear any spaces from entry
                        new_name_entry.set_text("");
                        cant_be_empty.show();
                    }
                } else if resp == gtk::ResponseType::Cancel {
                    dialog.close();
                }
            });

            dialog.show();
        }));
    }

    fn setup_delete_all(&self) {
        let imp = imp::FurTaskDetails::from_obj(self);
        imp.delete_all_btn.connect_clicked(clone!(@weak self as this => move |_|{
            let dialog = gtk::MessageDialog::with_markup(
                Some(&this),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Warning,
                gtk::ButtonsType::None,
                Some(&format!("<span size='large'>{}</span>", &gettext("Delete All?"))),
            );
            dialog.set_secondary_text(Some(&gettext("This will delete all occurrences of this task on this day.")));
            dialog.add_buttons(&[
                (&gettext("Cancel"), gtk::ResponseType::Reject),
                (&gettext("Delete"), gtk::ResponseType::Accept)
            ]);
            let delete_btn = dialog.widget_for_response(gtk::ResponseType::Accept).unwrap();
            delete_btn.add_css_class("destructive-action");

            dialog.connect_response(clone!(@strong dialog => move |_,resp|{
                if resp == gtk::ResponseType::Accept {
                    this.delete_all();
                    dialog.close();
                    this.close();
                    let window = rusttimetrackWindow::default();
                    window.reset_history_box();
                } else {
                    dialog.close();
                }
            }));

            if settings_manager::get_bool("delete-confirmation") {
                dialog.show();
            } else {
                dialog.response(gtk::ResponseType::Accept);
            }

        }));
    }

    fn setup_add_similar(&self) {
        let imp = imp::FurTaskDetails::from_obj(self);
        imp.add_similar_btn.connect_clicked(clone!(@weak self as this => move |_|{
            let imp2 = imp::FurTaskDetails::from_obj(&this);
            let dialog = gtk::MessageDialog::new(
                Some(&this),
                gtk::DialogFlags::MODAL,
                gtk::MessageType::Question,
                gtk::ButtonsType::None,
                &format!("<span size='x-large' weight='bold'>{}</span>", &gettext("New Task")),
            );
            dialog.set_use_markup(true);
            dialog.add_buttons(&[
                (&gettext("Cancel"), gtk::ResponseType::Cancel),
                (&gettext("Add"), gtk::ResponseType::Ok)
            ]);

            let message_area = dialog.message_area().downcast::<gtk::Box>().unwrap();
            let vert_box = gtk::Box::new(gtk::Orientation::Vertical, 5);
            let task_name_edit = gtk::Entry::new();
            task_name_edit.set_placeholder_text(Some(&imp2.this_task_name.borrow()));
            task_name_edit.set_text(&imp2.this_task_name.borrow());
            let task_tags_edit = gtk::Entry::new();
            let tags_placeholder = format!("#{}", imp2.this_task_tags.borrow());
            task_tags_edit.set_placeholder_text(Some(&tags_placeholder));
            task_tags_edit.set_text(&tags_placeholder);

            let labels_box = gtk::Box::new(gtk::Orientation::Horizontal, 5);
            labels_box.set_homogeneous(true);
            let start_label = gtk::Label::new(Some(&gettext("Start")));
            start_label.add_css_class("title-4");
            let stop_label = gtk::Label::new(Some(&gettext("Stop")));
            stop_label.add_css_class("title-4");
            let times_box = gtk::Box::new(gtk::Orientation::Horizontal, 5);
            times_box.set_homogeneous(true);

            let stop_time = Local::now();
            let start_time = stop_time - Duration::minutes(1);

            let time_formatter = "%F %H:%M:%S";
            let time_formatter_no_secs = "%F %H:%M";

            let mut start_time_w_year = start_time.format(time_formatter).to_string();
            if !settings_manager::get_bool("show-seconds") {
                start_time_w_year = start_time.format(time_formatter_no_secs).to_string();
            }
            let mut stop_time_w_year = stop_time.format(time_formatter).to_string();
            if !settings_manager::get_bool("show-seconds") {
                stop_time_w_year = stop_time.format(time_formatter_no_secs).to_string();
            }
            let start_time_edit = gtk::Entry::new();
            start_time_edit.set_text(&start_time_w_year);
            let stop_time_edit = gtk::Entry::new();
            stop_time_edit.set_text(&stop_time_w_year);

            let instructions = gtk::Label::new(Some(
                &gettext("*Use the format YYYY-MM-DD HH:MM:SS")));
            if !settings_manager::get_bool("show-seconds") {
                instructions.set_text(&gettext("*Use the format YYYY-MM-DD HH:MM"));
            }
            instructions.set_visible(false);
            instructions.add_css_class("error_message");

            let time_error = gtk::Label::new(Some(
                &gettext("*Start time cannot be later than stop time.")));
            time_error.set_visible(false);
            time_error.add_css_class("error_message");

            let future_error = gtk::Label::new(Some(
                &gettext("*Time cannot be in the future.")));
            future_error.set_visible(false);
            future_error.add_css_class("error_message");

            let name_error = gtk::Label::new(Some(
                &gettext("*Task name cannot be blank.")));
            name_error.set_visible(false);
            name_error.add_css_class("error_message");

            vert_box.append(&task_name_edit);
            vert_box.append(&task_tags_edit);
            labels_box.append(&start_label);
            labels_box.append(&stop_label);
            times_box.append(&start_time_edit);
            times_box.append(&stop_time_edit);
            vert_box.append(&labels_box);
            vert_box.append(&times_box);
            vert_box.append(&instructions);
            vert_box.append(&time_error);
            vert_box.append(&future_error);
            vert_box.append(&name_error);
            message_area.append(&vert_box);

            dialog.connect_response(clone!(@strong dialog => move |_ , resp| {
                if resp == gtk::ResponseType::Ok {
                    instructions.set_visible(false);
                    time_error.set_visible(false);
                    future_error.set_visible(false);
                    name_error.set_visible(false);
                    let mut do_not_close = false;
                    let mut new_start_time_local = Local::now();
                    let mut new_stop_time_local = Local::now();

                    // Task Name
                    if task_name_edit.text().trim().is_empty() {
                        name_error.set_visible(true);
                        do_not_close = true;
                    }

                    // Start Time
                    let new_start_time_str = start_time_edit.text();
                    let new_start_time: Result<NaiveDateTime, ParseError>;
                    if settings_manager::get_bool("show-seconds") {
                        new_start_time = NaiveDateTime::parse_from_str(
                                            &new_start_time_str,
                                            time_formatter);
                    } else {
                        new_start_time = NaiveDateTime::parse_from_str(
                                                &new_start_time_str,
                                                time_formatter_no_secs);
                    }
                    if let Err(_) = new_start_time {
                        instructions.set_visible(true);
                        do_not_close = true;
                    } else {
                        new_start_time_local = Local.from_local_datetime(&new_start_time.unwrap()).unwrap();
                        if (Local::now() - new_start_time_local).num_seconds() < 0 {
                            future_error.set_visible(true);
                            do_not_close = true;
                        }
                    }

                    // Stop Time
                    let new_stop_time_str = stop_time_edit.text();
                    let new_stop_time: Result<NaiveDateTime, ParseError>;
                    if settings_manager::get_bool("show-seconds") {
                        new_stop_time = NaiveDateTime::parse_from_str(
                                            &new_stop_time_str,
                                            time_formatter);
                    } else {
                        new_stop_time = NaiveDateTime::parse_from_str(
                                                &new_stop_time_str,
                                                time_formatter_no_secs);
                    }
                    if let Err(_) = new_stop_time {
                        instructions.set_visible(true);
                        do_not_close = true;
                    } else {
                        new_stop_time_local = Local.from_local_datetime(&new_stop_time.unwrap()).unwrap();
                        if (Local::now() - new_stop_time_local).num_seconds() < 0 {
                            future_error.set_visible(true);
                            do_not_close = true;
                        }
                    }

                    // Start time can't be later than stop time
                    if !do_not_close && (new_stop_time_local - new_start_time_local).num_seconds() < 0 {
                        time_error.set_visible(true);
                        do_not_close = true;
                    }

                    // Tags
                    let mut new_tag_list = "".to_string();
                    if !task_tags_edit.text().trim().is_empty() {
                        let new_tags = task_tags_edit.text();
                        let mut split_tags: Vec<&str> = new_tags.trim().split("#").collect();
                        split_tags = split_tags.iter().map(|x| x.trim()).collect();
                        // Don't allow empty tags
                        split_tags.retain(|&x| !x.trim().is_empty());
                        // Handle duplicate tags before they are saved
                        split_tags = split_tags.into_iter().unique().collect();
                        // Lowercase tags
                        let lower_tags: Vec<String> = split_tags.iter().map(|x| x.to_lowercase()).collect();
                        new_tag_list = lower_tags.join(" #");
                    }

                    if !do_not_close {
                        let _ = database::db_write(task_name_edit.text().trim(),
                                                    new_start_time_local,
                                                    new_stop_time_local,
                                                    new_tag_list);
                        let window = rusttimetrackWindow::default();
                        window.reset_history_box();
                        dialog.close();
                        this.close();
                    }

                } else if resp == gtk::ResponseType::Cancel {
                    dialog.close();
                }
            }));

            dialog.show();
        }));
    }

    fn delete_all(&self) {
        let imp = imp::FurTaskDetails::from_obj(self);
        let _ = database::delete_by_ids(imp.all_task_ids.borrow().to_vec());
    }
}
