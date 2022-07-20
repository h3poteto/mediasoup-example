cmake_minimum_required(VERSION 3.13)

project(room LANGUAGES CXX)

include_directories(${WEBSOCKETPP_INCLUDE_PATH})

add_executable(main.out main.cpp)

