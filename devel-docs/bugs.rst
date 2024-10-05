Reporting bugs
==============

Please report bugs at https://gitlab.gnome.org/GNOME/librsvg/-/issues

If you want to report a rendering bug, or a missing SVG feature,
please provide an example SVG file as an attachment to your bug
report.  It really helps if you can minimize the SVG to only the
elements required to reproduce the bug or see the missing feature, but
it is not absolutely required.  **Please be careful** of publishing
SVG images that you don't want other people to see, or images whose
copyright does not allow redistribution; the bug tracker is a public
resource and attachments are visible to everyone.

Feature requests
----------------

Librsvg aims to be a small and relatively simple SVG rendering
library.  Currently we do not plan to support scripting, animation, or
interactive features like mouse events on SVG elements.

However, we *do* aim to provide good support for SVG's graphical
features.  You can request new features by filing a bug report as
noted above.

It is especially helpful if you file a feature request along with a
sample SVG file that requires the feature. For example, a file that
uses an SVG element or CSS property that librsvg does not yet support.


Obtaining debug logs
--------------------

Librsvg can be asked to output debug logs.  Set the ``RSVG_LOG``
environment variable, and then librsvg will print some 
information to stdout:

.. code-block::

   $ RSVG_LOG=1 some-program-that-uses-librsvg
   ... debug output goes here ...

As of librsvg 2.43.5, there are no options you can set in the
``RSVG_LOG`` variable; the library just checks whether that environment
variable is present or not.

Security bugs
-------------

For especially sensitive bugs, please see :doc:`security`.
