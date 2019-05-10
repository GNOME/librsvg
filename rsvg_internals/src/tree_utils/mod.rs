use std::cell::RefCell;
use std::ops::Deref;
use std::rc::{Rc, Weak};

/// A strong reference to a node
///
/// These strong references can be compared with `==` as they
/// implement `PartialEq` and `Eq`; comparison means whether they
/// point to the same node struct.
pub struct NodeRef<T>(pub Rc<Node<T>>);

pub type NodeWeakRef<T> = Weak<Node<T>>;

impl<T> Clone for NodeRef<T> {
    fn clone(&self) -> NodeRef<T> {
        NodeRef(self.0.clone())
    }
}

impl<T> Deref for NodeRef<T> {
    type Target = Node<T>;

    #[inline]
    fn deref(&self) -> &Node<T> {
        &*self.0
    }
}

impl<T> Eq for NodeRef<T> {}
impl<T> PartialEq for NodeRef<T> {
    #[inline]
    fn eq(&self, other: &NodeRef<T>) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

pub struct Node<T> {
    parent: Option<NodeWeakRef<T>>,
    first_child: RefCell<Option<NodeRef<T>>>,
    last_child: RefCell<Option<NodeWeakRef<T>>>,
    next_sibling: RefCell<Option<NodeRef<T>>>,
    previous_sibling: RefCell<Option<NodeWeakRef<T>>>,
    data: T,
}

impl<T> Node<T> {
    pub fn new(data: T, parent: Option<&NodeRef<T>>) -> Node<T> {
        Node {
            parent: parent.map(|n| Rc::downgrade(&n.0)),
            first_child: RefCell::new(None),
            last_child: RefCell::new(None),
            next_sibling: RefCell::new(None),
            previous_sibling: RefCell::new(None),
            data,
        }
    }

    pub fn parent(&self) -> Option<NodeRef<T>> {
        match self.parent {
            None => None,
            Some(ref weak_node) => Some(NodeRef(weak_node.upgrade().unwrap())),
        }
    }

    pub fn first_child(&self) -> Option<NodeRef<T>> {
        match *self.first_child.borrow() {
            None => None,
            Some(ref node) => Some(node.clone()),
        }
    }

    pub fn last_child(&self) -> Option<NodeRef<T>> {
        match *self.last_child.borrow() {
            None => None,
            Some(ref weak_node) => Some(NodeRef(weak_node.upgrade().unwrap())),
        }
    }

    pub fn next_sibling(&self) -> Option<NodeRef<T>> {
        match *self.next_sibling.borrow() {
            None => None,
            Some(ref node) => Some(node.clone()),
        }
    }

    pub fn previous_sibling(&self) -> Option<NodeRef<T>> {
        match *self.previous_sibling.borrow() {
            None => None,
            Some(ref weak_node) => Some(NodeRef(weak_node.upgrade().unwrap())),
        }
    }

    pub fn is_ancestor(ancestor: NodeRef<T>, descendant: NodeRef<T>) -> bool {
        let mut desc = Some(descendant.clone());

        while let Some(ref d) = desc.clone() {
            if ancestor == *d {
                return true;
            }

            desc = d.parent();
        }

        false
    }

    pub fn append(&self, child: &NodeRef<T>) {
        assert!(child.next_sibling.borrow().is_none());
        assert!(child.previous_sibling.borrow().is_none());

        if let Some(last_child_weak) = self.last_child.replace(Some(Rc::downgrade(&child.0))) {
            if let Some(last_child) = last_child_weak.upgrade() {
                child.previous_sibling.replace(Some(last_child_weak));
                last_child.next_sibling.replace(Some(child.clone()));
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
            .and_then(|child_weak| child_weak.upgrade())
            .map(NodeRef);
        Children::new(self.first_child.borrow().clone(), last_child)
    }

    pub fn has_children(&self) -> bool {
        self.first_child.borrow().is_some()
    }

    pub fn borrow(&self) -> &T {
        &self.data
    }

    pub fn borrow_mut(&mut self) -> &mut T {
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

        if let Some(rc) = take_if_unique_strong(&self.next_sibling) {
            non_recursive_drop_unique_rc(rc, &mut stack);
        }

        fn non_recursive_drop_unique_rc<T>(mut rc: NodeRef<T>, stack: &mut Vec<NodeRef<T>>) {
            loop {
                if let Some(child) = take_if_unique_strong(&rc.first_child) {
                    stack.push(rc);
                    rc = child;
                    continue;
                }

                if let Some(sibling) = take_if_unique_strong(&rc.next_sibling) {
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
        Some(ref rc) if Rc::strong_count(&rc.0) > 1 => false,
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
                    .next_sibling
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
            self.next = next.next_sibling.borrow().clone();
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
                .previous_sibling
                .borrow()
                .as_ref()
                .and_then(|sib_weak| sib_weak.upgrade())
                .map(NodeRef);
            Some(next_back)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;

    type N = Node<()>;

    #[test]
    fn node_is_its_own_ancestor() {
        let node = NodeRef(Rc::new(N::new((), None)));
        assert!(Node::is_ancestor(node.clone(), node.clone()));
    }

    #[test]
    fn node_is_ancestor_of_child() {
        let node = NodeRef(Rc::new(N::new((), None)));
        let child = NodeRef(Rc::new(N::new((), Some(&node))));

        node.append(&child);

        assert!(Node::is_ancestor(node.clone(), child.clone()));
        assert!(!Node::is_ancestor(child.clone(), node.clone()));
    }

    #[test]
    fn node_children_iterator() {
        let node = NodeRef(Rc::new(N::new((), None)));
        let child = NodeRef(Rc::new(N::new((), Some(&node))));
        let second_child = NodeRef(Rc::new(N::new((), Some(&node))));

        node.append(&child);
        node.append(&second_child);

        let mut children = node.children();

        let c = children.next();
        assert!(c.is_some());
        let c = c.unwrap();
        assert!(c == child);

        let c = children.next_back();
        assert!(c.is_some());
        let c = c.unwrap();
        assert!(c == second_child);

        assert!(children.next().is_none());
        assert!(children.next_back().is_none());
    }
}
