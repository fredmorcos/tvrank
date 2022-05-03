cmake_minimum_required(VERSION 3.5)

include(FetchContent)

FetchContent_Declare(
  Corrosion
  GIT_REPOSITORY https://github.com/corrosion-rs/corrosion.git
  GIT_TAG v0.2.1 # Optionally specify a commit hash, version tag or branch here
)

FetchContent_MakeAvailable(Corrosion)

corrosion_import_crate(MANIFEST_PATH ../cpp/Cargo.toml)

project(tvrank-ui VERSION 0.8.4 LANGUAGES CXX)

set(CMAKE_INCLUDE_CURRENT_DIR ON)

set(CMAKE_AUTOUIC ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

# Build timestamp
string(TIMESTAMP TVRANK_BUILD_TIMESTAMP "%Y-%m-%dT%H:%M:%SZ")

configure_file(${CMAKE_CURRENT_SOURCE_DIR}/config.h.in
               ${CMAKE_BINARY_DIR}/config.h)

find_package(QT NAMES Qt6 REQUIRED COMPONENTS Widgets)
find_package(Qt${QT_VERSION_MAJOR} REQUIRED COMPONENTS Widgets)
find_package(Qt${QT_VERSION_MAJOR} REQUIRED COMPONENTS Concurrent)

set(PROJECT_SOURCES
  main.cpp
  mainwindow.h mainwindow.cpp mainwindow.ui
  titlesmodel.h titlesmodel.cpp
  progressstatus.h progressstatus.cpp progressstatus.ui
  tvrankserviceinit.h tvrankserviceinit.cpp
  tvrankservice.h tvrankservice.cpp
  icons.qrc
)

qt_add_executable(tvrank-qt MANUAL_FINALIZATION ${PROJECT_SOURCES})

# Define target properties for Android with Qt 6 as:
#    set_property(TARGET tvrank-ui APPEND PROPERTY QT_ANDROID_PACKAGE_SOURCE_DIR
#                 ${CMAKE_CURRENT_SOURCE_DIR}/android)
# For more information, see https://doc.qt.io/qt-6/qt-add-executable.html#target-creation

target_include_directories(tvrank-qt PRIVATE ../target)

target_link_libraries(tvrank-qt PRIVATE Qt${QT_VERSION_MAJOR}::Widgets)
target_link_libraries(tvrank-qt PRIVATE Qt${QT_VERSION_MAJOR}::Concurrent)
target_link_libraries(tvrank-qt PRIVATE tvrank-cpp)

set_target_properties(tvrank PROPERTIES
  MACOSX_BUNDLE_GUI_IDENTIFIER fredmorcos.com
  MACOSX_BUNDLE_BUNDLE_VERSION ${PROJECT_VERSION}
  MACOSX_BUNDLE_SHORT_VERSION_STRING ${PROJECT_VERSION_MAJOR}.${PROJECT_VERSION_MINOR}
  MACOSX_BUNDLE TRUE
  WIN32_EXECUTABLE TRUE
)

qt_finalize_executable(tvrank)
