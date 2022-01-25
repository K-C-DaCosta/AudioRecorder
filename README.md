# Wasm Recorder
it records audio and uploads it to server

# How to run
make sure cargo is installed( https://www.rust-lang.org/tools/install  then:


## clone 
```
git clone https://github.com/K-C-DaCosta/AudioRecorder.git
cd ./AudioRecorder
```

## install simple-http-server and wasm-bindgen-cli from crates.io
```
cargo install simple-http-server;
cargo install wasm-bindgen-cli;
```

## run script to start the server 
```
./build.sh build_and_run
```
this will start a server on `localhost:6969` 

## view demo
in any browser(i recommend firefox) go to the address:
`localhost:6969/index.html`

when you press submit a new binary file should show up in `./recorder_output/test`  the file is encoded in a custom binary format. 

## how to listen to the recording
This project encodes data into a custom audio format so no audio software will be able to decode it in its current representation. However, I wrote a test that decodes the uploaded recording to wav just type:
```
cargo test foo
```
A wav version of the file should be written to `recorder_output/test` and you can playback your voice in vlc/mpv/etc








