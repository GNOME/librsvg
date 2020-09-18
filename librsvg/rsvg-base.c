/* -*- Mode: C; tab-width: 4; indent-tabs-mode: nil; c-basic-offset: 4 -*- */
/* vim: set sw=4 sts=4 expandtab: */
/*
   rsvg.c: SAX-based renderer for SVG files into a GdkPixbuf.

   Copyright (C) 2000 Eazel, Inc.
   Copyright (C) 2002 Dom Lachowicz <cinamod@hotmail.com>

   This library is free software; you can redistribute it and/or
   modify it under the terms of the GNU Lesser General Public
   License as published by the Free Software Foundation; either
   version 2.1 of the License, or (at your option) any later version.

   This library is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Lesser General Public License for more details.

   You should have received a copy of the GNU Lesser General Public
   License along with this library; if not, write to the Free Software
   Foundation, Inc., 51 Franklin Street, Fifth Floor, Boston, MA  02110-1301  USA

   Author: Raph Levien <raph@artofcode.com>
*/

#include "config.h"

#include "rsvg.h"
#include "rsvg-css.h"

/* Implemented in rsvg_internals/src/dpi.rs */
extern void rsvg_rust_set_default_dpi_x_y(double dpi_x, double dpi_y);

/* Implemented in rsvg_internals/src/error.rs */
extern GQuark rsvg_rust_error_quark (void);

/* Implemented in librsvg/c_api.rs */
extern void rsvg_rust_init (void);
extern void rsvg_rust_term (void);
extern void rsvg_rust_cleanup (void);

/**
 * rsvg_error_quark:
 *
 * The error domain for RSVG
 *
 * Returns: The error domain
 */
GQuark
rsvg_error_quark (void)
{
    return rsvg_rust_error_quark ();
}

/**
 * rsvg_set_default_dpi:
 * @dpi: Dots Per Inch (aka Pixels Per Inch)
 *
 * Do not use this function.  Create an #RsvgHandle and call
 * rsvg_handle_set_dpi() on it instead.
 *
 * Since: 2.8
 *
 * Deprecated: 2.42.3: This function used to set a global default DPI.  However,
 * it only worked if it was called before any #RsvgHandle objects had been
 * created; it would not work after that.  To avoid global mutable state, please
 * use rsvg_handle_set_dpi() instead.
 */
void
rsvg_set_default_dpi (double dpi)
{
    rsvg_rust_set_default_dpi_x_y (dpi, dpi);
}

/**
 * rsvg_set_default_dpi_x_y:
 * @dpi_x: Dots Per Inch (aka Pixels Per Inch)
 * @dpi_y: Dots Per Inch (aka Pixels Per Inch)
 *
 * Do not use this function.  Create an #RsvgHandle and call
 * rsvg_handle_set_dpi_x_y() on it instead.
 *
 * Since: 2.8
 *
 * Deprecated: 2.42.3: This function used to set a global default DPI.  However,
 * it only worked if it was called before any #RsvgHandle objects had been
 * created; it would not work after that.  To avoid global mutable state, please
 * use rsvg_handle_set_dpi() instead.
 */
void
rsvg_set_default_dpi_x_y (double dpi_x, double dpi_y)
{
    rsvg_rust_set_default_dpi_x_y (dpi_x, dpi_y);
}

/**
 * rsvg_init:
 *
 * This function does nothing.
 *
 * Since: 2.9
 * Deprecated: 2.36: There is no need to initialize librsvg.
 **/
void
rsvg_init (void)
{
    rsvg_rust_init ();
}

/**
 * rsvg_term:
 *
 * This function does nothing.
 *
 * Since: 2.9
 * Deprecated: 2.36: There is no need to de-initialize librsvg.
 **/
void
rsvg_term (void)
{
    rsvg_rust_term ();
}

/**
 * rsvg_cleanup:
 *
 * Deprecated: 2.46: No-op. This function should not be called from normal programs.
 *
 * Since: 2.36
 **/
void
rsvg_cleanup (void)
{
    rsvg_rust_cleanup ();
}

/* This is defined like this so that we can export the Rust function... just for
 * the benefit of rsvg-convert.c
 */
RsvgCssColorSpec
rsvg_css_parse_color_ (const char *str)
{
    return rsvg_css_parse_color (str);
}
