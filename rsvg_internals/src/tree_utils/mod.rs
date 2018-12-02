use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub struct Node<T> {
    parent: Option<Weak<Node<T>>>, // optional; weak ref to parent
    first_child: RefCell<Option<Rc<Node<T>>>>,
    last_child: RefCell<Option<Weak<Node<T>>>>,
    next_sib: RefCell<Option<Rc<Node<T>>>>, // next sibling; strong ref
    prev_sib: RefCell<Option<Weak<Node<T>>>>, // previous sibling; weak ref
    pub data: T,
}

impl<T> Node<T> {
    pub fn get_parent(&self) -> Option<Rc<Node<T>>> {
        match self.parent {
            None => None,
            Some(ref weak_node) => Some(weak_node.upgrade().unwrap()),
        }
    }

    pub fn is_ancestor(ancestor: Rc<Node<T>>, descendant: Rc<Node<T>>) -> bool {
        let mut desc = Some(descendant.clone());

        while let Some(ref d) = desc.clone() {
            if Rc::ptr_eq(&ancestor, d) {
                return true;
            }

            desc = d.get_parent();
        }

        false
    }

    pub fn has_previous_sibling(&self) -> bool {
        !self.prev_sib.borrow().is_none()
    }

    pub fn has_next_sibling(&self) -> bool {
        !self.next_sib.borrow().is_none()
    }

    pub fn add_child(&self, child: &Rc<Node<T>>) {
        assert!(child.next_sib.borrow().is_none());
        assert!(child.prev_sib.borrow().is_none());

        if let Some(last_child_weak) = self.last_child.replace(Some(Rc::downgrade(child))) {
            if let Some(last_child) = last_child_weak.upgrade() {
                child.prev_sib.replace(Some(last_child_weak));
                last_child.next_sib.replace(Some(child.clone()));
                return;
            }
        }
        self.first_child.replace(Some(child.clone()));
    }

    pub fn children(&self) -> Children<T> {
        let last_child = self
            .last_child
            .borrow()
            .as_ref()
            .and_then(|child_weak| child_weak.upgrade());
        Children::new(self.first_child.borrow().clone(), last_child)
    }

    pub fn has_children(&self) -> bool {
        self.first_child.borrow().is_some()
    }

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

// An iterator over the Node's children
#[derive(Clone)]
pub struct Children<T> {
    next: Option<Rc<Node<T>>>,
    next_back: Option<Rc<Node<T>>>,
}

impl<T> Children<T> {
    fn new(next: Option<Rc<Node<T>>>, next_back: Option<Rc<Node<T>>>) -> Self {
        Self { next, next_back }
    }

    // true if self.next_back's next sibling is self.next
    fn finished(&self) -> bool {
        match &self.next_back {
            &Some(ref next_back) => {
                next_back
                    .next_sib
                    .borrow()
                    .clone()
                    .map(|rc| &*rc as *const Node<T>)
                    == self.next.clone().map(|rc| &*rc as *const Node<T>)
            }
            _ => true,
        }
    }
}

impl<T> Iterator for Children<T> {
    type Item = Rc<Node<T>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished() {
            return None;
        }
        self.next.take().and_then(|next| {
            self.next = next.next_sib.borrow().clone();
            Some(next)
        })
    }
}

impl<T> DoubleEndedIterator for Children<T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.finished() {
            return None;
        }
        self.next_back.take().and_then(|next_back| {
            self.next_back = next_back
                .prev_sib
                .borrow()
                .as_ref()
                .and_then(|sib_weak| sib_weak.upgrade());
            Some(next_back)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    #[test]
    fn node_is_its_own_ancestor() {
        let node = Rc::new(Node::<()>::new(None, ()));
        assert!(Node::is_ancestor(node.clone(), node.clone()));
    }

    #[test]
    fn node_is_ancestor_of_child() {
        let node = Rc::new(Node::<()>::new(None, ()));
        let child = Rc::new(Node::<()>::new(None, ()));

        node.add_child(&child);

        assert!(Node::is_ancestor(node.clone(), child.clone()));
        assert!(!Node::is_ancestor(child.clone(), node.clone()));
    }

    #[test]
    fn node_children_iterator() {
        let node = Rc::new(Node::<()>::new(None, ()));
        let child = Rc::new(Node::<()>::new(None, ()));
        let second_child = Rc::new(Node::<()>::new(None, ()));

        node.add_child(&child);
        node.add_child(&second_child);

        let mut children = node.children();

        let c = children.next();
        assert!(c.is_some());
        let c = c.unwrap();
        assert!(Rc::ptr_eq(&c, &child));

        let c = children.next_back();
        assert!(c.is_some());
        let c = c.unwrap();
        assert!(Rc::ptr_eq(&c, &second_child));

        assert!(children.next().is_none());
        assert!(children.next_back().is_none());
    }
}
