Observability
=============

Librsvg supports basic, mostly ad-hoc logging with an ``RSVG_LOG=1``
environment variable. This has not been very effective in letting me,
the maintainer, know what went wrong when someone reports a bug about
librsvg doing the wrong thing in an application. Part of it is because
the code could be more thorough about logging (e.g. log at all error
points), but also part of it is that there is no logging about what API
calls are made into the library. On each bug reported on behalf of a
particular application, my thought process goes something like this:

-  What was the app doing?

-  Can I obtain the problematic SVG?

-  Does the bug reporter even know what the problematic SVG was?

-  Was the app rendering with direct calls to the librsvg API?

-  Or was it using the gdk-pixbuf loader, and thus has very little
   control of how librsvg is used?

-  If non-pixbuf, what was the Cairo state when librsvg was called?

-  What sort of interesting API calls are being used? Stylesheet
   injection? Re-rendering single elements with different styles?

And every time, I must ask the bug reporter for information related to
that, or to point me to the relevant source code where they were using
librsvg… which is not terribly useful, since building their code and
reproducing the bug with it is A Yak That Should Not Have To Be Shaved.

Desiderata
----------

Know exactly what an application did with librsvg:

-  All API calls and their parameters.

-  State of the Cairo context at entry.

-  “What SVG?” - be careful and explicit about exfiltrating SVG data to
   the logs.

-  Basic platform stuff? Is the platform triple enough? Distro ID?

-  Versions of dependencies.

-  Version of librsvg itself.

Internals of the library:

-  Regular debug tracing. We may have options to enable/disable tracing
   domains: parsing, cascading, referencing elements, temporary surfaces
   during filtering, render tree, etc.

-  Log all points where an error is detected/generated, even if it will
   be discarded later (e.g. invalid CSS values are silently ignored, per
   the spec).

Enabling logging
----------------

It may be useful to be able to enable logging in various ways:

-  Programmatically, for when one has control of the source code of the
   problematic application. Enable logging at the problem spot, for the
   SVG you know that exhibits the problem, and be done with it. This can
   probably be at the individual ``RsvgHandle`` level, not globally. For
   global logging within a single process, see the next point.

-  For a single process which one can easily launch via the command
   line; e.g. with an environment variable. This works well for
   non-sandboxed applications. Something like
   ``RSVG_LOG_CONFIG=my_log_config.toml``.

-  With a configuration file, a la ``~/.config/librsvg.toml``. Many
   programs use librsvg and you don’t want logs for all of them; allow
   the configuration file to specify a process name, or maybe other ways
   of determining when to log. For session programs like gnome-shell,
   you can’t easily set an environment variable to enable logging -
   hence, a configuration file that only turns on logging from the
   gnome-shell process.

All of the above should be well documented, and then we can deprecate
``RSVG_LOG``.

Which SVG caused a crash?
-------------------------

Every once in a while, a bug report comes in like “$application crashed
in librsvg”. The application renders many SVGs, often indirectly via
gdk-pixbuf, and it is hard to know exactly which SVG caused the problem.
Think of gnome-shell or gnome-software.

For applications that call librsvg directly, if they pass the filename
or a GFile then it is not hard to find out the source SVG.

But for those that feed bytes into librsvg, including those that use it
indirectly via gdk-pixbuf, librsvg has no knowledge of the filename. We
need to use the base_uri then, or see if the pixbuf loader can be
modified to propagate this information (is it even available from the
GdkPixbufLoader machinery?).

If all else fails, we can have an exfiltration mechanism. How can we
avoid logging *all* the SVG data that gnome-shell renders, for example?
Configure the logger to skip the first N SVGs, and hope that the order
is deterministic? We can’t really “log only if there is a crash during
rendering”.

Log only the checksums of SVGs or data lengths, and use that to find
which SVG caused the crash? I.e. have the user use a two-step process to
find a crash: get a log (written synchronously) of all SVG
checksums/lengths, and then reconfigure the logger to only exfiltrate
the last one that got logged - presumably that one caused the crash.

Which dynamically-created SVG caused a problem?
-----------------------------------------------

Consider a bug like :issue:`GNOME/gnome-shell#5415` where an
application dynamically generates an SVG and feeds it to librsvg. That
bug was not a crash; it was about incorrect values returned from an
librsvg API function. For those cases it may be useful to be able to
exfiltrate an SVG and its stylesheets only if it matches a user-provided
substring.

Global configuration
--------------------

``$(XDG_CONFIG_HOME)/librsvg.toml`` - for startup-like processes like
gnome-shell, for which it is hard to set an environment variable:

Per-process configuration
-------------------------

``RSVG_LOG_CONFIG=my_log_config.toml my_process``

Programmatic API
----------------

FIXME

Configuration format
--------------------

.. code:: toml

   [logging]
   enabled = true
   process = "gnome-shell"              # mandatory for global config - don't want to log all processes - warn to g_log if key is not set
   output = "/home/username/rsvg.log"   # if missing, log to g_log only - or use a output_to_g_log=true instead?

API logging
-----------

Log cr state at entry, surface type, starting transform.

Log name/base_uri of rendered document.

Can we know if it is a gresource? Or a byte buffer? Did it come from
gdk-pixbuf?

Implementation
--------------

There is currently the start of a :internals:struct:`rsvg::session::Session`
type woven throughout the source code, with the idea of it being the
thing that records logging events, it may be better to plug into the
``tracing`` ecosystem:

https://crates.io/crates/tracing

Initial ideas:

* See the "In libraries" section in ``tracing``'s README; it shows how
  to create spans for API calls.

* How would we capture from gnome-shell?  `tracing-journald
  <https://tracing-rs.netlify.app/tracing_journald/index.html>`_?
  Or would things be easier for casual users if we logged to a file?

* Maybe later, have a ``tracing-sysprof`` crate to send the events to
  `sysprof <https://gitlab.gnome.org/GNOME/sysprof/-/tree/master/src>`_?

Log contents
------------

/home/username/rsvg.log - json doesn’t have comments; put one of these
in a string somehow:

::

     ******************************************************************************
     * This log file exists because you enabled logging in ~/.config/librsvg.toml *
     * for the "gnome-shell" process.                                             *
     *                                                                            *
     * If you want to disable this kind of log, please turn it off in that file   *
     * or delete that file entirely.                                              *
     ******************************************************************************

     ******************************************************************************
     * This log file exists because you enabled logging with                      *
     * RSVG_LOG_CONFIG=config.toml for the "single-process-name" process.         *
     *                                                                            *
     * If you want to disable this kind of log, FIXME                             */
     ******************************************************************************

To-do list
----------

- Audit code for GIO errors; log there.

- Audit code for Cairo calls that yield errors; log there.

- Log the entire ancestry of the element that caused the error? Is
  that an insta-reproducer?
