cmake_minimum_required(VERSION 3.31)
project(mcping C)

set(CMAKE_C_STANDARD 11)
file(GLOB HEADERS "*.h")
file(GLOB SOURCES "*.c")

find_package(PkgConfig)
pkg_check_modules(glib REQUIRED IMPORTED_TARGET glib-2.0)
link_libraries(PkgConfig::glib)
pkg_check_modules(cjson REQUIRED IMPORTED_TARGET libcjson)
link_libraries(PkgConfig::cjson)

include_directories(".")
add_executable(mcping ${HEADERS} ${SOURCES})
target_link_libraries(mcping resolv)
set_target_properties(mcping PROPERTIES OUTPUT_NAME "mcping" PREFIX "")