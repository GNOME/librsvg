dnl NAUTILUS_PATH_FREETYPE2([MINIMUM-VERSION, [ACTION-IF-FOUND [, ACTION-IF-NOT-FOUND]]])
dnl Test for FreeType2, and define FREETYPE2_CFLAGS and FREETYPE2_LIBS
dnl
dnl Shamelessly cut-n-pasted from AM_PATH_LIBART
dnl
AC_DEFUN(NAUTILUS_PATH_FREETYPE2,
[dnl 
dnl Get the cflags and libraries from the freetype-config script
dnl
AC_ARG_WITH(freetype2-prefix,[  --with-freetype2-prefix=PFX   Prefix where FREETYPE2 is installed (optional)],
            freetype2_prefix="$withval", freetype2_prefix="")
AC_ARG_WITH(freetype2-exec-prefix,[  --with-freetype2-exec-prefix=PFX Exec prefix where FREETYPE2 is installed (optional)],
            freetype2_exec_prefix="$withval", freetype2_exec_prefix="")
AC_ARG_ENABLE(freetype2test, [  --disable-freetype2test       Do not try to compile and run a test FREETYPE2 program],
		    , enable_freetype2test=yes)

  if test x$freetype2_exec_prefix != x ; then
     freetype2_args="$freetype2_args --exec-prefix=$freetype2_exec_prefix"
     if test x${FREETYPE2_CONFIG+set} != xset ; then
        FREETYPE2_CONFIG=$freetype2_exec_prefix/bin/freetype-config
     fi
  fi
  if test x$freetype2_prefix != x ; then
     freetype2_args="$freetype2_args --prefix=$freetype2_prefix"
     if test x${FREETYPE2_CONFIG+set} != xset ; then
        FREETYPE2_CONFIG=$freetype2_prefix/bin/freetype-config
     fi
  fi

  AC_PATH_PROG(FREETYPE2_CONFIG, freetype-config, no)
  min_freetype2_version=ifelse([$1], ,0.2.5,$1)
  AC_MSG_CHECKING(for FREETYPE2 - version >= $min_freetype2_version)
  no_freetype2=""
  if test "$FREETYPE2_CONFIG" = "no" ; then
    no_freetype2=yes
  else
    FREETYPE2_CFLAGS=`$FREETYPE2_CONFIG $freetype2conf_args --cflags`
    FREETYPE2_LIBS=`$FREETYPE2_CONFIG $freetype2conf_args --libs`

    freetype2_major_version=`$FREETYPE2_CONFIG $freetype2_args --version | \
	sed 's/\([[0-9]]*\)[[:.]]\([[0-9]]*\)[[:.]]\([[0-9]]*\)/\1/'`
    freetype2_minor_version=`$FREETYPE2_CONFIG $freetype2_args --version | \
	sed 's/\([[0-9]]*\)[[:.]]\([[0-9]]*\)[[:.]]\([[0-9]]*\)/\2/'`
    freetype2_micro_version=`$FREETYPE2_CONFIG $freetype2_args --version | \
	sed 's/\([[0-9]]*\)[[:.]]\([[0-9]]*\)[[:.]]\([[0-9]]*\)/\3/'`

    if test "x$enable_freetype2test" = "xyes" ; then
      ac_save_CFLAGS="$CFLAGS"
      ac_save_LIBS="$LIBS"
      CFLAGS="$CFLAGS $FREETYPE2_CFLAGS"
      LIBS="$LIBS $FREETYPE2_LIBS"
dnl
dnl Now check if the installed FREETYPE2 is sufficiently new. (Also sanity
dnl checks the results of freetype-config to some extent
dnl
      rm -f conf.freetype2test
      AC_TRY_RUN([
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <freetype/freetype.h>

char*
my_strdup (char *str)
{
  char *new_str;
  
  if (str)
    {
      new_str = malloc ((strlen (str) + 1) * sizeof(char));
      strcpy (new_str, str);
    }
  else
    new_str = NULL;
  
  return new_str;
}

int main ()
{
  int major, minor, micro;
  char *tmp_version;

  system ("touch conf.freetype2test");

  /* HP/UX 9 (%@#!) writes to sscanf strings */
  tmp_version = my_strdup("$min_freetype2_version");
  if (sscanf(tmp_version, "%d.%d.%d", &major, &minor, &micro) != 3) {
     printf("%s, bad version string\n", "$min_freetype2_version");
     exit(1);
   }

   if (($freetype2_major_version > major) ||
      (($freetype2_major_version == major) && ($freetype2_minor_version > minor)) ||
      (($freetype2_major_version == major) && ($freetype2_minor_version == minor) && ($freetype2_micro_version >= micro)))
    {
      return 0;
    }
  else
    {
      printf("\n");
      printf("*** \n");
      printf("*** 'freetype-config --version' returned %d.%d.%d, but the minimum version\n", $freetype2_major_version, $freetype2_minor_version, $freetype2_micro_version);
      printf("*** of FREETYPE2 required is %d.%d.%d. If freetype-config is correct, then it is\n", major, minor, micro);
      printf("*** best to upgrade to the required version.\n");
      printf("*** If freetype-config was wrong, set the environment variable FREETYPE2_CONFIG\n");
      printf("*** to point to the correct copy of freetype-config, and remove the file\n");
      printf("*** config.cache before re-running configure\n");
      printf("*** \n");

      return 1;
    }
}

],, no_freetype2=yes,[echo $ac_n "cross compiling; assumed OK... $ac_c"])
       CFLAGS="$ac_save_CFLAGS"
       LIBS="$ac_save_LIBS"
     fi
  fi
  if test "x$no_freetype2" = x ; then
     AC_MSG_RESULT(yes)
     ifelse([$2], , :, [$2])     
  else
     AC_MSG_RESULT(no)
     if test "$FREETYPE2_CONFIG" = "no" ; then
       echo "*** The freetype-config script installed by FREETYPE2 could not be found"
       echo "*** If FREETYPE2 was installed in PREFIX, make sure PREFIX/bin is in"
       echo "*** your path, or set the FREETYPE2_CONFIG environment variable to the"
       echo "*** full path to freetype-config."
     else
       if test -f conf.freetype2test ; then
        :
       else
          echo "*** Could not run FREETYPE2 test program, checking why..."
          CFLAGS="$CFLAGS $FREETYPE2_CFLAGS"
          LIBS="$LIBS $FREETYPE2_LIBS"
          AC_TRY_LINK([
#include <stdio.h>
#include <freetype/freetype.h>
],      [ return 0; ],
        [ echo "*** The test program compiled, but did not run. This usually means"
          echo "*** that the run-time linker is not finding FREETYPE2 or finding the wrong"
          echo "*** version of FREETYPE2. If it is not finding FREETYPE2, you'll need to set your"
          echo "*** LD_LIBRARY_PATH environment variable, or edit /etc/ld.so.conf to point"
          echo "*** to the installed location  Also, make sure you have run ldconfig if that"
          echo "*** is required on your system"
	  echo "***"
          echo "*** If you have an old version installed, it is best to remove it, although"
          echo "*** you may also be able to get things to work by modifying LD_LIBRARY_PATH"],
        [ echo "*** The test program failed to compile or link. See the file config.log for the"
          echo "*** exact error that occured. This usually means FREETYPE2 was incorrectly installed"
          echo "*** or that you have moved FREETYPE2 since it was installed. In the latter case, you"
          echo "*** may want to edit the freetype-config script: $FREETYPE2_CONFIG" ])
          CFLAGS="$ac_save_CFLAGS"
          LIBS="$ac_save_LIBS"
       fi
     fi
     FREETYPE2_CFLAGS=""
     FREETYPE2_LIBS=""
     ifelse([$3], , :, [$3])
  fi
  AC_SUBST(FREETYPE2_CFLAGS)
  AC_SUBST(FREETYPE2_LIBS)
  rm -f conf.freetype2test
])
