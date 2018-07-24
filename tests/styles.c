/* vim: set ts=4 nowrap ai expandtab sw=4: */

#include <glib.h>
#include "librsvg/rsvg.h"
#include "librsvg/rsvg-private.h"
#include "librsvg/rsvg-defs.h"
#include "librsvg/rsvg-styles.h"
#include "test-utils.h"

union Expected {
    guint color;
    Length length;
};

typedef struct _FixtureData
{
    const gchar *test_name;
    const gchar *bug_id;
    const gchar *file_path;
    const gchar *id;
    const gchar *target_name;
    union Expected expected;
} FixtureData;

static void
assert_equal_color (guint expected, guint actual)
{
    g_assert_cmphex (expected, ==, actual);
}

static void
assert_equal_length (Length *expected, Length *actual)
{
    g_assert_cmpfloat (expected->length, ==, actual->length);
    g_assert_cmpint (expected->unit, ==, actual->unit);
}

static void
assert_equal_value (FixtureData *fixture, RsvgNode *node)
{
    if (g_str_equal (fixture->target_name, "stroke"))
	assert_equal_color (fixture->expected.color, rsvg_node_get_state (node)->stroke->core.color->argb);
    else if (g_str_equal (fixture->target_name, "fill"))
        assert_equal_color (fixture->expected.color, rsvg_node_get_state (node)->fill->core.color->argb);
    else if (g_str_equal (fixture->target_name, "stroke-width"))
        assert_equal_length (&fixture->expected.length, &rsvg_node_get_state (node)->stroke_width);
    else
        g_assert_not_reached ();
}

static void
test_value (FixtureData *fixture)
{
    RsvgHandle *handle;
    RsvgNode *node;
    gchar *target_file;
    GError *error = NULL;

    if (fixture->bug_id)
        g_test_bug (fixture->bug_id);

    target_file = g_build_filename (test_utils_get_test_data_path (),
                                    fixture->file_path, NULL);
    handle = rsvg_handle_new_from_file (target_file, &error);
    g_free (target_file);

    node = rsvg_defs_lookup (handle->priv->defs, fixture->id);
    g_assert (node);
    g_assert (rsvg_node_get_state (node));

    assert_equal_value (fixture, node);

    g_object_unref (handle);
}

#define POINTS_PER_INCH (72.0)
#define POINTS_LENGTH(x) ((x) / POINTS_PER_INCH)

static const FixtureData fixtures[] =
{
    {"/styles/selectors/type", NULL, "styles/order.svg", "#black", "fill", .expected.color = 0xff000000},
    {"/styles/selectors/class", NULL, "styles/order.svg", "#blue", "fill", .expected.color = 0xff0000ff},
    {"/styles/selectors/#id", NULL, "styles/order.svg", "#brown", "fill", .expected.color = 0xffa52a2a},
    {"/styles/selectors/style", NULL, "styles/order.svg", "#gray", "fill", .expected.color = 0xff808080},
    {"/styles/selectors/style property prior than class", NULL, "styles/order.svg", "#red", "fill", .expected.color = 0xffff0000},
    {"/styles/selectors/#id prior than class", NULL, "styles/order.svg", "#green", "fill", .expected.color = 0xff008000},
    {"/styles/selectors/type#id prior than class", NULL, "styles/order.svg", "#pink", "fill", .expected.color = 0xffffc0cb},
    {"/styles/selectors/class#id prior than class", NULL, "styles/order.svg", "#yellow", "fill", .expected.color = 0xffffff00},
    {"/styles/selectors/type.class#id prior than class", NULL, "styles/order.svg", "#white", "fill", .expected.color = 0xffffffff},
    {"/styles/selectors/#id prior than type", "418823", "styles/bug418823.svg", "#bla", "fill", .expected.color = 0xff00ff00},
    {"/styles/selectors/comma-separate (fill)", "614643", "styles/bug614643.svg", "#red-rect", "fill", .expected.color = 0xffff0000},
    {"/styles/selectors/comma-separete (stroke)", "614643", "styles/bug614643.svg", "#red-path", "stroke", .expected.color = 0xffff0000},
    {"/styles/override presentation attribute", "614704", "styles/bug614704.svg", "#blue-rect", "fill", .expected.color = 0xff0000ff},
    {"/styles/selectors/2 or more selectors (fill)", "592207", "styles/bug592207.svg", "#target", "fill", .expected.color = 0xffff0000},
    {"/styles/selectors/2 or more selectors (stroke)", "592207", "styles/bug592207.svg", "#target", "stroke", .expected.color = 0xff0000ff},
    {"/styles/svg-element-style", "615701", "styles/svg-class.svg", "#svg", "fill", .expected.color = 0xff0000ff},
    {"/styles/presentation attribute in svg element", "620693", "styles/bug620693.svg", "#svg", "stroke", .expected.color = 0xffff0000},
    {"/styles/!important/stroke", "379629", "styles/bug379629.svg", "#base_shadow", "stroke", .expected.color = 0xffffc0cb /* pink */},
    {"/styles/!important/stroke-width", "379629", "styles/bug379629.svg", "#base_shadow", "stroke-width", .expected.length = {POINTS_LENGTH(5.), LENGTH_UNIT_INCH}},
    {"/styles/!important/class", "614606", "styles/bug614606.svg", "#path6306", "fill", .expected.color = 0xffff0000 /* red */ },
    {"/styles/!important/element", "614606", "styles/bug614606.svg", "#path6308", "fill", .expected.color = 0xff000000},
    {"/styles/!important/#id prior than class", NULL, "styles/important.svg", "#red", "fill", .expected.color = 0xffff0000 },
    {"/styles/!important/class prior than type", NULL, "styles/important.svg", "#blue", "fill", .expected.color = 0xff0000ff },
    {"/styles/!important/presentation attribute is invalid", NULL, "styles/important.svg", "#white", "fill", .expected.color = 0xffffffff },
    {"/styles/!important/style prior than class", NULL, "styles/important.svg", "#pink", "fill", .expected.color = 0xffffc0cb },
    /* {"/styles/selectors/descendant", "338160", "styles/bug338160.svg", "#base_shadow", "stroke-width", .expected.length = {2., LENGTH_UNIT_DEFAULT}}, */
};
static const gint n_fixtures = G_N_ELEMENTS (fixtures);

int
main (int argc, char *argv[])
{
    gint i;
    int result;

    g_test_init (&argc, &argv, NULL);

    for (i = 0; i < n_fixtures; i++)
        g_test_add_data_func (fixtures[i].test_name, &fixtures[i], (void*)test_value);

    result = g_test_run ();

    rsvg_cleanup ();

    return result;
}
