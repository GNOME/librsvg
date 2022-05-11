# Observability

Librsvg supports basic, mostly ad-hoc logging with an `RSVG_LOG=1`
environment variable.  This has not been very effective in letting me,
the maintainer, know what went wrong when someone reports a bug about
librsvg doing the wrong thing in an application.  Part of it is
because the code could be more thorough about logging (e.g. log at all
error points), but also part of it is that there is no logging about
what API calls are made into the library.  On each bug reported on
behalf of a particular application, my thought process goes something
like this:

* What was the app doing?

* Can I obtain the problematic SVG?

* Does the bug reporter even know what the problematic SVG was?

* Was the app rendering with direct calls to the librsvg API?

* Or was it using the gdk-pixbuf loader, and thus has very little control of how librsvg is used?

* If non-pixbuf, what was the Cairo state when librsvg was called?

* What sort of interesting API calls are being used?  Stylesheet
  injection?  Re-rendering single elements with different styles?

And every time, I must ask the bug reporter for information related to
that, or to point me to the relevant source code where they were using
librsvg... which is not terribly useful, since building their code and
reproducing the bug with it is A Yak That Should Not Have To Be
Shaved.

## Desiderata

Know exactly what an application did with librsvg:

* All API calls and their parameters.

* State of the Cairo context at entry.

* "What SVG?" - be careful and explicit about exfiltrating SVG data to the logs.

Internals of the library:

* Regular debug tracing.  We may have options to enable/disable
  tracing domains: parsing, cascading, referencing elements, temporary
  surfaces during filtering, render tree, etc.

* Log all points where an error is detected/generated, even if it will
  be discarded later (e.g. invalid CSS values are silently ignored,
  per the spec).

## Stuff to log



Log cr state at entry.

Log name/base_uri of rendered document.

Can we know if it is a gresource?  Or a byte buffer?  Did it come from gdk-pixbuf?

## Invocation

RSVG_LOG=1 is easy for specific processes or rsvg-convert

Login processes like gnome-shell need a config file.  ~/.config/librsvg.toml:

  [logging]
  enabled=true                     # make this the default if the file exists?
  process=gnome-shell              # mandatory - don't want to log all processes - warn to g_log if key is not set
  output=/home/federico/rsvg.log   # if missing, log to g_log only

/home/federico/rsvg.log - json doesn't have comments; put this in a string somehow:
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

** To-do list [0/1]

- [ ] Audit code for GIO errors; log there.

- [ ] Audit code for Cairo calls that yield errors; log there.

- [ ] Log the entire ancestry of the element that caused the error?  Is that an insta-reproducer?

** Ideas 

*** Log API calls?

Is this useful?  Not all the entry points; most cases are new_from_whatever() / render().

Better, log the filename, or the base_uri for a stream, or optionally exfiltrate the SVG in case of a resource or raw data.

*** What to log

Entry point at rendering: state of the Cairo context, surface type, starting transform, etc.

Versions of dependencies - pango, cairo, etc.  Distro name / Windows / MacOS?

*** Limit to a process

For global configuration (see below), put the process name in the configuration file.

For single-process config, use RSVG_LOG_CONFIG=filename.toml env var


*** Configuration and log format

~/.config/librsvg.toml - global configuration
  [logging]
  enabled=true
  process=gnome-shell              # mandatory - don't want to log all processes
  output=/home/federico/rsvg.log

/home/federico/rsvg.log - json doesn't have comments; put this in a string somehow:
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

Output JSON, so it can nest <g> and such?

Add a replayer?  This would effectively be "paint the render tree".
Just replay the user's provided log file, reproduce the bug.

