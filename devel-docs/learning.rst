Learning resources
==================

Like any other part of the web platform, SVG is a bit fractally
complex.  There is the SVG format itself, its drawing model, text
layout, font rendering, image compositing, and a bunch of fascinating
topics.  This chapter has links to various places where you can learn
about these topics.


The SVG format
--------------

`SVG Tutorial as an advent calendar <https://svg-tutorial.com/>`__ —
24 short lessons to learn basic SVG features.

`Blind SVG <https://blindsvg.com/>`__ — This is the absolute best
guide I have found for learning the SVG format gradually.  It is
designed so that blind and low-vision people can learn to write their
own illustrations using SVG, but it is useful for everyone!

`SVG tutorial at Mozilla Developer's Network
<https://developer.mozilla.org/en-US/docs/Web/SVG/Tutorial>`__ —
Detailed and friendly.

`Codepen <https://codepen.io/pen/>`__ — lets you paste SVG code in its
usual HTML editor and see it rendered immediately.  You can also add a
CSS stylesheet to experiment with styles.


The text rendering pipeline
---------------------------

`The journey of a word: how text ends up on a page
<https://www.youtube.com/watch?v=Is4PW6f4Pk4>`__ — talk by Simon
Cozens.  Talks about peculiarities of different language families,
their writing systems, complex text shaping, the OpenType font
formats, and how Harfbuzz works.  **You should absolutely watch this talk!**

`How Unicode Characters Become Glyphs on Your Screen
<https://www.youtube.com/watch?v=bt4MwIpcp2M>`__ — similar in spirit
to the talk above, but a bit more detailed.  Talks about typography
terminology, text segmentation, OpenType font features, and text
layout in some detail.

`The rendering pipeline in Pango
<https://docs.gtk.org/Pango/pango_rendering.html>`__ — Pango is
GNOME's text layout engine, which librsvg also uses.  This is a page
from Pango's documentation, with a high-level overview of the
pipeline.  You can then research individual terms like *itemization*,
*shaping*, etc.

`Pango, an open-source Unicode text layout engine
<https://people.redhat.com/otaylor/iuc25/pango-unicode-paper.pdf>`__
by Owen Taylor, original author of Pango.  This paper is a bit old,
but provides a good overview of what Pango does.  See also one of the
first papers about it, `Pango: internationalized text handling
<https://web.archive.org/web/20120227064838/http://ols.fedoraproject.org/OLS/Reprints-2001/taylor.pdf>`__.

`Complex text layout <https://en.wikipedia.org/wiki/Complex_text_layout>`__ on Wikipedia

`Text directionality
<https://learn.microsoft.com/en-us/globalization/fonts-layout/text-directionality>`__
— describes how different writing systems use different
directionalities, how logical order differs from visual order, and the
Unicode directional formatting characters.

`Ten years of Harfbuzz
<https://www.youtube.com/watch?v=T79LMEXkf9w>`__ by Behdad
Esfahbod, the maintainer of Harfbuzz, which provides the text shaping
engine for librsvg and GNOME.  This talk is mostly history, as it name implies.

`State of Text Rendering 2024 <https://behdad.org/text2024/>`__ — This
is a big document; read it casually.  It describes all the parts, all
the things, all the people, all the projects.


General knowledge
-----------------

`Why are 2D vector graphics so much harder than 3D?
<https://blog.mecheye.net/2019/05/why-is-2d-graphics-is-harder-than-3d-graphics/>`__
— a quick history of 2D graphics with lots of links for you to dive
into history.  The other articles in that blog are incredibly good, by
the way.

`Porter/Duff Compositing and Blend Modes
<https://ssp.impulsetrain.com/porterduff.html>`__ — how the alpha
channel works, the Porter/Duff compositing algebra and operators.
