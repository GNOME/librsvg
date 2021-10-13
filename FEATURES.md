# SVG and CSS features that librsvg supports

## Attributes supported by all elements

| Attribute | Notes                                                                               |
| ---       | ---                                                                                 |
| transform | The `transform` attribute has a different syntax than the CSS `transform` property. |
|           |                                                                                     |
| xml:lang  |                                                                                     |
|           |                                                                                     |
| xml:space |                                                                                     |

## Elements and their specific attributes

FIXME: add global attributes parsed in element.rs, not by specific element implementations

cond

| Element             | Attributes          | Notes                                                              |
| ---                 | ---                 | ---                                                                |
| a                   |                     |                                                                    |
|                     | xlink:href          | Needs xlink namespace                                              |
|                     | href                | SVG2                                                               |
|                     |                     |                                                                    |
| circle              |                     |                                                                    |
|                     | cx                  |                                                                    |
|                     | cy                  |                                                                    |
|                     | r                   |                                                                    |
|                     |                     |                                                                    |
| clipPath            |                     |                                                                    |
|                     | clipPathUnits       |                                                                    |
|                     |                     |                                                                    |
| defs                |                     |                                                                    |
|                     |                     |                                                                    |
| ellipse             |                     |                                                                    |
|                     | cx                  |                                                                    |
|                     | cy                  |                                                                    |
|                     | rx                  |                                                                    |
|                     | ry                  |                                                                    |
|                     |                     |                                                                    |
| feBlend             |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | in2                 |                                                                    |
|                     | mode                |                                                                    |
|                     |                     |                                                                    |
| feColorMatrix       |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | type                |                                                                    |
|                     | values              |                                                                    |
|                     |                     |                                                                    |
| feComponentTransfer |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     |                     |                                                                    |
| feComposite         |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | in2                 |                                                                    |
|                     | operator            |                                                                    |
|                     | k1                  |                                                                    |
|                     | k2                  |                                                                    |
|                     | k3                  |                                                                    |
|                     | k4                  |                                                                    |
|                     |                     |                                                                    |
| feConvolveMatrix    |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | order               |                                                                    |
|                     | divisor             |                                                                    |
|                     | bias                |                                                                    |
|                     | targetX             |                                                                    |
|                     | targetY             |                                                                    |
|                     | edgeMode            |                                                                    |
|                     | kernelMatrix        |                                                                    |
|                     | kernelUnitLength    |                                                                    |
|                     | preserveAlpha       |                                                                    |
|                     |                     |                                                                    |
| feDiffuseLighting   |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | surfaceScale        |                                                                    |
|                     | kernelUnitLength    |                                                                    |
|                     | diffuseConstant     |                                                                    |
|                     |                     |                                                                    |
| feDisplacementMap   |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | in2                 |                                                                    |
|                     | scale               |                                                                    |
|                     | xChannelSelector    |                                                                    |
|                     | yChannelSelector    |                                                                    |
|                     |                     |                                                                    |
| feDistantLight      |                     |                                                                    |
|                     | azimuth             |                                                                    |
|                     | elevation           |                                                                    |
|                     |                     |                                                                    |
| feFuncA             |                     | See "Filter effect feComponentTransfer"                            |
|                     |                     |                                                                    |
| feFuncB             |                     | See "Filter effect feComponentTransfer"                            |
|                     |                     |                                                                    |
| feFuncG             |                     | See "Filter effect feComponentTransfer"                            |
|                     |                     |                                                                    |
| feFuncR             |                     | See "Filter effect feComponentTransfer"                            |
|                     |                     |                                                                    |
| feFlood             |                     | See "Filter effects"                                               |
|                     |                     | Parameters come from the flood-color and flood-opacity properties. |
|                     |                     |                                                                    |
| feGaussianBlur      |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | stdDeviation        |                                                                    |
|                     |                     |                                                                    |
| feImage             |                     | See "Filter effects"                                               |
|                     | xlink:href          | Needs xlink namespace                                              |
|                     | href                | SVG2                                                               |
|                     | path                | Non-standard; used by old Adobe Illustrator versions.              |
|                     | preserveAspectRatio |                                                                    |
|                     |                     |                                                                    |
| feMerge             |                     | See "Filter effects"                                               |
|                     |                     |                                                                    |
| feMergeNode         |                     |                                                                    |
|                     | in                  |                                                                    |
|                     |                     |                                                                    |
| feMorphology        |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | operator            |                                                                    |
|                     | radius              |                                                                    |
|                     |                     |                                                                    |
| feOffset            |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | dx                  |                                                                    |
|                     | dy                  |                                                                    |
|                     |                     |                                                                    |
| fePointLight        |                     |                                                                    |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | z                   |                                                                    |
|                     |                     |                                                                    |
| feSpecularLighting  |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     | surfaceScale        |                                                                    |
|                     | kernelUnitLength    |                                                                    |
|                     | specularConstant    |                                                                    |
|                     | specularExponent    |                                                                    |
|                     |                     |                                                                    |
| feSpotLight         |                     |                                                                    |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | z                   |                                                                    |
|                     | pointsAtX           |                                                                    |
|                     | pointsAtY           |                                                                    |
|                     | pointsAtZ           |                                                                    |
|                     | specularExponent    |                                                                    |
|                     | limitingConeAngle   |                                                                    |
|                     |                     |                                                                    |
| feTile              |                     | See "Filter effects"                                               |
|                     | in                  |                                                                    |
|                     |                     |                                                                    |
| feTurbulence        |                     | See "Filter effects"                                               |
|                     | baseFrequency       |                                                                    |
|                     | numOctaves          |                                                                    |
|                     | seed                |                                                                    |
|                     | stitchTiles         |                                                                    |
|                     | type                |                                                                    |
|                     |                     |                                                                    |
| filter              |                     |                                                                    |
|                     | filterUnits         |                                                                    |
|                     | primitiveUnits      |                                                                    |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | width               |                                                                    |
|                     | height              |                                                                    |
|                     |                     |                                                                    |
| g                   |                     |                                                                    |
|                     |                     |                                                                    |
| image               |                     |                                                                    |
|                     | xlink:href          | Needs xlink namespace                                              |
|                     | href                | SVG2                                                               |
|                     | path                | Non-standard; used by old Adobe Illustrator versions.              |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | width               |                                                                    |
|                     | height              |                                                                    |
|                     | preserveAspectRatio |                                                                    |
|                     |                     |                                                                    |
|                     |                     |                                                                    |
| line                |                     |                                                                    |
|                     | x1                  |                                                                    |
|                     | y1                  |                                                                    |
|                     | x2                  |                                                                    |
|                     | y2                  |                                                                    |
|                     |                     |                                                                    |
| linearGradient      |                     |                                                                    |
|                     | gradientUnits       |                                                                    |
|                     | gradientTransform   |                                                                    |
|                     | spreadMethod        |                                                                    |
|                     | x1                  |                                                                    |
|                     | y1                  |                                                                    |
|                     | x2                  |                                                                    |
|                     | y2                  |                                                                    |
|                     |                     |                                                                    |
| marker              |                     |                                                                    |
|                     | markerUnits         |                                                                    |
|                     | refX                |                                                                    |
|                     | refY                |                                                                    |
|                     | markerWidth         |                                                                    |
|                     | markerHeight        |                                                                    |
|                     | orient              |                                                                    |
|                     | preserveAspectRatio |                                                                    |
|                     | viewBox             |                                                                    |
|                     |                     |                                                                    |
| mask                |                     |                                                                    |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | width               |                                                                    |
|                     | height              |                                                                    |
|                     | maskUnits           |                                                                    |
|                     | maskContentUnits    |                                                                    |
|                     |                     |                                                                    |
| path                |                     |                                                                    |
|                     | d                   |                                                                    |
|                     |                     |                                                                    |
| pattern             |                     |                                                                    |
|                     | xlink:href          | Needs xlink namespace                                              |
|                     | href                | SVG2                                                               |
|                     | patternUnits        |                                                                    |
|                     | patternContentUnits |                                                                    |
|                     | patternTransform    |                                                                    |
|                     | preserveAspectRatio |                                                                    |
|                     | viewBox             |                                                                    |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | width               |                                                                    |
|                     | height              |                                                                    |
|                     |                     |                                                                    |
| polygon             |                     |                                                                    |
|                     | points              |                                                                    |
|                     |                     |                                                                    |
| polyline            |                     |                                                                    |
|                     | points              |                                                                    |
|                     |                     |                                                                    |
| radialGradient      |                     |                                                                    |
|                     | gradientUnits       |                                                                    |
|                     | gradientTransform   |                                                                    |
|                     | spreadMethod        |                                                                    |
|                     | cx                  |                                                                    |
|                     | cy                  |                                                                    |
|                     | r                   |                                                                    |
|                     | fx                  |                                                                    |
|                     | fx                  |                                                                    |
|                     | fr                  |                                                                    |
|                     |                     |                                                                    |
| rect                |                     |                                                                    |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | width               |                                                                    |
|                     | height              |                                                                    |
|                     | rx                  |                                                                    |
|                     | ry                  |                                                                    |
|                     |                     |                                                                    |
| stop                |                     |                                                                    |
|                     | offset              |                                                                    |
|                     |                     |                                                                    |
| style               |                     |                                                                    |
|                     | type                |                                                                    |
|                     |                     |                                                                    |
| svg                 |                     |                                                                    |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | width               |                                                                    |
|                     | height              |                                                                    |
|                     | viewBox             |                                                                    |
|                     | preserveAspectRatio |                                                                    |
|                     |                     |                                                                    |
| switch              |                     |                                                                    |
|                     |                     |                                                                    |
| symbol              |                     |                                                                    |
|                     | preserveAspectRatio |                                                                    |
|                     | viewBox             |                                                                    |
|                     |                     |                                                                    |
| text                |                     |                                                                    |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | dx                  |                                                                    |
|                     | dy                  |                                                                    |
|                     |                     |                                                                    |
| tref                |                     |                                                                    |
|                     | xlink:href          | Needs xlink namespace                                              |
|                     |                     |                                                                    |
| tspan               |                     |                                                                    |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | dx                  |                                                                    |
|                     | dy                  |                                                                    |
|                     |                     |                                                                    |
| use                 |                     |                                                                    |
|                     | xlink:href          | Needs xlink namespace                                              |
|                     | href                | SVG2                                                               |
|                     | x                   |                                                                    |
|                     | y                   |                                                                    |
|                     | width               |                                                                    |
|                     | height              |                                                                    |

## CSS properties

Shorthands:

| Property | Notes |
| ---      | ---   |
| font     |       |
| marker   |       |

Longhands:

| Property                    | Notes                                                  |
| ---                         | ---                                                    |
| baseline-shift              |                                                        |
| clip-path                   |                                                        |
| clip-rule                   |                                                        |
| color                       |                                                        |
| color-interpolation-filters |                                                        |
| direction                   |                                                        |
| display                     |                                                        |
| enable-background           |                                                        |
| fill                        |                                                        |
| fill-opacity                |                                                        |
| fill-rule                   |                                                        |
| filter                      |                                                        |
| flood-color                 |                                                        |
| flood-opacity               |                                                        |
| font-family                 |                                                        |
| font-size                   |                                                        |
| font-stretch                |                                                        |
| font-style                  |                                                        |
| font-variant                |                                                        |
| font-weight                 |                                                        |
| letter-spacing              |                                                        |
| lighting-color              |                                                        |
| line-height                 | Not available as a presentation attribute.             |
| marker-end                  |                                                        |
| marker-mid                  |                                                        |
| marker-start                |                                                        |
| mask                        |                                                        |
| mix-blend-mode              | Not available as a presentation attribute.             |
| opacity                     |                                                        |
| overflow                    |                                                        |
| paint-order                 |                                                        |
| shape-rendering             |                                                        |
| stop-color                  |                                                        |
| stop-opacity                |                                                        |
| stroke                      |                                                        |
| stroke-dasharray            |                                                        |
| stroke-dashoffset           |                                                        |
| stroke-linecap              |                                                        |
| stroke-linejoin             |                                                        |
| stroke-miterlimit           |                                                        |
| stroke-opacity              |                                                        |
| stroke-width                |                                                        |
| text-anchor                 |                                                        |
| text-decoration             |                                                        |
| text-rendering              |                                                        |
| transform                   | SVG2; different syntax from the `transform` attribute. |
| unicode-bidi                |                                                        |
| visibility                  |                                                        |
| writing-mode                |                                                        |

### Filter effects

The following elements are filter effects:

* feBlend
* feColorMatrix
* feComponentTransfer
* feComposite
* feConvolveMatrix
* feDiffuseLighting
* feDisplacementMap
* feFlood
* feGaussianBlur
* feImage
* feMerge
* feMorphology
* feOffset
* feSpecularLighting
* feTile
* feTurbulence

All of those elements for filter effects support these attributes:

| Attribute | Notes |
| ---       | ---   |
| x         |       |
| y         |       |
| width     |       |
| height    |       |
| result    |       |

Some filter effect elements take one input in the `in` attribute, and
some others take two inputs in the `in`, `in2` attributes.  See the
table of elements above for details.

### Filter effect feComponentTransfer

The `feComponentTransfer` element can contain children `feFuncA`,
`feFuncR`, `feFuncG`, `feFuncB`, and those all support these
attributes:

| Attribute   | Notes |
| ---         | ---   |
| type        |       |
| tableValues |       |
| slope       |       |
| intercept   |       |
| amplitude   |       |
| exponent    |       |
| offset      |       |

# XML features

FIXME: `<xi:include href= parse= encoding=>`

FIXME: `<xi:fallback>`

FIXME: `xml:lang` attribute

FIXME: `xml:space` attribute

# Explicitly Unsupported features

* `flowRoot` element and its children - Inkscape, SVG 1.2 only., #13

* `glyph-orientation-horizontal` property - SVG1.1 only, removed in SVG2
