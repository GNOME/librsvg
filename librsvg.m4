# Configure paths for LIBRSVG
# Raph Levien 98-11-18
# stolen from Manish Singh    98-9-30
# stolen back from Frank Belew
# stolen from Manish Singh
# Shamelessly stolen from Owen Taylor

dnl AM_PATH_LIBRSVG([MINIMUM-VERSION, [ACTION-IF-FOUND [, ACTION-IF-NOT-FOUND]]])
dnl Test for LIBRSVG, and define LIBRSVG_CFLAGS and LIBRSVG_LIBS
dnl
AC_DEFUN(AM_PATH_LIBRSVG,
[dnl 
dnl Get the cflags and libraries from the librsvg-config script
dnl
AC_ARG_WITH(librsvg-prefix,[  --with-librsvg-prefix=PFX   Prefix where LIBRSVG is installed (optional)],
            librsvg_prefix="$withval", librsvg_prefix="")
AC_ARG_WITH(librsvg-exec-prefix,[  --with-librsvg-exec-prefix=PFX Exec prefix where LIBRSVG is installed (optional)],
            librsvg_exec_prefix="$withval", librsvg_exec_prefix="")
AC_ARG_ENABLE(librsvgtest, [  --disable-librsvgtest       Do not try to compile and run a test LIBRSVG program],
		    , enable_librsvgtest=yes)

  if test x$librsvg_exec_prefix != x ; then
     librsvg_args="$librsvg_args --exec-prefix=$librsvg_exec_prefix"
     if test x${LIBRSVG_CONFIG+set} != xset ; then
        LIBRSVG_CONFIG=$librsvg_exec_prefix/bin/librsvg-config
     fi
  fi
  if test x$librsvg_prefix != x ; then
     librsvg_args="$librsvg_args --prefix=$librsvg_prefix"
     if test x${LIBRSVG_CONFIG+set} != xset ; then
        LIBRSVG_CONFIG=$librsvg_prefix/bin/librsvg-config
     fi
  fi

  AC_PATH_PROG(LIBRSVG_CONFIG, librsvg-config, no)
  min_librsvg_version=ifelse([$1], ,0.0.1,$1)
  AC_MSG_CHECKING(for LIBRSVG - version >= $min_librsvg_version)
  no_librsvg=""
  if test "$LIBRSVG_CONFIG" = "no" ; then
    no_librsvg=yes
  else
    LIBRSVG_CFLAGS=`$LIBRSVG_CONFIG $librsvgconf_args --cflags`
    LIBRSVG_LIBS=`$LIBRSVG_CONFIG $librsvgconf_args --libs`

    librsvg_major_version=`$LIBRSVG_CONFIG $librsvg_args --version | \
           sed 's/\([[0-9]]*\).\([[0-9]]*\).\([[0-9]]*\)/\1/'`
    librsvg_minor_version=`$LIBRSVG_CONFIG $librsvg_args --version | \
           sed 's/\([[0-9]]*\).\([[0-9]]*\).\([[0-9]]*\)/\2/'`
    librsvg_micro_version=`$LIBRSVG_CONFIG $librsvg_config_args --version | \
           sed 's/\([[0-9]]*\).\([[0-9]]*\).\([[0-9]]*\)/\3/'`
    if test "x$enable_librsvgtest" = "xyes" ; then
      ac_save_CFLAGS="$CFLAGS"
      ac_save_LIBS="$LIBS"
      CFLAGS="$CFLAGS $LIBRSVG_CFLAGS"
      LIBS="$LIBS $LIBRSVG_LIBS"
dnl
dnl Now check if the installed LIBRSVG is sufficiently new. (Also sanity
dnl checks the results of librsvg-config to some extent
dnl
      rm -f conf.librsvgtest
      AC_TRY_RUN([
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <librsvg_lgpl/librsvg.h>

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

  system ("touch conf.librsvgtest");

  /* HP/UX 9 (%@#!) writes to sscanf strings */
  tmp_version = my_strdup("$min_librsvg_version");
  if (sscanf(tmp_version, "%d.%d.%d", &major, &minor, &micro) != 3) {
     printf("%s, bad version string\n", "$min_librsvg_version");
     exit(1);
   }

   if (($librsvg_major_version > major) ||
      (($librsvg_major_version == major) && ($librsvg_minor_version > minor)) ||
      (($librsvg_major_version == major) && ($librsvg_minor_version == minor) && ($librsvg_micro_version >= micro)))
    {
      return 0;
    }
  else
    {
      printf("\n*** 'librsvg-config --version' returned %d.%d.%d, but the minimum version\n", $librsvg_major_version, $librsvg_minor_version, $librsvg_micro_version);
      printf("*** of LIBRSVG required is %d.%d.%d. If librsvg-config is correct, then it is\n", major, minor, micro);
      printf("*** best to upgrade to the required version.\n");
      printf("*** If librsvg-config was wrong, set the environment variable LIBRSVG_CONFIG\n");
      printf("*** to point to the correct copy of librsvg-config, and remove the file\n");
      printf("*** config.cache before re-running configure\n");
      return 1;
    }
}

],, no_librsvg=yes,[echo $ac_n "cross compiling; assumed OK... $ac_c"])
       CFLAGS="$ac_save_CFLAGS"
       LIBS="$ac_save_LIBS"
     fi
  fi
  if test "x$no_librsvg" = x ; then
     AC_MSG_RESULT(yes)
     ifelse([$2], , :, [$2])     
  else
     AC_MSG_RESULT(no)
     if test "$LIBRSVG_CONFIG" = "no" ; then
       echo "*** The librsvg-config script installed by LIBRSVG could not be found"
       echo "*** If LIBRSVG was installed in PREFIX, make sure PREFIX/bin is in"
       echo "*** your path, or set the LIBRSVG_CONFIG environment variable to the"
       echo "*** full path to librsvg-config."
     else
       if test -f conf.librsvgtest ; then
        :
       else
          echo "*** Could not run LIBRSVG test program, checking why..."
          CFLAGS="$CFLAGS $LIBRSVG_CFLAGS"
          LIBS="$LIBS $LIBRSVG_LIBS"
          AC_TRY_LINK([
#include <stdio.h>
#include <librsvg_lgpl/librsvg.h>
],      [ return 0; ],
        [ echo "*** The test program compiled, but did not run. This usually means"
          echo "*** that the run-time linker is not finding LIBRSVG or finding the wrong"
          echo "*** version of LIBRSVG. If it is not finding LIBRSVG, you'll need to set your"
          echo "*** LD_LIBRARY_PATH environment variable, or edit /etc/ld.so.conf to point"
          echo "*** to the installed location  Also, make sure you have run ldconfig if that"
          echo "*** is required on your system"
	  echo "***"
          echo "*** If you have an old version installed, it is best to remove it, although"
          echo "*** you may also be able to get things to work by modifying LD_LIBRARY_PATH"],
        [ echo "*** The test program failed to compile or link. See the file config.log for the"
          echo "*** exact error that occured. This usually means LIBRSVG was incorrectly installed"
          echo "*** or that you have moved LIBRSVG since it was installed. In the latter case, you"
          echo "*** may want to edit the librsvg-config script: $LIBRSVG_CONFIG" ])
          CFLAGS="$ac_save_CFLAGS"
          LIBS="$ac_save_LIBS"
       fi
     fi
     LIBRSVG_CFLAGS=""
     LIBRSVG_LIBS=""
     ifelse([$3], , :, [$3])
  fi
  AC_SUBST(LIBRSVG_CFLAGS)
  AC_SUBST(LIBRSVG_LIBS)
  rm -f conf.librsvgtest
])
