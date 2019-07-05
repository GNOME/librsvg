# NMake Makefile portion for code generation and
# intermediate build directory creation
# Items in here should not need to be edited unless
# one is maintaining the NMake build files.

# Copy the pre-defined config.h.win32
$(OUTDIR)\librsvg\config.h: config.h.win32
	@if not exist $(@D) $(MAKE) /f Makefile.vc CFG=$(CFG) $(@D)
	@-copy $** $@

# Create the build directories
$(OUTDIR)\librsvg			\
$(OUTDIR)\rsvg-gdk-pixbuf-loader	\
$(OUTDIR)\rsvg-tools			\
$(OUTDIR)\rsvg-tests:
	@-mkdir $@

# Generate listing file for introspection
$(OUTDIR)\librsvg\Rsvg_2_0_gir_list:	\
$(librsvg_real_pub_HDRS)		\
$(librsvg_real_extra_pub_HDRS)		\
$(librsvg_real_SRCS)
	@if exist $@ del $@
	@for %%s in ($(librsvg_real_pub_HDRS) $(librsvg_real_extra_pub_HDRS)) do echo %%s >> $@
	@for %%s in ($(librsvg_real_SRCS)) do @if "%%~xs" == ".c" echo %%s >> $@
