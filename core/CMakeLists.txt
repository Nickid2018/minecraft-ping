cmake_minimum_required(VERSION 3.25)
project(mcping LANGUAGES C VERSION 1.0.0)

set(CMAKE_C_STANDARD 11)

file(GLOB CORE_HEADERS "*.h")
file(GLOB CORE_SOURCES "*.c")

find_package(PkgConfig)
pkg_check_modules(glib REQUIRED IMPORTED_TARGET glib-2.0)
link_libraries(PkgConfig::glib)
pkg_check_modules(cjson REQUIRED IMPORTED_TARGET libcjson)
link_libraries(PkgConfig::cjson)

include_directories(".")
add_library(mcping SHARED ${CORE_HEADERS} ${CORE_SOURCES})

include(GenerateExportHeader)
generate_export_header(mcping BASE_NAME mcping EXPORT_FILE_NAME mcping_export.h)

include(GNUInstallDirs)

target_include_directories(mcping PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}>
        $<INSTALL_INTERFACE:include/mcping>
)
target_link_libraries(mcping PRIVATE resolv)
set_target_properties(mcping PROPERTIES C_VISIBILITY_PRESET hidden)

install(FILES mcping.h DESTINATION ${CMAKE_INSTALL_INCLUDEDIR})

install(TARGETS mcping
        EXPORT mcping
        RUNTIME DESTINATION bin
        ARCHIVE DESTINATION lib
        LIBRARY DESTINATION lib
)