#include "librsvg-features.h"

/* General initialization hooks */
const unsigned int librsvg_major_version=LIBRSVG_MAJOR_VERSION,
  librsvg_minor_version=LIBRSVG_MINOR_VERSION,
  librsvg_micro_version=LIBRSVG_MICRO_VERSION;

const char *librsvg_version = LIBRSVG_VERSION;

void
librsvg_preinit(void *app, void *modinfo)
{
}

void
librsvg_postinit(void *app, void *modinfo)
{
}
