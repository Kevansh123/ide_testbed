use std::rc::Rc;
use futures_signals::signal::Mutable;

pub struct ContextMenuState {
    pub show_menu: Mutable<bool>,
    pub menu_position: Mutable<(i32, i32)>,
}


impl ContextMenuState {
    pub fn new() -> Rc<Self> {
        Rc::new(Self {
            show_menu: Mutable::new(false),
            menu_position: Mutable::new((0, 0)),
        })
    }
}