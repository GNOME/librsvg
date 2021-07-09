# NMake Makefile snippet for copying the built libraries, utilities and headers to
# a path under $(PREFIX).

install: all
	@if not exist $(PREFIX)\bin\ mkdir $(PREFIX)\bin
	@if not exist $(PREFIX)\lib\gdk-pixbuf-2.0\2.10.0\loaders\ mkdir $(PREFIX)\lib\gdk-pixbuf-2.0\2.10.0\loaders
	@if not exist $(PREFIX)\include\librsvg-$(RSVG_API_VER)\librsvg @mkdir $(PREFIX)\include\librsvg-$(RSVG_API_VER)\librsvg
	@copy /b $(OUTDIR)\rsvg-2*.dll $(PREFIX)\bin
	@copy /b $(OUTDIR)\rsvg_2.pdb $(PREFIX)\bin
	@copy /b $(LIBRSVG_LIB) $(PREFIX)\lib
	@copy /b $(LIBRSVG_LIB:-2.0=-2-static) $(PREFIX)\lib
	@for %%x in (dll pdb) do @copy /b $(OUTDIR)\libpixbufloader-svg.%%x $(PREFIX)\lib\gdk-pixbuf-2.0\2.10.0\loaders
	@copy $(OUTDIR)\rsvg-convert.exe $(PREFIX)\bin
	@-copy $(OUTDIR)\rsvg_convert.pdb $(PREFIX)\bin
	@for %%h in ($(librsvg_real_pub_HDRS)) do @copy %%h $(PREFIX)\include\librsvg-$(RSVG_API_VER)\librsvg\%%~nxh
	@set PATH=$(PREFIX)\bin;$(BINDIR);$(PATH)
	@-gdk-pixbuf-query-loaders > loaders.cache
	@for %%f in (loaders.cache) do @if %%~zf equ 0 echo *** GDK-Pixbuf loaders cache is not generated.  Run `gdk-pixbuf-query-loaders` in your ^$(PREFIX)\bin to generate it.
	@for %%f in (loaders.cache) do @if %%~zf equ 0 del loaders.cache
	@if exist loaders.cache move loaders.cache $(PREFIX)\lib\gdk-pixbuf-2.0\2.10.0
	@rem Copy the generated introspection files, if built
	@if exist $(OUTDIR)\Rsvg-$(RSVG_API_VER).gir if not exist $(PREFIX)\share\gir-1.0\ mkdir $(PREFIX)\share\gir-1.0
	@if exist $(OUTDIR)\Rsvg-$(RSVG_API_VER).gir copy $(OUTDIR)\Rsvg-$(RSVG_API_VER).gir $(PREFIX)\share\gir-1.0
	@if exist $(OUTDIR)\Rsvg-$(RSVG_API_VER).typelib if not exist $(PREFIX)\lib\girepository-1.0\ mkdir $(PREFIX)\lib\girepository-1.0
	@if exist $(OUTDIR)\Rsvg-$(RSVG_API_VER).typelib copy /b $(OUTDIR)\Rsvg-$(RSVG_API_VER).typelib $(PREFIX)\lib\girepository-1.0
	@-$(PYTHON) rsvgpc.py --version=$(RSVG_PKG_VERSION) --prefix=$(PREFIX) --source=$(OUTDIR)\librsvg-2.0.pc -o $(OUTDIR)\librsvg-2.0.pc.real
	@if not exist $(OUTDIR)\librsvg-2.0.pc.real echo *** librsvg-2.0.pc may not contain a ^$prefix that matches your build config.  Please check it.
	@if not exist $(PREFIX)\lib\pkgconfig mkdir $(PREFIX)\lib\pkgconfig
	@if exist $(OUTDIR)\librsvg-2.0.pc.real copy $(OUTDIR)\librsvg-2.0.pc.real $(PREFIX)\lib\pkgconfig\librsvg-2.0.pc
	@if not exist $(OUTDIR)\librsvg-2.0.pc.real copy $(OUTDIR)\librsvg-2.0.pc $(PREFIX)\lib\pkgconfig
	@echo.
	@echo ******* WARNING *******
	@echo People upgrading from librsvg-2.50.x or earlier
	@echo may want to consider recompiling their application
	@echo against this build of librsvg, as $(LIBRSVG_DLL)
	@echo is provided as a convenience and linking against
	@echo rsvg-2.0.lib will now always link to rsvg-2.dll.
	@echo ******* WARNING *******
