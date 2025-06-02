## 配环境提示

环境变量配置：

```sh
export DEP_LV_CONFIG_PATH=$(pwd)/lvgl
export CFLAGS=-I/opt/st/myir/3.1-snapshot/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi/usr/include
export BINDGEN_EXTRA_CLANG_ARGS=-I/opt/st/myir/3.1-snapshot/sysroots/cortexa7t2hf-neon-vfpv4-ostl-linux-gnueabi/usr/include
export LVGL_LINK=
```

编译调试版本：

```sh
cargo b --target armv7-unknown-linux-gnueabihf
```

编译生产版本：

```sh
cargo b -r -Z build-std=core,alloc,std,panic_abort -Z build-std-features=optimize_for_size,panic_immediate_abort --target armv7-unknown-linux-gnueabihf
```

带日志运行：

```sh
WAYLAND_DEBUG=1 RUST_LOG=info ./mp3 wavs
```
