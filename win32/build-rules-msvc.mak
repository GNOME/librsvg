# NMake Makefile portion for compilation rules
# Items in here should not need to be edited unless
# one is maintaining the NMake build files.  The format
# of NMake Makefiles here are different from the GNU
# Makefiles.  Please see the comments about these formats.

# Inference rules for compiling the .obj files.
# Used for libs and programs with more than a single source file.
# Format is as follows
# (all dirs must have a trailing '\'):
#
# {$(srcdir)}.$(srcext){$(destdir)}.obj::
# 	$(CC)|$(CXX) $(cflags) /Fo$(destdir) /c @<<
# $<
# <<
{$(OUTDIR)\librsvg\}.c{$(OUTDIR)\librsvg\}.obj:
	$(CC) $(LIBRSVG_CFLAGS) $(LIBRSVG_INCLUDES) /Fo$(@D)\ /Fd$(@D)\ /c @<<
$<
<<

{..\gdk-pixbuf-loader\}.c{$(OUTDIR)\rsvg-gdk-pixbuf-loader\}.obj:
	@if not exist $(@D)\ mkdir $(@D)
	@if not exist $(@D)\..\librsvg\config.h copy .\config.h.win32 $(@D)\..\librsvg\config.h
	$(CC) $(RSVG_PIXBUF_LOADER_CFLAGS) $(TOOLS_DEP_INCLUDES) /Fo$(@D)\ /Fd$(@D)\ /c @<<
$<
<<

{..\tests\}.c{$(OUTDIR)\rsvg-tests\}.obj:
	@if not exist $(@D)\ mkdir $(@D)
	@if not exist $(@D)\..\librsvg\config.h copy .\config.h.win32 $(@D)\..\librsvg\config.h
	$(CC) $(TEST_CFLAGS) $(LIBRSVG_LOG_DOMAIN) $(TOOLS_DEP_INCLUDES) /Fo$(@D)\ /Fd$(@D)\ /c @<<
$<
<<

# Rules for building .lib files
$(LIBRSVG_LIB): $(LIBRSVG_DLL)

# Rules for linking DLLs
# Format is as follows (the mt command is needed for MSVC 2005/2008 builds):
# $(dll_name_with_path): $(dependent_libs_files_objects_and_items)
#	link /DLL [$(linker_flags)] [$(dependent_libs)] [/def:$(def_file_if_used)] [/implib:$(lib_name_if_needed)] -out:$@ @<<
# $(dependent_objects)
# <<
# 	@-if exist $@.manifest mt /manifest $@.manifest /outputresource:$@;2
$(LIBRSVG_DLL): $(RSVG_INTERNAL_LIB) $(librsvg_OBJS) $(LIBRSVG_DEF)
	link /DLL $(LDFLAGS) $(LIBRSVG_DEP_LIBS)	\
	/implib:$(LIBRSVG_LIB)	\
	-out:$@ /def:$(LIBRSVG_DEF) @<<
$(librsvg_OBJS)
<<
	@-if exist $@.manifest mt /manifest $@.manifest /outputresource:$@;2

$(GDK_PIXBUF_SVG_LOADER):	\
$(LIBRSVG_LIB)	\
$(OUTDIR)\rsvg-gdk-pixbuf-loader\io-svg.obj
	link /DLL $(LDFLAGS) $** $(BASE_DEP_LIBS) /out:$@
	@-if exist $@.manifest mt /manifest $@.manifest /outputresource:$@;2

# Rules for linking Executables
# Format is as follows (the mt command is needed for MSVC 2005/2008 builds):
# $(dll_name_with_path): $(dependent_libs_files_objects_and_items)
#	link [$(linker_flags)] [$(dependent_libs)] -out:$@ @<<
# $(dependent_objects)
# <<
# 	@-if exist $@.manifest mt /manifest $@.manifest /outputresource:$@;1
$(OUTDIR)\rsvg-convert.exe:	\
vs$(VSVER)\$(CFG)\$(PLAT)\obj\rsvg_c_api\$(RUST_TARGET)-pc-windows-msvc\$(CFG)\rsvg-convert.exe
	@copy /b $** $@
	@if exist $(**D)\rsvg_convert.pdb copy /b $(**D)\rsvg_convert.pdb $(@D)

# Include the rules for the test programs
!include rsvg_tests_rules.mak

!if [del /f /q rsvg_tests_rules.mak]
!endif

$(rsvg_tests):
	link $(LDFLAGS) $** $(TEST_DEP_LIBS) /out:$@
	@-if exist $@.manifest mt /manifest $@.manifest /outputresource:$@;1

!ifdef INTROSPECTION
$(OUTDIR)\Rsvg-$(RSVG_API_VER).gir: $(LIBRSVG_LIB) $(OUTDIR)\librsvg\Rsvg_2_0_gir_list
	@-echo Generating $@...
	@set PATH=$(BINDIR);$(PATH)
	$(PYTHON) $(G_IR_SCANNER)	\
	--verbose -no-libtool	\
	--namespace=Rsvg	\
	--nsversion=2.0	\
	--pkg=pango --extra-library=libxml2	\
	--library=rsvg-2.0	\
	--add-include-path=$(G_IR_INCLUDEDIR)	\
	--include=GLib-2.0 --include=GObject-2.0	\
	--include=Gio-2.0 --include=cairo-1.0	\
	--include=GdkPixbuf-2.0	\
	--pkg-export=librsvg-2.0	\
	--cflags-begin	\
	$(LIBRSVG_INCLUDES:/I=-I) -DRSVG_COMPILATION	\
	$(EXTRA_BASE_CFLAGS:/=-)	\
	--cflags-end	\
	--c-include=librsvg/rsvg.h	\
	--filelist=$(OUTDIR)\librsvg\Rsvg_2_0_gir_list	\
	-L.\$(OUTDIR) -L$(LIBDIR) -L$(BINDIR)	\
	-o $@

$(OUTDIR)\Rsvg-2.0.typelib: $(OUTDIR)\Rsvg-2.0.gir
	@-echo Compiling $@...
	$(G_IR_COMPILER)	\
	--includedir=. --includedir=$(G_IR_TYPELIBDIR) --debug --verbose	\
	$(@D:\=/)/$(@B).gir	\
	-o $@
!endif

clean:
	@-if exist Rsvg-$(RSVG_API_VER)\ rmdir /s/q Rsvg-$(RSVG_API_VER)
	@if exist $(OUTDIR)\Rsvg-$(RSVG_API_VER).typelib del /f /q $(OUTDIR)\Rsvg-$(RSVG_API_VER).typelib
	@if exist $(OUTDIR)\Rsvg-$(RSVG_API_VER).gir del /f /q $(OUTDIR)\Rsvg-$(RSVG_API_VER).gir
	@-del /f /q $(OUTDIR)\*.dll
	@-del /f /q $(OUTDIR)\*.exe
	@-del /f /q $(OUTDIR)\*.pdb
	@-del /f /q $(OUTDIR)\*.ilk
	@-del /f /q $(OUTDIR)\*.exp
	@-del /f /q $(OUTDIR)\*.lib
	@-del /s /q $(OUTDIR)\rsvg-tests\*.obj
	@-del /s /q $(OUTDIR)\rsvg-tests\*.pdb
	@-del /s /q $(OUTDIR)\rsvg-gdk-pixbuf-loader\*.obj
	@-del /s /q $(OUTDIR)\rsvg-gdk-pixbuf-loader\*.pdb
	@-del /s /q $(OUTDIR)\librsvg\Rsvg_2_0_gir_list
	@-del /s /q $(OUTDIR)\librsvg\*.obj
	@-del /s /q $(OUTDIR)\librsvg\*.pdb
	@-del /s /q $(OUTDIR)\librsvg\_rsvg_dummy.c
	@-del /s /q $(OUTDIR)\librsvg\config.h
	@-rmdir /s /q $(OUTDIR)\output
	@-rmdir /s /q output
	@-rmdir /s /q $(OUTDIR)\rsvg-tests
	@-rmdir /s /q $(OUTDIR)\rsvg-gdk-pixbuf-loader
	@-rmdir /s /q $(OUTDIR)\librsvg
	$(MAKE) /f rsvg-rust.mak CFG=$(CFG) cargo-clean
	@-rmdir /s /q $(OUTDIR)\obj
	@-del build-$(PLAT)-$(CFG)*.bat
