/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/* 
   rsvg-gz.h: SAX-based renderer for SVGZ files into a GdkPixbuf.
 
   Copyright (C) 2003 Dom Lachowicz
  
   This program is free software; you can redistribute it and/or
   modify it under the terms of the GNU Library General Public License as
   published by the Free Software Foundation; either version 2 of the
   License, or (at your option) any later version.
  
   This program is distributed in the hope that it will be useful,
   but WITHOUT ANY WARRANTY; without even the implied warranty of
   MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
   Library General Public License for more details.
  
   You should have received a copy of the GNU Library General Public
   License along with this program; if not, write to the
   Free Software Foundation, Inc., 59 Temple Place - Suite 330,
   Boston, MA 02111-1307, USA.
*/

#ifndef RSVG_GZ_H
#define RSVG_GZ_H

#include "rsvg.h"

G_BEGIN_DECLS

RsvgHandle * rsvg_handle_new_gz (void);

G_END_DECLS

#endif /* RSVG_GZ_H */
