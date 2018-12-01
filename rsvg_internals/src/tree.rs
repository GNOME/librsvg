use std::cell::Cell;
use std::rc::Rc;

use node::{Node, NodeType};
use state::ComputedValues;

pub struct Tree {
    root: Rc<Node>,
    already_cascaded: Cell<bool>,
}

impl Tree {
    pub fn new(root: &Rc<Node>) -> Tree {
        Tree {
            root: root.clone(),
            already_cascaded: Cell::new(false),
        }
    }

    pub fn cascade(&self) {
        if !self.already_cascaded.get() {
            self.already_cascaded.set(true);
            let values = ComputedValues::default();
            self.root.cascade(&values);
        }
    }

    pub fn root(&self) -> Rc<Node> {
        self.root.clone()
    }

    pub fn root_is_svg(&self) -> bool {
        self.root.get_type() == NodeType::Svg
    }
}
