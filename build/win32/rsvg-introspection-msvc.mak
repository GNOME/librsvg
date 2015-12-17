# NMake Makefile to build Introspection Files for GDK-Pixbuf

!include detectenv-msvc.mak

APIVERSION = 2.0

CHECK_PACKAGE = gdk-pixbuf-2.0

!include introspection-msvc.mak

!if "$(BUILD_INTROSPECTION)" == "TRUE"
all: setgirbuildenv Rsvg-$(APIVERSION).gir Rsvg-$(APIVERSION).typelib

setgirbuildenv:
	@-set PYTHONPATH=$(PREFIX)\lib\gobject-introspection
	@-set PATH=vs$(VSVER)\$(CFG)\$(PLAT)\bin;$(PREFIX)\bin;$(PATH)
	@-set PKG_CONFIG_PATH=$(PKG_CONFIG_PATH)
	@-set LIB=vs$(VSVER)\$(CFG)\$(PLAT)\bin;$(LIB)

!include introspection.body.mak

install-introspection: all
	@-copy Rsvg-$(APIVERSION).gir $(G_IR_INCLUDEDIR)
	@-copy /b Rsvg-$(APIVERSION).typelib $(G_IR_TYPELIBDIR)

!else
all:
	@-echo $(ERROR_MSG)

install-introspection: all
!endif

clean:
	@-del /f/q Rsvg-$(APIVERSION).typelib
	@-del /f/q Rsvg-$(APIVERSION).gir
