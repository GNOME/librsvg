Fuzzing individual parsers
==========================

Status: not implemented.

This is a design document for the tasks identified in :issue:`1018`,
on how to improve librsvg's fuzzing infrastructure.

Status of fuzzing as of 2026/Sep/29
-----------------------------------

`Librsvg is in oss-fuzz
<https://github.com/google/oss-fuzz/tree/master/projects/librsvg>`__
and gets fuzzed continuously on Google's cloud.  We have gotten
interesting bug reports from there.

The only fuzz target available in librsvg's source tree is in
``fuzz/fuzz_targets/render_document.rs``.  This simply tries to parse
the data from the fuzzer as an SVG, and tries to render it.

While this is the minimum amount of fuzzing infrastructure we can
have, and while in theory it should allow for reaching a lot of the
code, in reality the fuzzer takes a lot of time to reach new code
paths, due to the extent of the code that is reachable from this
"parse anything" approach.

The latest update to the fuzzing machinery is from 2026/Sep/20, to
disable parsing of raster images when fuzzing.  The fuzzer would spend
a lot of time trying to generate ``data:`` URIs that can be decoded as
images, and while it did find problems in the ``image-rs`` crate,
those bugs are better done by fuzzing that crate and its codecs.

The `coverage report for oss-fuzz
<https://storage.googleapis.com/oss-fuzz-coverage/librsvg/reports/20260928/linux/src/librsvg/rsvg/src/report.html>`__
is a good place to look for places that the fuzzer has not yet
reached.

Pending: implement fuzz targets for mini-parsers
------------------------------------------------

The various mini-parsers inside librsvg are good candidates for individual fuzz targets:

* Parser for path data in ``path_parser.rs``.

* Parser for filter expressions in ``filter_func.rs``.  See `this comment <https://gitlab.gnome.org/GNOME/librsvg/-/work_items/1065#note_2060499>`__ for an example fuzz target.

* Each individual CSS property has a parser; look for properties in
  ``property_defs.rs`` and ``font_props.rs``.

* The things with ``impl Parse`` in ``parsers.rs``.  Note that these
  are the building blocks for some CSS properties.

Many (all?) of the ``impl Parse`` throughout the code make use of the
tokenizer from the ``cssparser`` crate.

None of the parsers is accessible from the public API.  The comment
linked above for filter expressions indicates how to expose these
parsing functions only when ``#[cfg(fuzzing)]`` is enabled, so that
code in the ``fuzz/fuzz_targets`` directory can use it.

Instead of exposing the individual ``impl Parse``, we can probably
have something like this:

.. code-block:: rust

   #[cfg(fuzzing)]
   mod fuzz_targets {
       use crate::filter_func::FilterFunc;  // this remains private
   
       pub fn fuzz_filter_function(data: &[u8]) {
           // actual code per #1065
       }
   }

Then, the fuzz target can just call ``fuzz_targets::fuzz_filter_function()``.
