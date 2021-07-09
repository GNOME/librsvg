# NMake Makefile portion for code generation and
# intermediate build directory creation
# Items in here should not need to be edited unless
# one is maintaining the NMake build files.

# Generate listing file for introspection
$(OUTDIR)\librsvg\Rsvg_2_0_gir_list: $(librsvg_real_pub_HDRS)
	@if not exist $(@D)\ mkdir $(@D)
	@if exist $@ del $@
	@for %%s in ($**) do @echo %%s >> $@
