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

target_include_directories(mcping PUBLIC
        $<BUILD_INTERFACE:${CMAKE_CURRENT_BINARY_DIR}>
        $<INSTALL_INTERFACE:include/mcping>
)
target_link_libraries(mcping PRIVATE resolv)
set_target_properties(mcping PROPERTIES C_VISIBILITY_PRESET hidden)

install(TARGETS mcping
        EXPORT mcping
        RUNTIME DESTINATION bin
        ARCHIVE DESTINATION lib
        LIBRARY DESTINATION lib
)
install(FILES mcping.h DESTINATION DESTINATION include)

include(GNUInstallDirs)
set(LIB_INSTALL_DIR "${CMAKE_INSTALL_LIBDIR}")
set(INCLUDE_INSTALL_DIR "${CMAKE_INSTALL_INCLUDEDIR}")
configure_file(
        "${CMAKE_CURRENT_SOURCE_DIR}/mcping.pc.in"
        "${CMAKE_CURRENT_BINARY_DIR}/mcping.pc"
        @ONLY
)
install(
        FILES "${CMAKE_CURRENT_BINARY_DIR}/mcping.pc"
        DESTINATION "${CMAKE_INSTALL_DATADIR}/pkgconfig"
)