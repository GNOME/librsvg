XML Parser in Rust
==================

**Status as of 2025/May/02: not implemented**

The purpose of this proposal is to replace libxml2 in librsvg with a
Rust-based XML parser.

Librsvg uses `libxml2 <https://gitlab.gnome.org/GNOME/libxml2>`_ to do
the initial XML parsing of an SVG document.  It does not let libxml2
build its own tree representation; instead, it uses the SAX2
"streaming" parser API and so librsvg builds a tree of its own with
tag names and attributes.

Pragmatically speaking, there is nothing wrong with using libxml2 in
the way that librsvg uses it:

* Libxml2 is fast.

* It is well-maintained, is fuzz-tested at scale, and is such a
  critical piece of infrastructure that people actually pay attention
  to it.

* It has built-in mitigations for common XML attacks like the
  "`billion laughs
  <https://en.wikipedia.org/wiki/Billion_laughs_attack>`".

* Librsvg is careful to turn off features like network access and
  external XML entities, which are a well-known source of
  attacks.

However, libxml2 has had many CVEs and security problems in the past.
It is the sort of infrastructure that should be replaced with
memory-safe code at some point.

Steps
-----

1. Separate the XML tree from the SVG element tree.

2. Change the XML tree to one that is produced by the new Rust-based
   XML parser.

The sections below explore each of these steps.

Separating the XML tree from the SVG element tree
-------------------------------------------------

Librsvg has a tree data structure, managed by the ``rctree`` crate,
where each node is a combination of XML data (element name for the
tag, and a list of attributes with their string values) and the parsed
SVG data (individual structs for ``Group``, ``Path``, etc., plus
parsed properties and element-specific attributes).

From ``document.rs``:

.. code-block:: rust

    pub struct Document {
        /// Tree of nodes; the root is guaranteed to be an `<svg>` element.
        tree: Node,
    
        // ...
    }


From ``node.rs``:

.. code-block:: rust

    pub type Node = rctree::Node<NodeData>;

    pub enum NodeData {
        Element(Box<Element>),
        Text(Box<Chars>),
    }

From ``xml/attributes.rs``:

.. code-block:: rust

    pub struct Attributes {
        attrs: Box<[(QualName, AttributeValue)]>,
        // ...
    }

From ``element.rs``:

.. code-block:: rust

    pub struct Element {
        element_name: QualName,
        attributes: Attributes,

        specified_values: SpecifiedValues,
        pub element_data: ElementData,
        // ... some fields omitted
    }

    pub enum ElementData {
        Circle(Box<Circle>),
        ClipPath(Box<ClipPath>),
        Ellipse(Box<Ellipse>),
        // ...
    }

Here, ``struct Element`` is a combination of XML string data
(``element_name``, ``attributes``), plus the result of parsing those
strings into SVG and CSS-specific information (``specified_values``,
``element_data``).

**Goal:** Basically, have ``Element`` *not* contain XML string data.
It may contain a pointer back to its corresponding XML node, and that
may even depend on what the crate that represents that XML tree lets
us do.

Things to consider
~~~~~~~~~~~~~~~~~~

* With the libxml2-based SAX2 parser, as soon as librsvg gets a "start
  element" event it will parse each value in the list of attributes.
  It will then use this information to construct an ``Element`` and
  then a ``Node``.  We may have to change this "build from the inside
  out" process to instead assume that an XML tree is available and
  full of strings, and later an SVG tree can be constructed from it.


Change the XML tree to one from a Rust-based parser
---------------------------------------------------

FIXME

* The code in ``css.rs`` which implements the ``selectors::Element``
  trait for nodes in the tree, needs O(1) access to a node's parent and
  to its next sibling.



Notes
-----

This used to be https://gitlab.gnome.org/GNOME/librsvg/-/issues/224
but it was mostly a wishlist item, instead of a specification document
like the present one.
