# Requirements

Install these packages

- nlohmann-json
- abseil-cpp

# Generate compile_command.json

```
$ cmake -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
  -DCMAKE_CXX_FLAGS=-latomic \
  -DWEBSOCKETPP_INCLUDE_PATH=${WEBSOCKETPP_INCLUDE_PATH} \
  -DLIBMEDIASOUP_INCLUDE_PATH=${LIBMEDIASOUP_INCLUDE_PATH} \
  -DJSON_INCLUDE_PATH=${JSON_INCLUDE_PATH} \
  -DLIBWEBRTC_INCLUDE_PATH=${LIBWEBRTC_INCLUDE_PATH} \
  -DLIBWEBRTC_BINARY_PATH=${LIBWEBRTC_BINARY_PATH} \
  .
```

# Build

```
$ cmake \
  -DCMAKE_CXX_FLAGS=-latomic \
  -DWEBSOCKETPP_INCLUDE_PATH=${WEBSOCKETPP_INCLUDE_PATH} \
  -DLIBMEDIASOUP_INCLUDE_PATH=${LIBMEDIASOUP_INCLUDE_PATH} \
  -DJSON_INCLUDE_PATH=${JSON_INCLUDE_PATH} \
  -DLIBWEBRTC_INCLUDE_PATH=${LIBWEBRTC_INCLUDE_PATH} \
  -DLIBWEBRTC_BINARY_PATH=${LIBWEBRTC_BINARY_PATH} \
  -S . -B ./build
$ cmake --build ./build
```

`CMAKE_CXX_FLAGS=-latomic` is required to avoid link errors:
```
/usr/bin/ld: /home/h3poteto/src/webrtc-checkout/src/out/m94/obj/libwebrtc.a(thread_group.o): in function `base::internal::ThreadGroup::ShouldYield(base::internal::TaskSourceSortKey)':
thread_group.cc:(.text+0x41): undefined reference to `__atomic_load'
```
