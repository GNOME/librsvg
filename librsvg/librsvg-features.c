#include "rsvg.h"

/**
 * LIBRSVG_MAJOR_VERSION:
 *
 * This is a C macro that expands to a number with the major version
 * of librsvg against which your program is compiled.
 *
 * For example, for librsvg-2.3.4, the major version is 2.
 *
 * C programs can use this as a compile-time check for the required
 * version, but note that generally it is a better idea to do
 * compile-time checks by calling <ulink
 * url="https://www.freedesktop.org/wiki/Software/pkg-config/">pkg-config</ulink>
 * in your build scripts.
 *
 * Note: for a run-time check on the version of librsvg that your
 * program is running with (e.g. the version which the linker used for
 * your program), or for programs not written in C, use
 * @rsvg_major_version instead.
 */

/**
 * LIBRSVG_MINOR_VERSION:
 *
 * This is a C macro that expands to a number with the minor version
 * of librsvg against which your program is compiled.
 *
 * For example, for librsvg-2.3.4, the minor version is 3.
 *
 * C programs can use this as a compile-time check for the required
 * version, but note that generally it is a better idea to do
 * compile-time checks by calling <ulink
 * url="https://www.freedesktop.org/wiki/Software/pkg-config/">pkg-config</ulink>
 * in your build scripts.
 *
 * Note: for a run-time check on the version of librsvg that your
 * program is running with (e.g. the version which the linker used for
 * your program), or for programs not written in C, use
 * @rsvg_minor_version instead.
 */

/**
 * LIBRSVG_MICRO_VERSION:
 *
 * This is a C macro that expands to a number with the micro version
 * of librsvg against which your program is compiled.
 *
 * For example, for librsvg-2.3.4, the micro version is 4.
 *
 * C programs can use this as a compile-time check for the required
 * version, but note that generally it is a better idea to do
 * compile-time checks by calling <ulink
 * url="https://www.freedesktop.org/wiki/Software/pkg-config/">pkg-config</ulink>
 * in your build scripts.
 *
 * Note: for a run-time check on the version of librsvg that your
 * program is running with (e.g. the version which the linker used for
 * your program), or for programs not written in C, use
 * @rsvg_micro_version instead.
 */

/**
 * LIBRSVG_VERSION:
 *
 * This is a C macro that expands to a string with the version of
 * librsvg against which your program is compiled.
 *
 * For example, for librsvg-2.3.4, this macro expands to
 * <literal>"2.3.4"</literal>.
 *
 * C programs can use this as a compile-time check for the required
 * version, but note that generally it is a better idea to do
 * compile-time checks by calling <ulink
 * url="https://www.freedesktop.org/wiki/Software/pkg-config/">pkg-config</ulink>
 * in your build scripts.
 *
 * Note: for a run-time check on the version of librsvg that your
 * program is running with (e.g. the version which the linker used for
 * your program), or for programs not written in C, use
 * @rsvg_version instead.
 */

/**
 * rsvg_major_version:
 *
 * Major version of the library.  For example, for version 2.3.4, the major
 * version will be 2.
 *
 * Since: 2.52
 */
const guint rsvg_major_version = LIBRSVG_MAJOR_VERSION;

/**
 * rsvg_minor_version:
 *
 * Minor version of the library.  For example, for version 2.3.4, the minor
 * version will be 3.
 *
 * Since: 2.52
 */
const guint rsvg_minor_version = LIBRSVG_MINOR_VERSION;

/**
 * rsvg_micro_version:
 *
 * Micro version of the library.  For example, for version 2.3.4, the micro
 * version will be 4.
 *
 * Since: 2.52
 */
const guint rsvg_micro_version = LIBRSVG_MICRO_VERSION;

/**
 * rsvg_version:
 *
 * String with the library version.  For example, "<literal>2.3.4</literal>".
 *
 * Since: 2.52
 */
const char rsvg_version[] = LIBRSVG_VERSION;
