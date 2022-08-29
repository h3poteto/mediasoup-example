cmake_minimum_required(VERSION 3.13)

project(room LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 14)
set(CMAKE_CXX_STANDARD_REQUIRED ON)

add_executable(${PROJECT_NAME} main.cpp)

include(FetchContent)

# なぜだかわからんがfetchさせるとlinkエラーが全部消える……why
message(STATUS "\nFetching mediasoupclient...\n")
FetchContent_Declare(
  mediasoupclient
  GIT_REPOSITORY https://github.com/versatica/libmediasoupclient.git
  GIT_TAG v3
)
FetchContent_MakeAvailable(mediasoupclient)

target_include_directories(${PROJECT_NAME} PUBLIC
    ${WEBSOCKETPP_SOURCE_PATH}
    ${mediasoupclient_SOURCE_DIR}/include
    ${LIBWEBRTC_SOURCE_PATH}
)

target_compile_definitions(${PROJECT_NAME}
    PRIVATE
        WEBRTC_POSIX
        WEBRTC_LINUX
)


target_link_libraries(${PROJECT_NAME} PUBLIC
    ${LIBWEBRTC_BINARY_PATH}/libwebrtc${CMAKE_STATIC_LIBRARY_SUFFIX}
    mediasoupclient
)
