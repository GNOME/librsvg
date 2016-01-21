/* librsvg - SVG rendering library
 * Copyright (C) 2015 Chun-wei Fan <fanc999@yahoo.com.tw>
 *
 * Author: Chun-wei Fan <fanc999@yahoo.com.tw>
 *
 * This library is free software; you can redistribute it and/or
 * modify it under the terms of the GNU Lesser General Public
 * License as published by the Free Software Foundation; either
 * version 2 of the License, or (at your option) any later version.
 *
 * This library is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
 * Lesser General Public License for more details.
 *
 * You should have received a copy of the GNU Lesser General Public
 * License along with this library. If not, see <http://www.gnu.org/licenses/>.
 */

#include <float.h>

/* include the system's math.h */
#include <../include/math.h>
#include <glib.h>

#if (_MSC_VER < 1800)
/* it seems of the supported compilers only
 * MSVC does not have isnan(), but it does
 * have _isnan() which does the same as isnan()
 */
#ifndef __MSVC_ISNAN_FALLBACK__
#define __MSVC_ISNAN_FALLBACK__
static inline gboolean
isnan (double x)
{
  return _isnan (x);
}
#endif /* __MSVC_ISNAN_FALLBACK__ */
#endif /* _MSC_VER < 1800 */
