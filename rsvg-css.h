#include <glib/gtypes.h>
#include <pango/pangoft2.h>

double
rsvg_css_parse_length (const char *str, gdouble pixels_per_inch, 
		       gint *percent, gint *em, gint *ex);

double
rsvg_css_parse_normalized_length(const char *str, gdouble pixels_per_inch,
				 gdouble width_or_height, gdouble font_size,
				 gdouble x_height);

gboolean
rsvg_css_param_match (const char *str, const char *param_name);

int
rsvg_css_param_arg_offset (const char *str);

guint32
rsvg_css_parse_color (const char *str);

guint
rsvg_css_parse_opacity (const char *str);

double
rsvg_css_parse_angle (const char * str);

double
rsvg_css_parse_frequency (const char * str);

double
rsvg_css_parse_time (const char * str);

PangoStyle
rsvg_css_parse_font_style (const char * str, PangoStyle inherit);

PangoVariant
rsvg_css_parse_font_variant (const char * str, PangoVariant inherit);

PangoWeight
rsvg_css_parse_font_weight (const char * str, PangoWeight inherit);

PangoStretch
rsvg_css_parse_font_stretch (const char * str, PangoStretch inherit);

const char *
rsvg_css_parse_font_family (const char * str, const char * inherit);
