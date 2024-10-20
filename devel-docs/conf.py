# Configuration file for the Sphinx documentation builder.
#
# This file only contains a selection of the most common options. For a full
# list see the documentation:
# https://www.sphinx-doc.org/en/master/usage/configuration.html

# -- Path setup --------------------------------------------------------------

# If extensions (or modules to document with autodoc) are in another directory,
# add these directories to sys.path here. If the directory is relative to the
# documentation root, use os.path.abspath to make it absolute, like shown here.
import os
import sys
sys.path.insert(0, os.path.abspath('_extensions'))


# -- Project information -----------------------------------------------------

project = 'Development guide for librsvg'
copyright = '2022, Federico Mena Quintero'
author = 'Federico Mena Quintero'


# -- General configuration ---------------------------------------------------

# Add any Sphinx extension module names here, as strings. They can be
# extensions coming with Sphinx (named 'sphinx.ext.*') or your custom
# ones.
extensions = [
    # Used to shorten external links.
    # https://www.sphinx-doc.org/en/master/usage/extensions/extlinks.html
    "sphinx.ext.extlinks",
    # Used to link issues, merge requests, CVEs, etc.
    # https://github.com/sloria/sphinx-issues
    "sphinx_issues",
    # Used to reference entities in the internals documentation.
    # ./_extensions/internals.py
    "internals",
    # Used to reference entries in the source tree.
    # ./_extensions/source.py
    "source",
]

# Add any paths that contain templates here, relative to this directory.
templates_path = ['_templates']

# List of patterns, relative to source directory, that match files and
# directories to ignore when looking for source files.
# This pattern also affects html_static_path and html_extra_path.
exclude_patterns = ['_build', 'Thumbs.db', '.DS_Store']


# -- Options for HTML output -------------------------------------------------

# The theme to use for HTML and HTML Help pages.  See the documentation for
# a list of builtin themes.
#
html_theme = 'furo'

# Add any paths that contain custom static files (such as style sheets) here,
# relative to this directory. They are copied after the builtin static files,
# so a file named "default.css" will overwrite the builtin "default.css".
html_static_path = ['_static']


# Options for the linkcheck builder.  This is used by the ci/check_docs_links.sh script.
#
# https://www.sphinx-doc.org/en/master/usage/configuration.html#options-for-the-linkcheck-builder

linkcheck_ignore = [
    # These URLs fail for some reason, but work in the browser.
    r'https://crates.io/crates/.*',

    # Links with anchors for section names or line numbers fail.  But since we usually
    # use specific commit ids, they should be correct anyway.
    r'https://github.com/.*#.*',
    r'https://gitlab.gnome.org/.*#.*',
    r'https://gitlab.freedesktop.org/.*#.*',
]


# Options for the `sphinx.ext.extlinks` extension. See `extensions` above.

extlinks = {
    "rustsec": ("https://rustsec.org/advisories/RUSTSEC-%s", "RUSTSEC-%s"),
}
extlinks_detect_hardcoded_links = True


# Options for the `sphinx-issues` extension. See `extensions` above.

issues_default_group_project = "GNOME/librsvg"
issues_uri = "https://gitlab.gnome.org/{group}/{project}/-/issues/{issue}"
issues_prefix = "#"
issues_pr_uri = "https://gitlab.gnome.org/{group}/{project}/-/merge_requests/{pr}"
issues_pr_prefix = "!"
issues_commit_uri = "https://gitlab.gnome.org/{group}/{project}/-/commit/{commit}"
issues_commit_prefix = "@"
issues_user_uri = "https://gitlab.gnome.org/{user}"
issues_user_prefix = "@"
