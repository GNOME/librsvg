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
	@for %%s in ($**) do echo %%s >> $@
