/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   moz-plugin.c: Mozilla plugin

   Copyright (C) 2003 Dom Lachowicz <cinamod@hotmail.com>
   Copyright (C) 2003 David Schleef <ds@schleef.org>

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

   Author: Dom Lachowicz <cinamod@hotmail.com>  
*/

#include <stdio.h>
#include <stdint.h>
#include <unistd.h>
#include <sys/wait.h>
#include <config.h>

#include <X11/Xlib.h>
#include <X11/Intrinsic.h>

#define XP_UNIX 1
#define MOZ_X11 1
#include "npapi.h"
#include "npupp.h"

#define DEBUG(x)

typedef struct
{
	NPP instance;
	Display *display;
	Window window;
	int x;
	int y;
	int width, height;
} Plugin;

static NPNetscapeFuncs mozilla_funcs;

static NPError
plugin_newp (NPMIMEType mime_type, NPP instance,
			 uint16_t mode, int16_t argc, char *argn[], char *argv[],
			 NPSavedData * saved)
{
	Plugin *plugin;
	int i;
	
	DEBUG ("plugin_newp");
	
	if (instance == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;
	
	instance->pdata = mozilla_funcs.memalloc (sizeof (Plugin));
	plugin = (Plugin *) instance->pdata;
	
	if (plugin == NULL)
		return NPERR_OUT_OF_MEMORY_ERROR;
	memset (plugin, 0, sizeof (Plugin));
	
	/* mode is NP_EMBED, NP_FULL, or NP_BACKGROUND (see npapi.h) */
	plugin->instance = instance;
	
	for (i = 0; i < argc; i++)
		{
			printf ("argv[%d] %s %s\n", i, argn[i], argv[i]);
			if (strcmp (argn[i], "width") == 0)
				{
					plugin->width = strtol (argv[i], NULL, 0);
				}
			if (strcmp (argn[i], "height") == 0)
				{
					plugin->height = strtol (argv[i], NULL, 0);
				}
		}
	
	return NPERR_NO_ERROR;
}

static NPError
plugin_destroy (NPP instance, NPSavedData ** save)
{
	Plugin *plugin;
	
	DEBUG ("plugin_destroy");
	
	if (instance == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;
	
	plugin = (Plugin *) instance->pdata;
	if (plugin == NULL)
		{
			return NPERR_NO_ERROR;
		}
	
	mozilla_funcs.memfree (instance->pdata);
	instance->pdata = NULL;
	
	return NPERR_NO_ERROR;
}

static NPError
plugin_set_window (NPP instance, NPWindow * window)
{
	Plugin *plugin;

	DEBUG ("plugin_set_window");
	
	if (instance == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;
	
	plugin = (Plugin *) instance->pdata;
	if (plugin == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;
	
	if (plugin->window)
		{
			DEBUG ("existing window");
			if (plugin->window == (Window) window->window)
				{
					DEBUG ("resize");
					/* Resize event */
					/* Not currently handled */
				}
			else
				{
					DEBUG ("change");
					printf ("ack.  window changed!\n");
				}
		}
	else
		{
			NPSetWindowCallbackStruct *ws_info;
			
			DEBUG ("about to fork");
			
			ws_info = window->ws_info;
			plugin->window = (Window) window->window;
			plugin->display = ws_info->display;
			
			plugin_fork (plugin);
		}
	
	DEBUG ("leaving plugin_set_window");
	
	return NPERR_NO_ERROR;
}

static NPError
plugin_new_stream (NPP instance, NPMIMEType type,
				   const char *window, NPStream ** stream_ptr)
{
	DEBUG ("plugin_new_stream");
	
	return NPERR_NO_ERROR;
}

static NPError
plugin_destroy_stream (NPP instance, NPStream * stream, NPError reason)
{
	DEBUG ("plugin_destroy_stream");
	
	return NPERR_NO_ERROR;
}

static int32
plugin_write_ready (NPP instance, NPStream * stream)
{
	/* This is arbitrary */
	
	DEBUG ("plugin_write_ready");
	
	return 4096;
}

static int32
plugin_write (NPP instance, NPStream * stream, int32 offset,
			  int32 len, void *buffer)
{
	Plugin *plugin;
	
	DEBUG ("plugin_write");
	
	if (instance == NULL)
		return 0;
	plugin = (Plugin *) instance->pdata;
	
	if (plugin == NULL)
		return 0;
	
	/* TODO */
	write (plugin->send_fd, buffer, len);
	
	return len;
}

static void
plugin_stream_as_file (NPP instance, NPStream * stream, const char *fname)
{
	Plugin *plugin;
	
	DEBUG ("plugin_stream_as_file");
	
	if (instance == NULL)
		return;
	plugin = (Plugin *) instance->pdata;
	
	if (plugin == NULL)
		return;
	
	printf ("plugin_stream_as_file\n");
}

/* exported functions */

NPError
NP_GetValue (void *future, NPPVariable variable, void *value)
{
	NPError err = NPERR_NO_ERROR;
	
	switch (variable)
		{
		case NPPVpluginNameString:
			*((char **) value) = "Scalable Vector Graphics";
			break;
		case NPPVpluginDescriptionString:
			*((char **) value) =
				"Scalable Vector Graphics, as handled by RSVG-" VERSION
				".  "
				"Views SVG images.<br><br>"
				"This is alpha software.  It will probably behave in many situations, but "
				"may also ride your motorcycle, drink all your milk, or use your computer "
				"to browse porn.  Comments, feature requests, and patches are welcome.<br><br>"
				"See <a href=\"http://librsvg.sourceforge.net/\">"
				"http://librsvg.sourceforge.net/</a> for information.<br><br>";
			break;
		default:
			err = NPERR_GENERIC_ERROR;
		}
	return err;
}

char *
NP_GetMIMEDescription (void)
{
	return ("application/svg:svg:Scalable Vector Graphics");
}

NPError
NP_Initialize (NPNetscapeFuncs * moz_funcs, NPPluginFuncs * plugin_funcs)
{
	printf ("NP_Initialize\n");
	
	if (moz_funcs == NULL || plugin_funcs == NULL)
		return NPERR_INVALID_FUNCTABLE_ERROR;
	
	if ((moz_funcs->version >> 8) > NP_VERSION_MAJOR)
		return NPERR_INCOMPATIBLE_VERSION_ERROR;
	if (moz_funcs->size < sizeof (NPNetscapeFuncs))
		return NPERR_INVALID_FUNCTABLE_ERROR;
	if (plugin_funcs->size < sizeof (NPPluginFuncs))
		return NPERR_INVALID_FUNCTABLE_ERROR;
	
	memcpy (&mozilla_funcs, moz_funcs, sizeof (NPNetscapeFuncs));
	
	plugin_funcs->version = (NP_VERSION_MAJOR << 8) + NP_VERSION_MINOR;
	plugin_funcs->size = sizeof (NPPluginFuncs);
	plugin_funcs->newp = NewNPP_NewProc (plugin_newp);
	plugin_funcs->destroy = NewNPP_DestroyProc (plugin_destroy);
	plugin_funcs->setwindow = NewNPP_SetWindowProc (plugin_set_window);
	plugin_funcs->newstream = NewNPP_NewStreamProc (plugin_new_stream);
	plugin_funcs->destroystream =
		NewNPP_DestroyStreamProc (plugin_destroy_stream);
	plugin_funcs->writeready = NewNPP_WriteReadyProc (plugin_write_ready);
	plugin_funcs->asfile = NewNPP_StreamAsFileProc (plugin_stream_as_file);
	plugin_funcs->write = NewNPP_WriteProc (plugin_write);
	
	return NPERR_NO_ERROR;
}

NPError
NP_Shutdown (void)
{
	return NPERR_NO_ERROR;
}
