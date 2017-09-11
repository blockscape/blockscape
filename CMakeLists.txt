cmake_minimum_required(VERSION 3.8)
project(blockscape_core)

set(CMAKE_CXX_FLAGS "${CMAKE_CXX_FLAGS} --std=c++17 -Wall -Wextra -Werror=implicit-function-declaration")
set(CMAKE_CXX_FLAGS_DEBUG "-O0 -g")
set(CMAKE_CXX_FLAGS_RELEASE "-O3")

include_directories(lib/GSL/include/)

file(GLOB_RECURSE SOURCE_FILES src/*.cpp)
add_library(blockscape_core ${SOURCE_FILES})
