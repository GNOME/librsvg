#include <glib/gtypes.h>

double
rsvg_css_parse_length (const char *str, gdouble pixels_per_inch, int *fixed);

double
rsvg_css_parse_normalized_length(const char *str, gdouble pixels_per_inch,
				 gdouble normalize_to);

gboolean
rsvg_css_param_match (const char *str, const char *param_name);

int
rsvg_css_param_arg_offset (const char *str);

guint32
rsvg_css_parse_color (const char *str);

guint
rsvg_css_parse_opacity (const char *str);

double
rsvg_css_parse_fontsize (const char *str);
