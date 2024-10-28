use std::{pin::Pin, rc::Rc};

use dominator::{clone, events, html, svg, Dom, EventOptions};
use dominator_bulma::{block, column, columns, icon, icon_text};
use futures::StreamExt;
use futures_signals::{signal::{self, Mutable, Signal, SignalExt}, signal_vec::{MutableVec, SignalVecExt}};
use crate::contextmenu::ContextMenuState;

pub mod editor;
pub mod welcome;

const TAB_HEIGHT: u32 = 48;

enum Activity {
    Editor(Rc<editor::Editor>),
    Welcome(Rc<welcome::Welcome>),
}

impl Activity {
    pub fn render(
        this: &Rc<Activity>,
        width: impl Signal<Item = u32> + 'static,
        height: impl Signal<Item = u32> + 'static
    ) -> Pin<Box<dyn Signal<Item = Option<dominator::Dom>>>> {
        match this.as_ref() {
            Activity::Editor(editor) => Box::pin(editor::Editor::render(editor, width, height)),
            Activity::Welcome(welcome) => Box::pin(welcome::Welcome::render(welcome, width, height)),
        }
    }

    pub fn label(&self) -> Dom {
        match self {
            Activity::Editor(editor) => editor.label(),
            Activity::Welcome(welcome) => welcome.label(),
        }
    }

    pub fn icon(&self) -> Dom {
        match self {
            Activity::Editor(editor) => editor.icon(),
            Activity::Welcome(welcome) => welcome.icon(),
        }
    }

    fn render_tab(
        this: &Rc<Activity>,
        panel: &Rc<ActivityPanel>
    ) -> Dom {
        let close_icon = svg!("svg", {
            .attr("height", "1em")
            .attr("viewBox", "0 0 24 24")
            .child(svg!("path", {
                .attr("d", CLOSE_ICON_PATH)
            }))
        });

        let mouse_over = Mutable::new(false);
        let mouse_over_close = Mutable::new(false);
        let is_active = panel.active_activity.signal_ref(clone!(this => move |active_activity| {
            active_activity.as_ref().is_some_and(|active_activity| Rc::ptr_eq(active_activity, &this))
        }));

        block!("py-3", "px-3", {
            .style("cursor", "pointer")
            .event(clone!(mouse_over => move |_: events::PointerOver| {
                mouse_over.set_neq(true);
            }))
            .event(clone!(mouse_over => move |_: events::PointerOut| {
                mouse_over.set_neq(false);
            }))
            .event(clone!(panel, this => move |_: events::PointerDown| {                
                panel.active_activity.set(Some(this.clone()))
            }))
            
            .class_signal("has-background-white", signal::or(is_active, mouse_over.signal()))
            .child(icon_text!({
                .child(icon!({
                    .child(this.icon())
                }))
                .child(this.label())
                .apply_if(matches!(**this, Activity::Editor(_)), |dom| {
                    dom.child(icon!({
                        .event(clone!(mouse_over_close => move |_: events::PointerOver| {
                            mouse_over_close.set_neq(true);
                        }))
                        .event(clone!(mouse_over_close => move |_: events::PointerOut| {
                            mouse_over_close.set_neq(false);
                        }))
                        .event_with_options(&EventOptions::preventable(), clone!(panel, this => move |ev: events::PointerDown| {
                            ev.stop_propagation();
                            panel.activities.lock_mut().retain(|activity| !Rc::ptr_eq(activity, &this));
                            let mut active_activity = panel.active_activity.lock_mut();
                            if active_activity.as_ref().is_some_and(|active_activity| Rc::ptr_eq(active_activity, &this)) {
                                *active_activity = panel.activities.lock_ref().first().cloned();
                            }
                        }))
                        .class_signal("has-background-white-ter", mouse_over_close.signal())
                        .class_signal("is-invisible", signal::not(mouse_over.signal()))
                        .child(close_icon)
                    }))
                })
            }))
        })
    }
}

pub struct ActivityPanel {
    activities: MutableVec<Rc<Activity>>,
    active_activity: Mutable<Option<Rc<Activity>>>,
    context_menu_state:Rc<ContextMenuState>
}

impl Default for ActivityPanel {
    fn default() -> Self {
        let welcome = Rc::new(Activity::Welcome(Rc::new(welcome::Welcome::new())));
        
        Self {
            activities: vec![welcome.clone()].into(),
            active_activity: Some(welcome).into(),
            context_menu_state: ContextMenuState::new()
        }
    }
}

const CLOSE_ICON_PATH: &str = "M19,6.41L17.59,5L12,10.59L6.41,5L5,6.41L10.59,12L5,17.59L6.41,19L12,13.41L17.59,19L19,17.59L13.41,12L19,6.41Z";

impl ActivityPanel {
    pub fn render(
        this: &Rc<ActivityPanel>,
        workspace_command_rx: crate::WorkspaceCommandReceiver,
        width: impl Signal<Item = u32> + 'static,
        height: impl Signal<Item = u32> + 'static
    ) -> dominator::Dom {
        let activity_count = this.activities.signal_vec_cloned().len().broadcast();
        let width = width.broadcast();
        let height = height.broadcast();
        let context_menu_state = this.context_menu_state.clone();
        
        columns!("is-gapless", "is-mobile", "is-multiline", {
            .future(workspace_command_rx.for_each(clone!(this => move |command| clone!(this => async move {
                match command {
                    crate::WorkspaceCommand::OpenFile(file) => {
                        let mut activities = this.activities.lock_mut();
                        let editor = activities.iter()
                            .find(|activity| match &***activity {
                                Activity::Editor(editor) => Rc::ptr_eq(&editor.file, &file),
                                _ => false,
                            })
                            .cloned()
                            .unwrap_or_else(move || {
                                let editor = Rc::new(Activity::Editor(Rc::new(editor::Editor::new(file))));
                                activities.push_cloned(editor.clone());
                                editor
                            });
                        this.active_activity.set(Some(editor));
                    },
                }
            }))))
            .child_signal(activity_count.signal().map(clone!(height => move |count| {
                (count == 0).then(|| Self::render_background(height.signal()))
            })))
            .child(column!("is-full", {
                .class("has-background-white-ter")
                .child(columns!("is-gapless", "is-mobile", {
                    .children_signal_vec(this.activities.signal_vec_cloned().map(clone!(this => move |activity| {
                        column!("is-narrow", {
                            .child(Activity::render_tab(&activity, &this))
                            .event_with_options(&EventOptions::preventable(), clone!(context_menu_state => move |event: events::ContextMenu| {
                                event.prevent_default();  
                                context_menu_state.show_menu.set(true); 
                                context_menu_state.menu_position.set((event.x(), event.y())); 
                            }))
                            .child_signal(context_menu_state.show_menu.signal_ref(clone!(context_menu_state => move |&show| {
                                if show {
                                    Some(html!("div", {
                                        .class("context-menu")
                                        .style("position", "absolute")
                                        .style("background-color", "lightgray")
                                        .style("border", "1px solid black")
                                        .style("padding", "10px")
                                        .style("z-index", "1000")
                                        .style_signal("left", context_menu_state.menu_position.signal_ref(|(x, _y)| {
                                            format!("{}px", x)
                                        }))
                                        .style_signal("top", context_menu_state.menu_position.signal_ref(|(_x, y)| {
                                            format!("{}px", y)
                                        }))
                                        .children(&mut [
                                            html!("div", {
                                                .text("Option 1")
                                                .style("cursor", "pointer")
                                                .event(clone!(context_menu_state => move |_event: events::MouseDown| {
                                                    web_sys::console::log_1(&"Option 1 clicked".into());
                                                    context_menu_state.show_menu.set_neq(false); // Hide the menu after clicking
                                                }))
                                            }),
                                            html!("div", {
                                                .text("Option 2")
                                                .style("cursor", "pointer")
                                                .event(clone!(context_menu_state => move |_event: events::MouseDown| {
                                                    web_sys::console::log_1(&"Option 2 clicked".into());
                                                    context_menu_state.show_menu.set_neq(false); // Hide the menu after clicking
                                                }))
                                            }),
                                            html!("div", {
                                                .text("Option 3")
                                                .style("cursor", "pointer")
                                                .event(clone!(context_menu_state => move |_event: events::MouseDown| {
                                                    web_sys::console::log_1(&"Option 3 clicked".into());
                                                    context_menu_state.show_menu.set_neq(false); // Hide the menu after clicking
                                                }))
                                            }),
                                            html!("div", {
                                                .text("Option 4")
                                                .style("cursor", "pointer")
                                                .event(clone!(context_menu_state => move |_event: events::MouseDown| {
                                                    web_sys::console::log_1(&"Option 4 clicked".into());
                                                    context_menu_state.show_menu.set_neq(false); // Hide the menu after clicking
                                                }))
                                            })
                                        ])
                                    }))
                                } else {
                                    None
                                }
                            })))
                        })
                    })))
                }))
            }))
            .child_signal(this.active_activity
                .signal_cloned()
                .map(move |activity: Option<Rc<Activity>>| activity
                    .map(clone!(width, height => move |activity| column!("is-full", {
                        .child_signal(Activity::render(
                            &activity,
                            width.signal(),
                            height.signal_ref(|height| height.saturating_sub(TAB_HEIGHT))))
                    })))
                )
            )
        })
    }

    fn render_background(
        height: impl Signal<Item = u32> + 'static
    ) -> Dom {
        column!("is-full", {
            .style_signal("height", height.map(|height| format!("{height}px")))
            .style("background-image", "url('images/background.png')")
            .style("background-repeat", "no-repeat")
            .style("background-position", "center")
            .style("background-size", "auto 40%")
        })
    }
}
