

**EXample usage:**

```bash
use yo_esp::{
    microphone::Microphone,
    run_audio_stream,
    AudioStreamConfig,
    CommandHandler,
};

struct MyHandler;
impl CommandHandler for MyHandler {
    fn on_wake_word_detected(&mut self) {
        info!("Wake word detected!");
    }
    fn on_command_executed(&mut self) {
        info!("Command executed");
    }
    fn on_command_failed(&mut self) {
        info!("Command failed");
    }
}

#[embassy_executor::task]
async fn audio_task(
    mic: Microphone,
    handler: MyHandler,
    stack: &'static embassy_net::Stack<'static>,
    addr: SocketAddr,
    config: AudioStreamConfig,
) {
    run_audio_stream(mic, handler, stack, addr, config).await;
}

// om main() after i2s init:
        let mic = Microphone::new(i2s_rx);
        let handler = MyHandler;
        let config = AudioStreamConfig {
            room: "esp",
            ..Default::default()
        };
        spawner.spawn(audio_task(mic, handler, stack, remote_addr, config)).unwrap();
```
