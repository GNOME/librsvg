Security
========

Reporting security bugs
-----------------------

Please mail the maintainer at federico@gnome.org. You can use the GPG
public key from https://viruta.org/docs/fmq-gpg.asc to send encrypted
mail.

Librsvg releases with security fixes
------------------------------------

Librsvg releases have a version number like major.minor.micro.

Before version 2.55.x, librsvg's versioning scheme was such that a
release with an *even* minor number was considered a stable release
suitable for production use (e.g. 2.54.x), and an *odd* minor number
was a development release only.

Starting with 2.55.x, all minor numbers are considered stable.
Development and beta versions have a micro version starting at 90
(e.g. 2.55.90), per `GNOME's release versioning as of 2022/September
<https://discourse.gnome.org/t/even-odd-versioning-is-confusing-lets-stop-doing-it/10391>`_.

The following list is only for stable release streams.

2.56.3
~~~~~~

CVE-2023-38633 - `#996
<https://gitlab.gnome.org/GNOME/librsvg/-/issues/996>`_ - Arbitrary
file read when xinclude href has special characters.

2.55.3
~~~~~~

CVE-2023-38633 - `#996
<https://gitlab.gnome.org/GNOME/librsvg/-/issues/996>`_ - Arbitrary
file read when xinclude href has special characters.

2.54.7
~~~~~~

CVE-2023-38633 - `#996
<https://gitlab.gnome.org/GNOME/librsvg/-/issues/996>`_ - Arbitrary
file read when xinclude href has special characters.

2.52.11
~~~~~~~

CVE-2023-38633 - `#996
<https://gitlab.gnome.org/GNOME/librsvg/-/issues/996>`_ - Arbitrary
file read when xinclude href has special characters.

2.50.9
~~~~~~

CVE-2023-38633 - `#996
<https://gitlab.gnome.org/GNOME/librsvg/-/issues/996>`_ - Arbitrary
file read when xinclude href has special characters.

2.50.4
~~~~~~

RUSTSEC-2020-0146 - lifetime erasure in generic-array.

2.48.12
~~~~~~~

CVE-2023-38633 - `#996
<https://gitlab.gnome.org/GNOME/librsvg/-/issues/996>`_ - Arbitrary
file read when xinclude href has special characters.

2.48.10
~~~~~~~

CVE-2020-35905 - RUSTSEC-2020-0059 - data race in futures-util.

CVE-2020-35906 - RUSTSEC-2020-0060 - use-after-free in futures-task.

CVE-2021-25900 - RUSTSEC-2021-0003 - buffer overflow in smallvec.

RUSTSEC-2020-0146 - lifetime erasure in generic-array.

2.48.0
~~~~~~

CVE-2019-20446 - guard against exponential growth of CPU time from
malicious SVGs.

Releases older than 2.48.0 are not recommended
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

See notes below on :ref:`libcroco<libcroco>`.

2.46.7
~~~~~~

CVE-2023-38633 - `#996
<https://gitlab.gnome.org/GNOME/librsvg/-/issues/996>`_ - Arbitrary
file read when xinclude href has special characters.

See notes below on :ref:`libcroco<libcroco>`.

2.46.5
~~~~~~

RUSTSEC-2020-0146 - lifetime erasure in generic-array.

CVE-2021-25900 - RUSTSEC-2021-0003 - buffer overflow in smallvec.

See notes below on :ref:`libcroco<libcroco>`.

2.44.17
~~~~~~~

RUSTSEC-2020-0146 - lifetime erasure in generic-array.

CVE-2019-15554 - RUSTSEC-2019-0012 - memory corruption in smallvec.

CVE-2019-15551 - RUSTSEC-2019-0009 - double-free and use-after-free in
smallvec.

CVE-2021-25900 - RUSTSEC-2021-0003 - buffer overflow in smallvec.

See notes below on :ref:`libcroco<libcroco>`.

2.44.16
~~~~~~~

CVE-2019-20446 - guard against exponential growth of CPU time from
malicious SVGs.

See notes below on :ref:`libcroco<libcroco>`.

2.42.8
~~~~~~

CVE-2019-20446 - guard against exponential growth of CPU time from
malicious SVGs.

See notes below on :ref:`libcroco<libcroco>`.

2.42.9
~~~~~~

CVE-2018-20991 - RUSTSEC-2018-0003 - double-free in smallvec.

See notes below on :ref:`libcroco<libcroco>`.

2.40.21
~~~~~~~

CVE-2019-20446 - guard against exponential growth of CPU time from
malicious SVGs.

See notes below on :ref:`libcroco<libcroco>`.

2.40.18
~~~~~~~

CVE-2017-11464 - Fix division-by-zero in the Gaussian blur code.

See notes below on :ref:`libcroco<libcroco>`.

Earlier releases should be avoided and are not listed here.
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

.. _libcroco:

Important note on libcroco
~~~~~~~~~~~~~~~~~~~~~~~~~~

Note that librsvg 2.46.x and earlier use
`libcroco <https://gitlab.gnome.org/Archive/libcroco/>`__ for parsing
CSS, but that library is deprecated, unmaintained, and has open CVEs as
of May 2021.

If your application processes untrusted data, please avoid using librsvg
2.46.x or earlier. The first release of librsvg that does not use
libcroco is 2.48.0.

Librsvg’s dependencies
----------------------

Librsvg depends on the following libraries implemented in memory-unsafe
languages:

- **libxml2** - loading XML data.
- **cairo** - 2D rendering engine.
- **freetype2** - font renderer.
- **harfbuzz** - text shaping engine.
- **pango** - high-level text rendering.
- **fontconfig** - system fonts and rules for using them.

And of course, their recursive dependencies as well, such as
**glib/gio**.


Security considerations for the image-rs crate
----------------------------------------------

Librsvg uses the `image-rs <https://github.com/image-rs/image>`_ crate
for decoding raster images.  You may want to look at its dependencies
for specific codecs like the ``png`` or ``zune-jpeg`` crates.

Librsvg explicitly compiles ``image-rs`` with support for only the following formats:

* JPEG
* PNG
* GIF
* WEBP

The following formats are optional, and selected at compilation time:

* AVIF

See the "Compile-time options" section in :doc:`compiling` for details.


Security considerations for libxml2
-----------------------------------

Librsvg uses the following configuration for the SAX2 parser in libxml2:

-  ``XML_PARSE_NONET`` - forbid network access.
-  ``XML_PARSE_BIG_LINES`` - store big line numbers.

As a special case, librsvg enables ``replaceEntities`` in the
``_xmlParserCtxtPtr`` struct so that libxml2 will expand references only
to internal entities declared in the DTD subset. External entities are
disabled.

For example, the following document renders two rectangles that are
expanded from internal entities:

::

   <!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1 Basic//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11-basic.dtd" [
     <!ENTITY Rect1 "<rect x='15' y='10' width='20' height='30' fill='blue'/>">
     <!ENTITY Rect2 "<rect x='10' y='5' width='10' height='20' fill='green'/>">
   ]>
   <svg xmlns="http://www.w3.org/2000/svg" width="60" height="60">
     &Rect1;
     &Rect2;
   </svg>

However, an external entity like

::

     <!ENTITY foo SYSTEM "foo.xml">

will generate an XML parse error and the document will not be loaded.

Security considerations for Cairo
---------------------------------

Cairo versions before 1.17.0 are easy to crash if given coordinates
that fall outside the range of its 24.8 fixed-point numbers.  Please
make sure that you use librsvg with Cairo 1.17.0 or newer.

The first version of librsvg to require at least Cairo 1.17.0 is
librsvg 2.56.90 (development), or librsvg 2.57.0 (stable).

Security considerations for librsvg
-----------------------------------

**Built-in limits:** Librsvg has built-in limits for the following:

- Limit on the maximum number of loaded XML elements, set to 1,000,000
  (one million). SVG documents with more than this number of elements
  will fail to load. This is a mitigation for malicious documents that
  would otherwise consume large amounts of memory, for example by
  including a huge number of ``<g/>`` elements with no useful content.
  This is set in the file ``rsvg/src/limits.rs`` in the
  ``MAX_LOADED_ELEMENTS`` constant.

- Limit on the maximum number of referenced elements while rendering.
  The ``<use>`` element in SVG and others like ``<pattern>`` can
  reference other elements in the document. Malicious documents can
  cause an exponential number of references to be resolved, so librsvg
  places a limit of 500,000 references (half a million) to avoid
  unbounded consumption of CPU time. This is set in the file
  ``rsvg/src/limits.rs`` in the ``MAX_REFERENCED_ELEMENTS`` constant.

- Limit on the nesting level for XML Includes (``xi:include``), to
  avoid infinite recursion from an SVG file that includes itself.
  This is set in the file ``rsvg/src/limits.rs`` in the
  ``MAX_XINCLUDE_DEPTH`` constant.

Librsvg has no built-in limits on the total amount of memory or CPU time
consumed to process a document. Your application may want to place
limits on this, especially if it processes untrusted SVG documents.

**Processing external files:** Librsvg processes references to
external files by itself: XML XInclude, ``xlink:href`` attributes,
etc. Please see the section "`Security and locations of referenced
files
<https://gnome.pages.gitlab.gnome.org/librsvg/Rsvg-2.0/class.Handle.html#security-and-locations-of-referenced-files>`_"
in the reference documentation to see what criteria are used to accept
or reject a file based on its location. If your application has more
stringent requirements, it may need to sandbox its use of librsvg.

**SVG features:** Librsvg ignores animations, scripts, and events
declared in SVG documents. It always handles referenced images, similar
to SVG’s `static processing
mode <https://www.w3.org/TR/SVG2/conform.html#static-mode>`__.
