# Configuration stuff for using the Python interpreter

# Either having python.exe your PATH will work or passing in
# PYTHON=<full path to your Python interpretor> will do. Be aware
# in Windows 10/11 installations where there may be a stub Python
# executable in $(LOCALAPPDATA)\Microsoft\WindowsApps if you did
# not install Python via the Microsoft Store, if PYTHON is not
# specified

!ifndef PYTHON
DEFAULT_PYTHON=1
PYTHON=python
APPSTORE_PYTHON=$(LOCALAPPDATA)\Microsoft\WindowsApps\python.exe
!endif

!ifdef DEFAULT_PYTHON
!if [if exist $(APPSTORE_PYTHON) echo WARN_PYTHON=1 >python-check.mak]
!endif
!if [if not exist $(APPSTORE_PYTHON) echo WARN_PYTHON=0 >python-check.mak]
!endif
!include python-check.mak
!if [del /f/q python-check.mak]
!endif
!endif
!if defined(WARN_PYTHON) && "$(WARN_PYTHON)" == "1"
!ifndef IS_NOT_GIT
warn-appstore-python:
	@echo You may be using a stub Python executable if you have not installed
	@echo Python from the Microsoft Store. If running NMake using Python fails
	@echo either with "nmake /f generate-msvc.mak generate-nmake-files" with no
	@echo config.h.win32 nor config-msvc.mak generated, or building librsvg
	@echo with "INTROSPECTION=1" passed in , try again passing
	@echo PYTHON=^<location_of_your_actual_python_executable^> in the NMake
	@echo command line. If running nmake Makefile.vc ... install, librsvg-2.0.pc
	@echo may not have been generated, as well.
	@echo.
!else
warn-appstore-python:
	@echo You may be using a stub Python executable if you have not installed
	@echo Python from the Microsoft Store. If running NMake using Python fails
	@echo when building librsvg with "INTROSPECTION=1" passed in , try again passing
	@echo PYTHON=^<location_of_your_actual_python_executable^> in the NMake
	@echo command line. If running nmake Makefile.vc ... install, librsvg-2.0.pc
	@echo may not have been generated, as well.
	@echo.
!endif
!else
warn-appstore-python:
!endif
