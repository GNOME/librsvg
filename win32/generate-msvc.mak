# NMake Makefile portion for code generation and
# intermediate build directory creation
# Items in here should not need to be edited unless
# one is maintaining the NMake build files.

$(OUTDIR)\librsvg\_rsvg_dummy.c:
	@echo Generating dummy source file...
	@if not exist $(@D)\ mkdir $(@D) 
	echo static int __rsvg_dummy; > $@

$(OUTDIR)\librsvg\librsvg.def: .\librsvg.symbols
	@echo Generating $@...
	@if not exist $(@D)\ mkdir $(@D) 
	@echo EXPORTS>$@
	$(CC) /EP $**>>$@

# Generate listing file for introspection
$(OUTDIR)\librsvg\Rsvg_2_0_gir_list: $(librsvg_real_pub_HDRS)
	@if exist $@ del $@
	@for %%s in ($**) do @echo %%s >> $@

# Generate documentation (introspection must be built)
!ifdef INTROSPECTION
generate-docs: ..\doc\librsvg.toml $(OUTDIR)\Rsvg-$(RSVG_API_VER).gir
	@echo Generating documentation...
	@$(GI_DOCGEN) generate -C $** --content-dir=..\doc	\
	--add-include-path=$(G_IR_INCLUDEDIR)
!else
generate-docs:
	@echo Introspection must be enabled to build documentation
!endif

# Generate NMake Makefiles (for git checkouts only)

!ifndef IS_NOT_GIT
config.h.win32: ..\.git ..\configure.ac prebuild.py config.h.win32.in
config-msvc.mak: ..\.git ..\configure.ac prebuild.py config-msvc.mak.in
..\include\librsvg\rsvg-version.h: ..\.git ..\configure.ac prebuild.py ..\include\librsvg\rsvg-version.h.in

generate-nmake-files: config.h.win32 config-msvc.mak ..\include\librsvg\rsvg-version.h
	@if not "$(PYTHON)" == "" $(PYTHON) prebuild.py
	@if "$(PYTHON)" == "" echo You need to specify your Python interpreter PATH by passing in PYTHON^=^<full_path_to_python_interpreter^>

remove-generated-nmake-files: ..\.git
	@-del /f/q config-msvc.mak
	@-del /f/q config.h.win32
	@-del /f/q ..\include\librsvg\rsvg-version.h
	@-for /f %%d in ('dir /ad /b vs*') do @rmdir /s/q %%d
!endif
