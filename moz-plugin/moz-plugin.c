/* vim: set sw=4: -*- Mode: C; tab-width: 4; indent-tabs-mode: t; c-basic-offset: 4 -*- */
/*
   moz-plugin.c: Mozilla plugin

   Copyright (C) 2003-2004 Dom Lachowicz <cinamod@hotmail.com>
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

#include <config.h>

#include <stdio.h>
#include <unistd.h>
#include <sys/wait.h>
#include <signal.h>
#include <stdarg.h>

#include <X11/Xlib.h>
#include <X11/Intrinsic.h>

#include <glib.h>

#define XP_UNIX 1
#define MOZ_X11 1
#include "npapi.h"
#include "npupp.h"

#define DEBUG(x) _svg_plugin_debug x

static void 
_svg_plugin_debug(const char * fmt, ...)
{
#if 0
	FILE * err;
	va_list ap;

	err = fopen("/tmp/moz.svg", "ab");

	va_start(ap, fmt);
	vfprintf(err, fmt, ap);
	va_end(ap);

	fclose(err);
#endif
}

typedef struct
{
	NPP instance;
	Window window;

	int width, height, window_width, window_height;
	int sizes_in_percentages;

	GByteArray * bytes;

	int send_fd;
	int pid;

	char *base_url;
} SvgPlugin;

static NPNetscapeFuncs mozilla_funcs;

static void
svg_plugin_kill (SvgPlugin * plugin)
{
	DEBUG(("plugin_kill\n"));

	if(plugin->send_fd > 0)
		{
			close (plugin->send_fd);
			plugin->send_fd = -1;
		}

	if(plugin->pid > 0)
		{
			kill (plugin->pid, SIGKILL);
			waitpid (plugin->pid, NULL, 0);
			
			plugin->pid = -1;
		}
}

static NPError
svg_plugin_fork (SvgPlugin * plugin, int * plugin_pid, int * plugin_send_fd)
{
	char xid_str[20];
	char width_str[G_ASCII_DTOSTR_BUF_SIZE];
	char height_str[G_ASCII_DTOSTR_BUF_SIZE];
	char window_width_str[G_ASCII_DTOSTR_BUF_SIZE];
	char window_height_str[G_ASCII_DTOSTR_BUF_SIZE];
	char *argv[32];
	int argc = 0;
	GError *err = NULL;
	
	DEBUG(("plugin_fork\n"));

	sprintf (xid_str, "%ld", plugin->window);
			
	argv[argc++] = BINDIR "rsvg-view";

	/* xid */
	argv[argc++] = "-i";
	argv[argc++] = xid_str;
	
	if (plugin->width)
		{
			/* width */
			if (plugin->sizes_in_percentages) {

				if(plugin->window_width > 0) {
					sprintf (window_width_str, "%d", plugin->window_width);
					argv[argc++] = "-w";
					argv[argc++] = window_width_str;
				}

				g_ascii_dtostr (width_str, sizeof (width_str), (double)plugin->width / 100.);
				argv[argc++] = "-x";
			} 
			else {
				sprintf (width_str, "%d", plugin->width);
				argv[argc++] = "-w";
			}

			argv[argc++] = width_str;
		}
	
	if (plugin->height)
		{
			/* height */
			if (plugin->sizes_in_percentages) {

				if(plugin->window_height > 0) {
					sprintf (window_height_str, "%d", plugin->window_height);
					argv[argc++] = "-h";
					argv[argc++] = window_height_str;
				}

				g_ascii_dtostr (height_str, sizeof (height_str), (double)plugin->height / 100.);
				argv[argc++] = "-y";
			} 
			else {
				sprintf (height_str, "%d", plugin->height);
				argv[argc++] = "-h";
			}

			argv[argc++] = height_str;
		}
	
	/* HACK!! hardcode bg color to white for Uraeus' viewing pleasure */
	argv[argc++] = "-b";
	argv[argc++] = "white";

	if (plugin->base_url) {
		argv[argc++] = "-u";
		argv[argc++] = plugin->base_url;
	}

	/* HACK: keep aspect ratio */
	if (plugin->sizes_in_percentages) {
		argv[argc++] = "-k";
	}

	/* read from stdin */
	argv[argc++] = "-s";
	argv[argc] = NULL;

	if(!g_spawn_async_with_pipes(NULL, argv, NULL, G_SPAWN_DO_NOT_REAP_CHILD | G_SPAWN_STDOUT_TO_DEV_NULL, 
								 NULL, NULL, plugin_pid,
								 plugin_send_fd, NULL, NULL, &err))
		{
			DEBUG(("Spawn failed\n"));

			if(err) 
				{
					fprintf(stderr, "%s\n", err->message);
					g_error_free(err);
				}

			return NPERR_INVALID_INSTANCE_ERROR;
		}

	return NPERR_NO_ERROR;
}

static NPError
svg_plugin_redraw (SvgPlugin * plugin)
{
	NPError res = NPERR_NO_ERROR;

	DEBUG(("plugin_redraw\n"));

	if(plugin && plugin->bytes && plugin->bytes->len)
		{
			if (plugin->pid <= 0)
				{
					if ((res = svg_plugin_fork (plugin, &plugin->pid, &plugin->send_fd)) == NPERR_NO_ERROR) {
						if(plugin->pid > 0)
							{
								size_t nwritten = 0;
								while(nwritten < plugin->bytes->len)
									nwritten += write (plugin->send_fd, plugin->bytes->data + nwritten, plugin->bytes->len - nwritten);
							}
						else
							{
								res = NPERR_INVALID_INSTANCE_ERROR;
							}
					}
				}
		}

	return res;
}

static NPError
svg_plugin_newp (NPMIMEType mime_type, NPP instance,
				 guint16 mode, gint16 argc, char *argn[], char *argv[],
				 NPSavedData * saved)
{
	SvgPlugin *plugin;
	int i;
	
	DEBUG (("plugin_newp\n"));
  
	if (instance == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;
	
	instance->pdata = mozilla_funcs.memalloc (sizeof (SvgPlugin));
	plugin = (SvgPlugin *) instance->pdata;

	if (plugin == NULL)
		return NPERR_OUT_OF_MEMORY_ERROR;
	memset (plugin, 0, sizeof (SvgPlugin));
	
	/* mode is NP_EMBED, NP_FULL, or NP_BACKGROUND (see npapi.h) */
	plugin->instance = instance;
	
	for (i = 0; i < argc; i++)
		{
			DEBUG (("argv[%d] %s %s\n", i, argn[i], argv[i]));
			
			if (strcmp (argn[i], "width") == 0) {
				if (strstr (argv [i], "%") != NULL)
					plugin->sizes_in_percentages = 1;
				plugin->width = strtol (argv[i], NULL, 0);
			}

			if (strcmp (argn[i], "height") == 0) {
				if (strstr (argv [i], "%") != NULL)
					plugin->sizes_in_percentages = 1;
				plugin->height = strtol (argv[i], NULL, 0);
			}
		}   

  return NPERR_NO_ERROR;
}

static NPError
svg_plugin_destroy (NPP instance, NPSavedData ** save)
{
	SvgPlugin *plugin;
	
	DEBUG (("plugin_destroy\n"));
	
	if (instance == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;
	
	plugin = (SvgPlugin *) instance->pdata;
	if (plugin == NULL)
		return NPERR_NO_ERROR;

	if(plugin->bytes)
		g_byte_array_free (plugin->bytes, TRUE);
	
	svg_plugin_kill (plugin);

	if (plugin->base_url)
		mozilla_funcs.memfree (plugin->base_url);

	mozilla_funcs.memfree (instance->pdata);
	instance->pdata = NULL;
	
	return NPERR_NO_ERROR;
}

static NPError
svg_plugin_set_window (NPP instance, NPWindow * window)
{
	SvgPlugin *plugin;
	NPError res = NPERR_NO_ERROR;
	
	DEBUG (("plugin_set_window\n"));
	
	if (instance == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;

	plugin = (SvgPlugin *) instance->pdata;
	if (plugin == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;
	
	if (plugin->window)
		{
			if (plugin->window == (Window) window->window)
				{
					DEBUG (("window resized\n"));

#if 0
					plugin->window_width = window->width;
					plugin->window_height = window->height;

					res = svg_plugin_redraw (plugin);
#endif
				}
			else
				{
					DEBUG (("change. ack.  window changed!\n"));
				}
		}
	else
		{
			NPSetWindowCallbackStruct *ws_info;
			
			ws_info = window->ws_info;
			plugin->window = (Window) window->window;
		}
	
	return res;
}

static NPError
svg_plugin_new_stream (NPP instance, NPMIMEType type, NPStream *stream,
		      NPBool seekable, uint16 *stype)
{
	SvgPlugin *plugin;

	DEBUG (("plugin_new_stream\n"));

	if (instance == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;
	
	plugin = (SvgPlugin *) instance->pdata;
	if (plugin == NULL)
		return NPERR_NO_ERROR;	
	
	g_return_val_if_fail(plugin->bytes == NULL, NPERR_NO_ERROR);

	plugin->bytes = g_byte_array_new();
	
	*stype = NP_NORMAL;

	return NPERR_NO_ERROR;
}

static NPError
svg_plugin_destroy_stream (NPP instance, NPStream * stream, NPError reason)
{
	SvgPlugin *plugin;
	NPError res = NPERR_NO_ERROR;
	size_t url_len;

	DEBUG (("plugin_destroy_stream\n"));

	if (instance == NULL)
		return NPERR_INVALID_INSTANCE_ERROR;
	
	plugin = (SvgPlugin *) instance->pdata;
	if (plugin == NULL)
		return NPERR_NO_ERROR;

	if (stream->url) {
		url_len = strlen(stream->url);
		plugin->base_url = mozilla_funcs.memalloc (url_len + 1);
		strcpy(plugin->base_url, stream->url);
		plugin->base_url[url_len] = '\0';
	}

	/* trigger */
	res = svg_plugin_redraw (plugin);

	if(plugin->send_fd > 0)
		{
			close (plugin->send_fd);
			plugin->send_fd = -1;
		}

	return res;
}

static gint32
svg_plugin_write_ready (NPP instance, NPStream * stream)
{
	DEBUG (("plugin_write_ready\n"));
	
	/* This is arbitrary */
	return (8*1024);
}

static void
svg_plugin_print (NPP instance, NPPrint * platformPrint)
{
	SvgPlugin *plugin;

	DEBUG (("plugin_print\n"));
	
	if (instance == NULL)
		return;

	plugin = (SvgPlugin *) instance->pdata;
	
	if (plugin == NULL)
		return;

	if (NP_EMBED == platformPrint->mode) {
		NPPrintCallbackStruct *printer = (NPPrintCallbackStruct *)platformPrint->print.embedPrint.platformPrint;

		if (0) {
			gchar * temp_filename;
			FILE * fp;
			int fd;
			
			temp_filename = g_build_filename (g_get_tmp_dir(), ".mozilla-svg-print-XXXXXX", NULL);
			fd = g_mkstemp (temp_filename);
			close (fd);

			/* todo: get rsvg-view to print the file */

			if ((fp = fopen (temp_filename, "rb")) != NULL) {
				gchar buf [8*1024];
				gint nread;

				while ((nread = fread (buf, 1, sizeof (buf), fp)) > 0) {
					gint nwritten = 0;

					do {
						nwritten += fwrite (buf + nwritten, 1, (nread - nwritten), printer->fp);
					} while (nwritten != nread);
				}
				
				fclose (fp);
			}

			remove (temp_filename);

			g_free (temp_filename);
		}
	}
}

static gint32
svg_plugin_write (NPP instance, NPStream * stream, gint32 offset,
			  gint32 len, void *buffer)
{
	SvgPlugin *plugin;

	DEBUG (("plugin_write\n"));
	
	if (instance == NULL)
		return 0;

	plugin = (SvgPlugin *) instance->pdata;
	
	if (plugin == NULL)
		return 0;
	
	if (!plugin->bytes)
		return 0;
	
	(void)g_byte_array_append (plugin->bytes, buffer, len);

	return len;
}

static void
svg_plugin_stream_as_file (NPP instance, NPStream * stream, const char *fname)
{
	SvgPlugin *plugin;
	
	DEBUG (("plugin_stream_as_file\n"));
	
	if (instance == NULL)
		return;
	plugin = (SvgPlugin *) instance->pdata;
	
	if (plugin == NULL)
		return;
}

/* exported functions */

NPError
NP_GetValue (void *future, NPPVariable variable, void *value)
{
	NPError err = NPERR_NO_ERROR;
	
	DEBUG (("NP_GetValue\n"));

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
	DEBUG (("NP_GetMIMEDescription\n"));

	/* unfortunately, a lot of win32 servers serving up Adobe content return bogus mime-types... */
	return ("image/svg+xml:svg,svgz:Scalable Vector Graphics;image/svg-xml:svg,svgz:Scalable Vector Graphics;"
			"image/svg:svg,svgz:Scalable Vector Graphics;image/vnd.adobe.svg+xml:svg,svgz:Scalable Vector Graphics;"
			"text/xml-svg:svg,svgz:Scalable Vector Graphics");
}

NPError
NP_Initialize (NPNetscapeFuncs * moz_funcs, NPPluginFuncs * plugin_funcs)
{
	DEBUG (("NP_Initialize\n"));
	
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
	plugin_funcs->newp = NewNPP_NewProc (svg_plugin_newp);
	plugin_funcs->destroy = NewNPP_DestroyProc (svg_plugin_destroy);
	plugin_funcs->setwindow = NewNPP_SetWindowProc (svg_plugin_set_window);
	plugin_funcs->newstream = NewNPP_NewStreamProc (svg_plugin_new_stream);
	plugin_funcs->destroystream =
		NewNPP_DestroyStreamProc (svg_plugin_destroy_stream);
	plugin_funcs->writeready = NewNPP_WriteReadyProc (svg_plugin_write_ready);
	plugin_funcs->asfile = NewNPP_StreamAsFileProc (svg_plugin_stream_as_file);
	plugin_funcs->write = NewNPP_WriteProc (svg_plugin_write);
	plugin_funcs->print = NewNPP_PrintProc (svg_plugin_print);

	return NPERR_NO_ERROR;
}

NPError
NP_Shutdown (void)
{
	DEBUG(("NP_Shutdown"));

	return NPERR_NO_ERROR;
}
