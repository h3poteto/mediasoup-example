cmake_minimum_required(VERSION 3.13)

project(room LANGUAGES CXX)

add_executable(room main.cpp)

target_include_directories(${PROJECT_NAME} PUBLIC
    ${WEBSOCKETPP_INCLUDE_PATH}
    ${LIBMEDIASOUP_INCLUDE_PATH}
    ${JSON_INCLUDE_PATH}
    ${LIBWEBRTC_INCLUDE_PATH}
)


target_compile_definitions(${PROJECT_NAME}
    PRIVATE
        WEBRTC_POSIX
        WEBRTC_LINUX
)

target_link_libraries(${PROJECT_NAME} PUBLIC
    ${LIBWEBRTC_BINARY_PATH}/libwebrtc${CMAKE_STATIC_LIBRARY_SUFFIX}
)
