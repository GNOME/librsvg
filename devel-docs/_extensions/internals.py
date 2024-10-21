"""Custom reStructuredText entities/helpers for referencing entities in the
librsvg internals documentation.

For any changes made in this module, please ensure
``devel-docs/devel_docs_mod_guide.rst`` is updated accordingly, if necessary.

For help/reference on modifying these, see:

- https://www.sphinx-doc.org/en/master/development/tutorials/extending_syntax.html
- https://docutils.sourceforge.io/docs/howto/rst-roles.html
- https://protips.readthedocs.io/link-roles.html
- https://www.sphinx-doc.org/en/master/development/tutorials/adding_domain.html
- https://www.sphinx-doc.org/en/master/extdev/utils.html#sphinx.util.docutils.SphinxRole
- https://www.sphinx-doc.org/en/master/extdev/utils.html#sphinx.util.docutils.ReferenceRole
- https://github.com/sphinx-doc/sphinx/blob/master/sphinx/roles.py
"""

from __future__ import annotations

from docutils.nodes import literal, reference
from sphinx.domains import Domain
from sphinx.util.docutils import ReferenceRole

BASE_URL = "https://gnome.pages.gitlab.gnome.org/librsvg/internals"


class CrateRole(ReferenceRole):
    """A custom Sphinx role for referencing crates in the librsvg internals
    documentation.
    """

    def run(self):
        node = reference(
            self.rawtext,
            refuri=f"{BASE_URL}/{self.target}/index.html",
            **self.options,
        )
        node += literal(self.rawtext, self.title)

        return [node], []


class ModuleRole(ReferenceRole):
    """A custom Sphinx role for referencing modules in the librsvg internals
    documentation.
    """

    def run(self):
        components = self.target.split("::")

        try:
            *parents, module = components
            if not parents:
                raise ValueError
        except ValueError:
            msg = self.inliner.reporter.error(
                f"Invalid module target: {self.target!r}", line=self.lineno
            )
            prb = self.inliner.problematic(self.rawtext, self.rawtext, msg)

            return [prb], [msg]

        node = reference(
            self.rawtext,
            refuri=f"{BASE_URL}/{'/'.join(parents)}/{module}/index.html",
            **self.options,
        )
        node += literal(
            self.rawtext, self.title if self.has_explicit_title else module
        )

        return [node], []


class TopLevelRole(ReferenceRole):
    """A custom Sphinx role for referencing top-level entities in the librsvg
    internals documentation.
    """

    def __init__(self, target_kind: str, *, target_is_callable: bool = False):
        super().__init__()
        self.target_kind = target_kind
        self.target_is_callable = target_is_callable

    def run(self):
        *parents, item = self.target.split("::")

        if not parents:
            msg = self.inliner.reporter.error(
                f"Invalid {self.target_kind} target: {self.target!r}",
                line=self.lineno,
            )
            prb = self.inliner.problematic(self.rawtext, self.rawtext, msg)

            return [prb], [msg]

        node = reference(
            self.rawtext,
            refuri=(
                f"{BASE_URL}/{'/'.join(parents)}/{self.target_kind}"
                f".{item}.html"
            ),
            **self.options,
        )
        node += literal(
            self.rawtext,
            (
                self.title if self.has_explicit_title
                else f"{item}()" if self.target_is_callable
                else item
            ),
        )

        return [node], []


class MemberRole(ReferenceRole):
    """A custom Sphinx role for referencing members of structs, enums, etc
    in the librsvg internals documentation.
    """

    def __init__(
        self,
        target_kind: str,
        parent_tag: str,
        member_tag: str,
        *,
        target_is_callable: bool = False,
    ):
        super().__init__()
        self.target_kind = target_kind
        self.parent_tag = parent_tag
        self.member_tag = member_tag
        self.target_is_callable = target_is_callable

    def run(self):
        show_parent = not self.target.startswith("~")
        target = self.target if show_parent else self.target[1:]
        components = target.split("::")

        try:
            *parents, parent, member = components
            if not parents:
                raise ValueError
        except ValueError:
            msg = self.inliner.reporter.error(
                f"Invalid {self.target_kind} target: {target!r}",
                line=self.lineno,
            )
            prb = self.inliner.problematic(self.rawtext, self.rawtext, msg)

            return [prb], [msg]

        node = reference(
            self.rawtext,
            refuri=(
                f"{BASE_URL}/{'/'.join(parents)}/{self.parent_tag}"
                f".{parent}.html#{self.member_tag}.{member}"
            ),
            **self.options,
        )

        if not self.has_explicit_title:
            title = f"{parent}::{member}" if show_parent else member

        node += literal(
            self.rawtext,
            (
                self.title if self.has_explicit_title
                else f"{title}()" if self.target_is_callable
                else title
            ),
        )

        return [node], []


class InternalsDomain(Domain):
    """A custom Sphinx domain for referencing the librsvg internals docs."""

    name = "internals"
    label = "Librsvg Internals Docs"
    roles = {
        "crate": CrateRole(),
        "module": ModuleRole(),

        # Top-level entities
        "struct": TopLevelRole("struct"),
        "enum": TopLevelRole("enum"),
        "trait": TopLevelRole("trait"),
        "type": TopLevelRole("type"),
        "fn": TopLevelRole("fn", target_is_callable=True),
        "macro": TopLevelRole("macro"),
        "constant": TopLevelRole("constant"),
        "static": TopLevelRole("static"),

        # Member entities
        "struct-field": MemberRole("struct field", "struct", "structfield"),
        "struct-method": MemberRole(
            "struct method", "struct", "method", target_is_callable=True
        ),
        "enum-variant": MemberRole("enum variant", "enum", "variant"),
        "trait-method": MemberRole(
            "provided trait method", "trait", "method", target_is_callable=True
        ),
        "trait-tymethod": MemberRole(
            "required trait method",
            "trait",
            "tymethod",
            target_is_callable=True,
        ),
    }


def setup(app):
    app.add_domain(InternalsDomain)
