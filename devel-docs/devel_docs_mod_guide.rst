:orphan:

Modifying the development guide
===============================

The following are guidelines and tips for modifying this guide.

Extra reStructuredText roles
----------------------------

Aside, the `roles provided out-of-the box
<https://www.sphinx-doc.org/en/master/usage/restructuredtext/roles.html>`_,
some 3rd-party and custom roles may also be used.

Referencing issues, commits, users, etc
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

All roles provided by the `sphinx-issues
<https://github.com/sloria/sphinx-issues>`_ extension may be used as
described on its page but note that the repository-related and user roles
are restricted to the ``gitlab.gnome.org`` GitLab instance e.g:

- ``:user:`federico``` -> :user:`federico`,
- ``:issue:`GNOME/gnome-shell#5415``` -> :issue:`GNOME/gnome-shell#5415`,

and the default repository for the repository-related roles is
`GNOME/librsvg <https://gitlab.gnome.org/GNOME/librsvg>`_ e.g:

- ``:issue:`1``` -> :issue:`1`,
- ``:commit:`550ba0c83939dfd0e829528dc8175639ad92dd83```
  -> :commit:`550ba0c83939dfd0e829528dc8175639ad92dd83`.

Referencing the source tree
~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. rst:role:: source

   References entries in the librsvg source tree.

   The target should be a valid path to an entry in the source tree
   relative to the source tree root, which may be prepended with a branch
   name, tag name or commit hash to reference the source tree at that
   branch, tag or commit.

   In other words, the target is of the form ``[<ref>:][<path>]``.
   ``<ref>:`` may be omitted to reference the default branch (``main``) and
   ``<path>`` may be omitted to reference the source tree root.

   For example:

   - ``:source:`rsvg/src/``` -> :source:`rsvg/src/`
   - ``:source:`Maintainers <README.md#maintainers>```
     -> :source:`Maintainers <README.md#maintainers>`
   - ``:source:`550ba0c83939dfd0e829528dc8175639ad92dd83:rsvg/src/```
     -> :source:`550ba0c83939dfd0e829528dc8175639ad92dd83:rsvg/src/`
   - ``:source:`2.59's README <2.59.0:README.md>```
     -> :source:`2.59's README <2.59.0:README.md>`
   - ``:source:`2.59.1:``` -> :source:`2.59.1:`

To add to or modify these roles see
:source:`devel-docs/_extensions/source.py`.

Referencing the internals documentation
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The following reference entities in the library's internals documentation:

.. rst:role:: internals:crate

   This references a crate.

   The target should be the name of the crate e.g
   ``:internals:crate:`librsvg_c``` -> :internals:crate:`librsvg_c`.

.. rst:role:: internals:module

   This references a module.

   The target should be a rust-style reference for the module e.g:

   - ``:internals:module:`rsvg::api``` -> :internals:module:`rsvg::api`
   - ``:internals:module:`rsvg::xml::xml2```
     -> :internals:module:`rsvg::xml::xml2`
   - ``:internals:module:`librsvg_c::handle <librsvg_c::handle>```
     -> :internals:module:`librsvg_c::handle <librsvg_c::handle>`

.. rst:role:: internals:struct
.. rst:role:: internals:enum
.. rst:role:: internals:trait
.. rst:role:: internals:type
.. rst:role:: internals:fn
.. rst:role:: internals:macro
.. rst:role:: internals:constant
.. rst:role:: internals:static

   These reference top-level entities.

   The target should be the rust-style fully-qualified reference for an
   entity e.g:

   - ``:internals:enum:`rsvg::RenderingError```
     -> :internals:enum:`rsvg::RenderingError`
   - ``:internals:struct:`librsvg_c::handle::RsvgHandle```
     -> :internals:struct:`librsvg_c::handle::RsvgHandle`
   - ``:internals:fn:`rsvg::drawing_ctx::draw_tree```
     -> :internals:fn:`rsvg::drawing_ctx::draw_tree`
   - ``:internals:constant:`rsvg::xml::xml2::XML_SAX2_MAGIC```
     -> :internals:constant:`rsvg::xml::xml2::XML_SAX2_MAGIC`

.. rst:role:: internals:struct-field
.. rst:role:: internals:struct-method
.. rst:role:: internals:enum-variant
.. rst:role:: internals:trait-method
.. rst:role:: internals:trait-tymethod

   These reference members of structs, enums, etc.

   The target should be the rust-style **fully-qualified** reference for a
   member entity. This normally renders as ``<parent>::<member>`` but the
   reference target may be prepended by a ``~`` (tilde) to render as just
   ``<member>``.
  
   For example:

   - ``:internals:struct-field:`rsvg::Length::unit```
     -> :internals:struct-field:`rsvg::Length::unit`
   - ``:internals:struct-method:`rsvg::element::Element::new```
     -> :internals:struct-method:`rsvg::element::Element::new`
   - ``:internals:struct-method:`~rsvg::element::Element::new```
     -> :internals:struct-method:`~rsvg::element::Element::new`
   - ``:internals:enum-variant:`rsvg::RenderingError::InvalidId```
     -> :internals:enum-variant:`rsvg::RenderingError::InvalidId`

   .. note::

      :rst:role:`internals:trait-method` references a **provided** trait
      method i.e a trait method that has a default implementation, such as
      :internals:trait-method:`rsvg::element::ElementTrait::draw`;
      while :rst:role:`internals:trait-tymethod` references a **required**
      trait method i.e a trait method that only has a prototype, such as
      :internals:trait-tymethod:`rsvg::length::Normalize::normalize`.

      To reference a struct's implementation of a trait's method, use
      :rst:role:`internals:struct-method`.

To add to or modify these roles see
:source:`devel-docs/_extensions/internals.py`.

Referencing RUSTSEC advisories
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. rst:role:: rustsec

   References the official page of a RUSTSEC advisory.

   The target should be the ID of the advisory e.g
   ``:rustsec:`2020-0146``` -> :rustsec:`2020-0146`.
