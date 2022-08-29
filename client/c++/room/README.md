# Requirements

Install these packages

- nlohmann-json

# Generate compile_command.json

```
$ cmake -DCMAKE_EXPORT_COMPILE_COMMANDS=ON \
  -DCMAKE_CXX_FLAGS=-latomic \
  -DWEBSOCKETPP_SOURCE_PATH=${WEBSOCKETPP_SOURCE_PATH} \
  -DLIBMEDIASOUP_SOURCE_PATH=${LIBMEDIASOUP_SOURCE_PATH} \
  -DJSON_INCLUDE_PATH=${JSON_INCLUDE_PATH} \
  -DLIBWEBRTC_SOURCE_PATH=${LIBWEBRTC_SOURCE_PATH} \
  -DLIBWEBRTC_BINARY_PATH=${LIBWEBRTC_BINARY_PATH} \
  .
```

# Build

```
$ cmake \
  -DCMAKE_CXX_FLAGS=-latomic \
  -DWEBSOCKETPP_SOURCE_PATH=${WEBSOCKETPP_SOURCE_PATH} \
  -DLIBMEDIASOUP_SOURCE_PATH=${LIBMEDIASOUP_SOURCE_PATH} \
  -DJSON_INCLUDE_PATH=${JSON_INCLUDE_PATH} \
  -DLIBWEBRTC_SOURCE_PATH=${LIBWEBRTC_SOURCE_PATH} \
  -DLIBWEBRTC_BINARY_PATH=${LIBWEBRTC_BINARY_PATH} \
  -S . -B ./build
$ cmake --build ./build
```

`CMAKE_CXX_FLAGS=-latomic` is required to avoid link errors:
```
/usr/bin/ld: /home/h3poteto/src/webrtc-checkout/src/out/m94/obj/libwebrtc.a(thread_group.o): in function `base::internal::ThreadGroup::ShouldYield(base::internal::TaskSourceSortKey)':
thread_group.cc:(.text+0x41): undefined reference to `__atomic_load'
```
