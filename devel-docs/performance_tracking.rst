Performance tracking
====================

This project is suitable for a few months of work, like an Outreachy
or Summer of Code internship.

As of 2022/Sep, there is no infrastructure to track librsvg's
performance over time.  At different times there are efforts to reduce
the memory consumption of certain parts of the code, or to make them
faster, but there is no monitoring of how the library is doing as its
code evolves.

Given a set of example files (which can change over time), it would be
nice to track the following measurements over time:

- Maximum heap usage during loading and during rendering.

- CPU time for loading; CPU time for rendering.

These would let us answer questions like, is librsvg using more or
less memory over time?  More or less CPU time?  Can we detect
regressions with this tool?

Getting the actual measurements is not terribly hard; just running
``/usr/bin/time rsvg-convert car.svg > /dev/null`` already produces a useful
big-picture view of things:

::
  $ /usr/bin/time rsvg-convert car.svg > /dev/null
  0.90user 0.07system 0:01.10elapsed 88%CPU (0avgtext+0avgdata 39460maxresident)k
  224inputs+0outputs (5major+6646minor)pagefaults 0swaps

The hard part is the logistics of accumulating the reports over time,
and graphing them.  This is your project!

Goals
-----

- Given a librsvg commit, extract performance metrics for a corpus of
  test files.

- Store those metrics somewhere, preferably in gnome.org.

- Plot the metrics over time.  The plot should make it easy to jump to
  a particular commit, so that we can do, "memory usage increased
  considerably at this point; which commit is responsible?".

Questions to be researched by the intern
----------------------------------------

- Gathering a set of interesting documents to keep around for regular
  testing; `Featured Pictures from Wikimedia Commons
  <https://commons.wikimedia.org/wiki/Category:Featured_pictures_on_Wikimedia_Commons_-_vector>`_
  is a good source.  Ask the maintainer for more!

- Is CPU time an accurate measure, even with busy CI runners?  Or does
  this need a "quiet" machine?  Do small files that get rendered very
  quickly need to be measured by averaging several runs?

- Maintaining a history of measurements, probably keyed by commit id
  and document.  Where do we keep that data?  In a git repo?  Or in a
  web service - can we host it at gnome.org?  In the maintainer's
  laptop?  "Append something to a perf log file somewhere and commit"
  -> "Run a plotting program in $somewhere's CI" is probably the
  Minimum Viable thing.

- Notifying the performance infrastructure about a new commit to test.
  Can we do this from the CI?  Maybe with `downstream pipelines
  <https://docs.gitlab.com/ee/ci/pipelines/downstream_pipelines.html>`_?

- `GitLab's reports of custom metrics
  <https://docs.gitlab.com/ee/ci/testing/metrics_reports.html>`_ look
  cool and are displayed conveniently as part of a merge request's
  page, but are only present in the Premium edition, not in GNOME's
  GitLab instance.  Is any of that worth exploring?  Is the suggested
  `OpenMetrics format <https://openmetrics.io/>`_ good as-is, or is it
  overkill?

- Instant gratification: can we generate a new plot and publish it as
  part of a CI pipeline's artifacts?  Sparklines of performance
  metrics over time, to maximize the bling/size ratio?

Inspiration, but beware of overkill
-----------------------------------

- `Are We Fast Yet <https://arewefastyet.com/>`_, Firefox's performance metrics.

- `WebKit Performance Dashboard <https://perf.webkit.org/>`_.


Sample documents
----------------

Interesting types of SVG documents to put into the performance tracker:

- Lots of objects.

- Lots of filters.

- Lots of text.

- Wikimedia Commons has good examples: `featured pictures
  <https://commons.wikimedia.org/wiki/
  Category:Featured_pictures_on_Wikimedia_Commons_-_vector>`_, `SVG by
  subject
  <https://commons.wikimedia.org/wiki/Category:SVG_by_subject>`_.
