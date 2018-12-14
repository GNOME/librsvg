use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub type NodeRef<T> = Rc<Node<T>>;
pub type NodeWeakRef<T> = Weak<Node<T>>;

pub struct Node<T> {
    pub parent: Option<NodeWeakRef<T>>, // optional; weak ref to parent
    pub first_child: RefCell<Option<NodeRef<T>>>,
    pub last_child: RefCell<Option<Weak<Node<T>>>>,
    pub next_sib: RefCell<Option<NodeRef<T>>>, // next sibling; strong ref
    pub prev_sib: RefCell<Option<NodeWeakRef<T>>>, // previous sibling; weak ref
    pub data: T,
}

impl<T> Node<T> {
    pub fn get_parent(&self) -> Option<NodeRef<T>> {
        match self.parent {
            None => None,
            Some(ref weak_node) => Some(weak_node.upgrade().unwrap()),
        }
    }

    pub fn is_ancestor(ancestor: NodeRef<T>, descendant: NodeRef<T>) -> bool {
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

    pub fn add_child(&self, child: &NodeRef<T>) {
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

/// Prevent stack overflow when recursively dropping nodes
///
/// Dropping nodes is recursive, since a node owns strong references
/// to its next sibling and its first child.  When there is an SVG
/// with a flat hierarchy of a few hundred thousand elements,
/// recursively dropping these siblings can cause stack overflow.
///
/// Here, we convert recursion to an explicit heap-allocated stack of
/// nodes that need to be dropped.  This technique is borrowed from
/// [kuchiki]'s tree implementation.
///
/// [kuchiki]: https://github.com/kuchiki-rs/kuchiki/blob/master/src/tree.rs
impl<T> Drop for Node<T> {
    fn drop(&mut self) {
        let mut stack = Vec::new();

        if let Some(rc) = take_if_unique_strong(&self.first_child) {
            non_recursive_drop_unique_rc(rc, &mut stack);
        }

        if let Some(rc) = take_if_unique_strong(&self.next_sib) {
            non_recursive_drop_unique_rc(rc, &mut stack);
        }

        fn non_recursive_drop_unique_rc<T>(mut rc: NodeRef<T>, stack: &mut Vec<NodeRef<T>>) {
            loop {
                if let Some(child) = take_if_unique_strong(&rc.first_child) {
                    stack.push(rc);
                    rc = child;
                    continue;
                }

                if let Some(sibling) = take_if_unique_strong(&rc.next_sib) {
                    rc = sibling;
                    continue;
                }

                if let Some(parent) = stack.pop() {
                    rc = parent;
                    continue;
                }

                return;
            }
        }
    }
}

/// Return `Some` if the `NodeRef` is the only strong reference count
///
/// Note that this leaves the tree in a partially inconsistent state, since
/// the weak references to the node referenced by `r` will now point to
/// an unlinked node.
fn take_if_unique_strong<T>(r: &RefCell<Option<NodeRef<T>>>) -> Option<NodeRef<T>> {
    let mut r = r.borrow_mut();

    let has_single_ref = match *r {
        None => false,
        Some(ref rc) if Rc::strong_count(rc) > 1 => false,
        Some(_) => true,
    };

    if has_single_ref {
        r.take()
    } else {
        None
    }
}

// An iterator over the Node's children
pub struct Children<T> {
    next: Option<NodeRef<T>>,
    next_back: Option<NodeRef<T>>,
}

impl<T> Children<T> {
    fn new(next: Option<NodeRef<T>>, next_back: Option<NodeRef<T>>) -> Self {
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

// Implement Clone manually, since we want to disambiguate that we want
// to call clone on Rc and not T
impl<T> Clone for Children<T> {
    fn clone(&self) -> Children<T> {
        Children {
            next: self.next.clone(),
            next_back: self.next_back.clone(),
        }
    }
}

impl<T> Iterator for Children<T> {
    type Item = NodeRef<T>;

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

    type N = Node<()>;

    impl N {
        pub fn new() -> N {
            N {
                parent: None,
                first_child: RefCell::new(None),
                last_child: RefCell::new(None),
                next_sib: RefCell::new(None),
                prev_sib: RefCell::new(None),
                data: (),
            }
        }

        pub fn new_with_parent(parent: NodeWeakRef<()>) -> N {
            N {
                parent: Some(parent),
                first_child: RefCell::new(None),
                last_child: RefCell::new(None),
                next_sib: RefCell::new(None),
                prev_sib: RefCell::new(None),
                data: (),
            }
        }
    }

    #[test]
    fn node_is_its_own_ancestor() {
        let node = Rc::new(N::new());;
        assert!(Node::is_ancestor(node.clone(), node.clone()));
    }

    #[test]
    fn node_is_ancestor_of_child() {
        let node = Rc::new(N::new());;
        let child = Rc::new(N::new_with_parent(Rc::downgrade(&node)));

        node.add_child(&child);

        assert!(Node::is_ancestor(node.clone(), child.clone()));
        assert!(!Node::is_ancestor(child.clone(), node.clone()));
    }

    #[test]
    fn node_children_iterator() {
        let node = Rc::new(N::new());;
        let child = Rc::new(N::new_with_parent(Rc::downgrade(&node)));
        let second_child = Rc::new(N::new_with_parent(Rc::downgrade(&node)));

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
