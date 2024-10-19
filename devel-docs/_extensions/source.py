"""Custom reStructuredText entities/helpers for referencing entries in the
librsvg source tree.

For any changes made in this module, please ensure
``devel-docs/devel_docs_mod_guide.rst`` is updated accordingly, if necessary.

For help/reference on modifying these, see:

- https://www.sphinx-doc.org/en/master/development/tutorials/extending_syntax.html
- https://docutils.sourceforge.io/docs/howto/rst-roles.html
- https://protips.readthedocs.io/link-roles.html
- https://www.sphinx-doc.org/en/master/extdev/utils.html#sphinx.util.docutils.SphinxRole
- https://www.sphinx-doc.org/en/master/extdev/utils.html#sphinx.util.docutils.ReferenceRole
- https://github.com/sphinx-doc/sphinx/blob/master/sphinx/roles.py
"""

from __future__ import annotations

from docutils.nodes import reference
from sphinx.util.docutils import ReferenceRole

BASE_URL = "https://gitlab.gnome.org/GNOME/librsvg/-/tree"


class SourceRole(ReferenceRole):
    """A custom Sphinx role for referencing entries in the librsvg source tree.
    """

    def run(self):
        ref, _, path = self.target.rpartition(":")

        if path.startswith("/"):
            msg = self.inliner.reporter.error(
                f"Invalid source tree entry path: {path!r}", line=self.lineno
            )
            prb = self.inliner.problematic(self.rawtext, self.rawtext, msg)

            return [prb], [msg]

        if ref:
            # 12 <= 9 characters + elipsis (3 periods);
            # 12, to allow `librsvg-X.YZ`.
            short_ref = f"{ref[:9]}..." if len(ref) > 12 else ref
        else:
            short_ref = ref = "main"

        node = reference(
            self.rawtext,
            (
                self.title if self.has_explicit_title
                else path or 'the source tree' if ref == "main"
                else f"{path or 'the source tree'} (@ {short_ref})"
            ),
            refuri=f"{BASE_URL}/{ref}/{path}",
            **self.options,
        )

        return [node], []


def setup(app):
    app.add_role("source", SourceRole())
