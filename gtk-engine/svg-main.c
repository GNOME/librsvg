/* GTK+ Rsvg Engine
 * Copyright (C) 1998-2000 Red Hat, Inc.
 * Copyright (C) 2002 Dom Lachowicz
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Library General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Library General Public License for more details.
 *
 * You should have received a copy of the GNU Library General Public
 * License along with this library; if not, write to the
 * Free Software Foundation, Inc., 59 Temple Place - Suite 330,
 * Boston, MA 02111-1307, USA.
 *
 * Written by Owen Taylor <otaylor@redhat.com>, based on code by
 * Carsten Haitzler <raster@rasterman.com>
 */

#include "config.h"

#include "svg.h"
#include "svg-style.h"
#include "svg-rc-style.h"

#include <gmodule.h>

G_MODULE_EXPORT const gchar* g_module_check_init (GModule *module);
G_MODULE_EXPORT void theme_init (GTypeModule *module);
G_MODULE_EXPORT void theme_exit (void);
G_MODULE_EXPORT GtkRcStyle * theme_create_rc_style (void);

void
theme_init (GTypeModule *module)
{
  rsvg_rc_style_register_type (module);
  rsvg_style_register_type (module);
}

void
theme_exit (void)
{
}

GtkRcStyle *
theme_create_rc_style (void)
{
  return GTK_RC_STYLE (g_object_new (RSVG_TYPE_RC_STYLE, NULL));  
}

/* The following function will be called by GTK+ when the module
 * is loaded and checks to see if we are compatible with the
 * version of GTK+ that loads us.
 */
const gchar*
g_module_check_init (GModule *module)
{
  /* See bugs 357406 and 362217 */
  g_module_make_resident (module);

  return gtk_check_version (GTK_MAJOR_VERSION,
			    GTK_MINOR_VERSION,
			    GTK_MICRO_VERSION - GTK_INTERFACE_AGE);
}
