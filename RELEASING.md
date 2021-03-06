# Release process checklist for librsvg

Feel free to print this document or copy it to a text editor to check
off items while making a release.

- [ ] Refresh your memory with https://wiki.gnome.org/MaintainersCorner/Releasing
- [ ] Increase the package version number in `configure.ac` (it may already be increased but not released; double-check it).
- [ ] Copy version number to `Cargo.toml`.
- [ ] Tweak the library version number in `configure.ac` if the API changed; follow the steps there.
- [ ] Update `NEWS`, see below for the preferred format.
- [ ] Commit the changes above.
- [ ] Make a tarball with `make distcheck DESTDIR=/tmp/foo` - fix things until it passes.
- [ ] Create a signed tag - `git tag -s x.y.z` with the version number.
- [ ] `git push` to the appropriate branch to gitlab.gnome.org/GNOME/librsvg
- [ ] `git push` the signed tag to gitlab.gnome.org/GNOME/librsvg
- [ ] `scp librsvg-x.y.z.tar.xz master.gnome.org:`
- [ ] `ssh master.gnome.org` and then `ftpadmin install librsvg-x.y.z.tar.xz`
- [ ] If this is a `x.y.0` release, [notify the release team][release-team] on whether to use it for the next GNOME version via an issue on their `GNOME/releng` project.

## Version numbers

`configure.ac` and `Cargo.toml` must have the same **package version**
number - this is the number that users of the library see.

`configure.ac` is where the **library version** is defined; this is
what gets encoded in the SONAME of `librsvg.so`.

Librsvg follows an even/odd numbering scheme for the **package
version**.  For example, the 2.50.x series is for stable releases, and
2.51.x is for unstable/development ones.  The [release-team] needs to
be notified when a new series comes about, so they can adjust their
tooling for the stable or development GNOME releases.  File an issue
in their [repository][release-team] to indicate whether the new
`librsvg-x.y.0` is a stable or development series.

## Format for release notes in NEWS

The `NEWS` file contains the release notes.  Please use something
close to this format; it is not mandatory, but makes the formatting
consistent, and is what tooling expects elsewhere.  Skim bits of the
NEWS file for examples on style and content.

New entries go at the **top** of the file.

```
=============
Version x.y.z
=============

Commentary on the release; put anything here that you want to
highlight.  Note changes in the build process, if any, or any other
things that may trip up distributors.

Next is a list of features added and issues fixed; use gitlab's issue
numbers. I tend to use this order: first security bugs, then new
features and user-visible changes, finally regular bugs.  The
rationale is that if people stop reading early, at least they will
have seen the most important stuff first.

- #123 - title of the issue, or short summary if it warrants more
  discussion than just the title.

- #456 - fix blah blah (Contributor's Name).

Special thanks for this release:

- Any people that you want to highlight.  Feel free to omit this
  section if the release is otherwise unremarkable.
```

## Making a tarball

```
make distcheck DESTDIR=/tmp/foo
```

The `DESTDIR` is a quirk, required because otherwise the gdk-pixbuf
loader will try to install itself into the system's location for
pixbuf loaders, and it won't work.  The `DESTDIR` is what Linux
distribution packaging scripts use to `make install` the compiled
artifacts to a temporary location before building a system package.

## Copying the tarball to master.gnome.org

If you don't have a maintainer account there, ask federico@gnome.org
to do it or [ask the release team][release-team] to do it by filing an
issue on their `GNOME/releng` project.

[release-team]: https://gitlab.gnome.org/GNOME/releng/-/issues
