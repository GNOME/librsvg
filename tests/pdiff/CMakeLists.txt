PROJECT (PerceptualDiff)
SET(DIFF_SRC PerceptualDiff.cpp LPyramid.cpp RGBAImage.cpp
CompareArgs.cpp Metric.cpp)

ADD_EXECUTABLE (perceptualdiff ${DIFF_SRC})

# look for libtiff
FIND_PATH(TIFF_INCLUDE_DIR tiff.h
  /usr/local/include
  /usr/include
  /opt/local/include
)

FIND_LIBRARY(TIFF_LIBRARY tiff
  /usr/lib
  /usr/local/lib
  /opt/local/lib
)

IF(TIFF_INCLUDE_DIR)
  IF(TIFF_LIBRARY)
    SET( TIFF_FOUND "YES" )
    SET( TIFF_LIBRARIES ${TIFF_LIBRARY} )
  ENDIF(TIFF_LIBRARY)
ENDIF(TIFF_INCLUDE_DIR)

IF(TIFF_FOUND)
  INCLUDE_DIRECTORIES(${TIFF_INCLUDE_DIR})
  TARGET_LINK_LIBRARIES(perceptualdiff ${TIFF_LIBRARY})
ENDIF(TIFF_FOUND)

# look for libpng
FIND_PATH(PNG_INCLUDE_DIR png.h
  /usr/local/include
  /usr/include
  /opt/local/include
)

FIND_LIBRARY(PNG_LIBRARY png
  /usr/lib
  /usr/local/lib
  /opt/local/lib
)

IF(PNG_INCLUDE_DIR)
  IF(PNG_LIBRARY)
    SET( PNG_FOUND "YES" )
    SET( PNG_LIBRARIES ${PNG_LIBRARY} )
  ENDIF(PNG_LIBRARY)
ENDIF(PNG_INCLUDE_DIR)

IF(PNG_FOUND)
  INCLUDE_DIRECTORIES(${PNG_INCLUDE_DIR})
  TARGET_LINK_LIBRARIES(perceptualdiff ${PNG_LIBRARY})
ENDIF(PNG_FOUND)