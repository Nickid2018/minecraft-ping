cmake_minimum_required(VERSION 3.25)
project(McPingBin C)

set(CMAKE_C_STANDARD 11)
file(GLOB MAIN_HEADERS "*.h")
file(GLOB MAIN_SOURCES "*.c")

find_package(PkgConfig)
pkg_check_modules(cjson REQUIRED IMPORTED_TARGET libcjson)
link_libraries(PkgConfig::cjson)

add_executable(McPingBin ${MAIN_HEADERS} ${MAIN_SOURCES})
target_link_libraries(McPingBin PRIVATE mcping)
set_target_properties(McPingBin PROPERTIES OUTPUT_NAME "mcping")
install(TARGETS McPingBin RUNTIME DESTINATION bin)