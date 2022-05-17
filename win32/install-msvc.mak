# NMake Makefile snippet for copying the built libraries, utilities and headers to
# a path under $(PREFIX).

install: all
	@if not exist $(PREFIX)\bin\ mkdir $(PREFIX)\bin
	@if not exist $(PREFIX)\lib\gdk-pixbuf-2.0\2.10.0\loaders\ mkdir $(PREFIX)\lib\gdk-pixbuf-2.0\2.10.0\loaders
	@if not exist $(PREFIX)\include\librsvg-$(RSVG_API_VER)\librsvg @mkdir $(PREFIX)\include\librsvg-$(RSVG_API_VER)\librsvg
	@for %%x in (dll pdb) do @copy /b $(LIBRSVG_DLL_FILENAME).%%x $(PREFIX)\bin
	@copy /b $(LIBRSVG_LIB) $(PREFIX)\lib
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
	@-$(PYTHON) rsvgpc.py --version=$(RSVG_PKG_VERSION) --prefix=$(PREFIX)
	@if not exist librsvg-2.0.pc echo *** librsvg-2.0.pc is not generated!  Generate it later using ^$(PYTHON) --version=$(RSVG_PKG_VERSION) --prefix=^$(PREFIX)
	@if exist librsvg-2.0.pc if not exist $(PREFIX)\lib\pkgconfig mkdir $(PREFIX)\lib\pkgconfig
	@if exist librsvg-2.0.pc move librsvg-2.0.pc $(PREFIX)\lib\pkgconfig
	@if exist Rsvg-$(RSVG_API_VER)\ if not exist $(PREFIX)\doc\Rsvg-$(RSVG_API_VER)\ mkdir $(PREFIX)\doc\Rsvg-$(RSVG_API_VER)
	@if exist Rsvg-$(RSVG_API_VER)\ copy /b/y Rsvg-$(RSVG_API_VER)\* $(PREFIX)\doc\Rsvg-$(RSVG_API_VER)
