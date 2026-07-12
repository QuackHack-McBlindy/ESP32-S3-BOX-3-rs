# **ESP32-S3-BOX-3-rs**

[![Sponsors](https://img.shields.io/github/sponsors/QuackHack-McBlindy?logo=githubsponsors&label=Sponsor&style=flat&labelColor=ff1493&logoColor=fff&color=rgba(234,74,170,0.5) "")](https://github.com/sponsors/QuackHack-McBlindy) [![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-Sponsor?style=flat&logo=buymeacoffee&logoColor=fff&labelColor=ff1493&color=ff1493)](https://buymeacoffee.com/quackhackmcblindy)




Bare Metal *(no_std)* `ESP32-S3-BOX-3` firmware written in Rust (no `esp-idf`).   
Designed to be used as a voice assistant and smart speaker.  
Run this with a tiny GGML model (raise fuzzy matching level) and it will be **instant!**.  

**Box features:**
  

- [x] On-Device WebServer & Web Media Player (with casting to Android TV)
- [x] Voice Command Execution (Wake word, speech to shell command)
- [x] + hold-and-talk  
- [x] Stream any audio to speaker (wav, mp3, flac, m3u,, mp4, ...)
- [x] Browse server stored media in the GUI and cast to TV
- [ ] Alarms & timers
- [x] SSHd (+ client)
- [x] Interactive Shell over SSH
- [x] tinyWeather app - tap weather page to cycle displayed day (3-day forecast)
-  [  ] Embeddable mesh netoworking via WireGuard™ server implementation
- [x] Live bidirectional intercom audio call with mesh
- [x] Backend: `yo`

`yo` is not only the backend server service but it's also where you will write your voice commands.  
This is where your `ESP32-S3` microphone audio will be streamed.   

- [yo](https://github.com/QuackHack-McBlindy/yo)  
  - Wake Word Detection
  - Speech To Text
  - Text To Speech
  - Voice Command Execution
  - Control any device option with your voice!


<br>



<br>


## **Installation**

<details><summary><strong>
❄️ Using flakes (TODO)
</strong></summary>

*not yet...*

</details>


<details><summary><strong>
📦 Building from source
</strong></summary>


Configure WiFi and other required seetings in the example `.env` file.  

```bash
$ mv .env.example .env
$ nano .env
```


## **Build and flash!**

```bash
cargo run --release
```


</details>


<details><summary><strong>
🐋 Docker (recommended)
</strong></summary>

```bash
$ git clone https://github.com/QuackHack-McBlindy/ESP32-S3-BOX-3-rs
$ cd ESP32-S3-BOX-3-rs
```

Configure WiFi and other required seetings in the example `.env` file.  

```bash
$ mv .env.example .env
$ nano .env
```

`docker-compose.yaml` may require you to change the defined serial port.  
To locate the serial port for use with the `docker-compose.yaml` file you can run the following command:  

```bash
$ ls -l /dev/serial/by-id/
```

**Build and Flash!**

```bash
$ docker compose build
$ docker compose up
```


</details>


<br><br>

    
  


### **API**    

The API is designed to be easily expandable.       
*Fetch all your available endpoints at:* `curl http://<ESP_IP>/api`      
  
  
Using the internal API you can for example set the `ESP32-S3` display brightness to 75 percentage using:    

```bash
curl http://<ESP_IP>/api/settings/display/brightness/75 
```
  

| Endpoint | Description |
|----------|-------------|
| `/` | Serves the web frontend (HTML dashboard) |
| `/favicon.ico` | Serves the favicon |
| `/www/{file}` | Serves static files from the `www` directory |
| `/api` | Returns a plain‑text list of all available API endpoints |
| `/api/process/{value}` | Process a plain text natural language sentence and executes corresponding script with extracted parameters. |
| `/api/shell/{value}` | Send a Shell command (see supported commands below) |
| `/api/sensor/{value}` | Read a single sensor/system value (see supported keys below) |
| `/api/sensors` | Returns all sensor/system values as JSON |
| `/api/weather/update` | Update tinyWeather data |
| `/api/download/file/music/{filename}` | Download a song from the SD card’s `/Music` directory |
| `/api/download/file/share/{filename}` | Download any file from the `/share` directory of the SD card |
| `/api/upload/file/music/{filename}` | Upload a song to the SD card’s `/Music` directory **Note: POST** |
| `/api/media/gallery` | Draws `path` on the display. If no path is provided, the gallery is drawn. |
| `/api/media/prev` | Sends `previous` command to the media player – plays previous track |
| `/api/media/next` | Sends `next` command to the media player – plays next track |
| `/api/media/play_pause` | Toggles play/pause |
| `/api/media/heart` | Saves currently playing track to your favourite playlist |
| `/api/media/search/song/{value}` | Fuzzy search for the best single match and play it |
| `/api/media/search/songs/{value}` | Fuzzy search up to 10 matches and add them to the playlist |
| `/api/media/playlist/add/{value}` | Add a song (exact filename/path) to the playlist |
| `/api/media/playlist/remove/{value}` | Remove a song (exact filename/path) from the playlist |
| `/api/media/playlist/clear` | Clear the entire temporary playlist |
| `/api/media/playlist/fav` | Plays favourite playlist. Add/remove with heart. |
| `/api/settings/api/off` | Stops the internal API (including webserver). **Note: use GUI to turn back on** |
| `/api/settings/ssh/{value}` | Enable/disable/toggle the SSH server (`on`, `off`, `toggle`) |
| `/api/settings/sleep` | Enter deep sleep immediately |
| `/api/settings/sleep/reset` | Reset the deep‑sleep timer |
| `/api/settings/power/low/{value}` | Toggle low‑power mode (`on`, `off`, `toggle`) |
| `/api/settings/cpu/{value}` | Set CPU frequency (`80`, `160`, `240`) |
| `/api/settings/mic/volume/{value}` | Set microphone gain (0–100%). `{value}` as integer percent |
| `/api/settings/mic/mute/{value}` | Mute/unmute mic: `1`/`on`/`mute`, `0`/`off`/`unmute`, or `toggle` |
| `/api/settings/speaker/{value}` | Toggle speaker task on/off |
| `/api/settings/speaker/stream/{value}` | Toggle speaker streaming task on/off |
| `/api/settings/speaker/volume/{value}` | Set speaker volume (0–100%). Automatically handles mute/unmute & amplifier state |
| `/api/settings/speaker/mute/{value}` | Mute/unmute speaker: same options as mic mute |
| `/api/settings/speaker/amp/{value}` | Amplifier power: `on`, `off`, or `toggle` |
| `/api/settings/speaker/play/ding` | Play a test “ding” sound on the speaker |
| `/api/settings/voice/{value}` | Enable/disable/toggle the entire voice pipeline (`on`, `off`, `toggle`) |
| `/api/settings/voice/wakeword/{value}` | Enable/disable wake‑word streaming (`on`, `off`, `enable`, `disable`) |
| `/api/settings/voice/intercom/{value}` | Start/stop bidirectional live intercom mode with the backend.  |
| `/api/settings/vpn/{value}` | Enable/disable/toggle the VPN client (`on`, `off`, `toggle`) |
| `/api/settings/display/brightness/{value}` | Set backlight brightness (0–100%). `{value}` as integer percent |
| `/api/settings/display/state/{value}` | Set display state (`on`, `off`, `toggle`) |
| `/api/settings/display/page/{value}` | Change display page. `{value}` integer: 0=clock, 1=battery, 2=apps, 10=media player, etc. |
| `/api/settings/display/text/{value}` | Displays the provided value as a large text on the display |
| `/api/settings/display/call/{value}` | Show the incoming call screen with the caller’s name. The watch can accept/decline the call |
| `/api/settings/display/redraw` | Force a full redraw of the display |
| `/api/settings/display/redraw/loop/{value}` | Enable/disable the redraw loop (`on`, `off`) |
| `/api/settings/wifi/off` | Turns off the WiFi **Note: use GUI to turn back on** |
| `/api/settings/wifi/scan` | Trigger a WiFi scan (results printed to serial/log) |




### Supported sensor keys for `/api/sensor/{value}`

| Key(s)                                                       | Description                         |
|--------------------------------------------------------------|-------------------------------------|
| `battery`, `battery_level`, `battery_percentage`             | Battery charge in percent           |
| `battery_voltage`, `voltage`                                 | Battery voltage in millivolts       |
| `battery_charging`                                           | Charging status                     |
| `battery_need_charging`                                      | Low battery warning                 |
| `battery_full`                                               | Battery full flag                   |
| `battery_usb_connected`                                      | USB connection status               |
| `brightness`, `display`                                      | Display brightness (0–100)          |
| `display_state`                                              | Display power state                 |
| `display_timeout`, `screen_timeout`                          | Display auto‑off timeout (seconds)  |
| `rssi`, `wifi_signal`, `wifi`                                | Wi‑Fi signal strength in dBm        |
| `ip`                                                         | Device IPv4 address                 |
| `cpu`, `cpu_freq`, `cpu_speed`                               | Current CPU frequency in MHz        |
| `speaker`                                                    | Speaker volume (0–100)              |
| `mic`                                                        | Microphone gain (0–100)             |
| `uptime`                                                     | System uptime (e.g., "02h 15m 30s") |
| `time`                                                       | Current time in HH:MM:SS            |
| `firmware`, `version`                                        | Firmware version string             |
| `mic_muted`                                                  | Microphone mute state               |
| `speaker_muted`                                              | Speaker mute state                  |
| `speaker_task_state`                                         | Speaker task running                |
| `speaker_allow_streaming`                                    | Streaming allowed flag              |
| `amplifier_state`                                            | Audio amplifier power state         |         
| `low_power`, `low_power_mode`, `power_save`                  | Low‑power mode state                |
| `sd_ready`                                                   | SD card ready status                |
| `media_is_playing`                                           | Media playback active               |
| `ssh`, `ssh_state`                                           | SSH server state                    |
| `vpn`, `vpn_state`                                           | VPN client state   
| `powerdown_timeout`, `sleep_timeout`                         | Deep‑sleep timeout (seconds)        |



<br><br>



# **HARDWARE**


### **📶 Specs**

- Main Module: ESP32-S3-WROOM-1
- SoC: ESP32-S3 (dual-core Xtensa LX7 240 MHz)
- Memory: SRAM 512 KB internal, 16 MB QSPI Flash, 16 MB Octal PSRAM @80MHz

### **🔋 PMU (AXP2101)**

- Rechargable 18650 Battery *(note: O=11mm)*
- Messure battery procentage on ADC1 by dividing battery voltage with 4.11 

### **🖥️ Display (ILI9341)**

- SPI
- Interface speed: 40 MHz
- Backlight output GPIO: 47 (LEDC)
- 2.4" LCD
- 320x240

### **👉 Touch (GT911)**

- GPIO 3
- Adress: `0x5D`
- i2c bus a
- 10 Point Captive Touch

### **📢 Amplifier (NS4150)**

- Digital Output GPIO: 15 (I2S output) 
- 16-bit, 48 kHz sample rate
- built-in 8Ω/1W speaker (NS4150)
- Audio Codec (ES8311) 0x18
- Channel Left

### **🎙️ Microphone (ES7210)**

- Digital Input GPIO: 16 (I2S input) 
- Dual digital microphones
- Audio Codec (ES7210)
- 16-bit, 16 kHz sample rate
- 0x40

### **🕵️ Presence Sensor (MS58-3909S68U4)**

- Radar at GPIO: 21  
- Frequency band: 5.8 GHz
- 2 meter range

### **🌡️ Temperature Sensor (AHT20)**

- Temperature Sensor
- Humidity Sensor

### **🧭 Gyroscope (ICM-42607-P)**

- 3-axis Gyroscope
- 3-axis Accelerometer 


### **📡 Infrared (IR)**

- Emitter (IRM-H638T) 0x68 ?
- Receiver (IR-6721C/TR8) 


### **🧩 Extensions** 

- ESP32-S3-BOX-3-DOCK
- ESP32-S3-BOX-3-SENSOR
- ESP32-S3-BOX-3-BRACKET
- ESP32-S3-BOX-3-BREAD: PCIe to 2.54mm headers 
- 2x headers (16 GPIOs, 3.3V)
- SD card slot (up to 32gb)
- USB A

### **⭕ Buttons**

- Top Left (GPIO 0)
- Reset
- Boot
- Mute (GPIO 46)

### **I2C**

**Bus A**
- 100kHz
- sda: GPIO 08 (pullup_enabled)
- scl: GPIO 18 (pullup_enabled)

**Bus B**
- 50kHz
- sda: GPIO 41 (pullup_enabled)
- scl: GPIO 40 (pullup_enabled)


### **i2S**

- lrclk_pin: GPIO45 (ignore_strapping_warning)  
- bclk_pin: GPIO17
- mclk_pin: GPIO2

### **Audio ADC (es7210)**

- I2C Bus A
- 16bit, 16000 sample rate

### **Audio DAC (es8311)**

- I2C Bus A
- 16bit, 48000 sample rate


<br>

## **🪑 Table**

| Component              | Interface       | Pin(s) / Address          | Notes                                                                 |
|------------------------|-----------------|---------------------------|-----------------------------------------------------------------------|
| **ESP32-S3**           | -               | -                         | Main microcontroller, 16MB flash, octal PSRAM @80MHz                 |
| **Display (LCD)**      | SPI             | CLK=GPIO7, MOSI=GPIO6     | ILI9xxx driver, model `S3BOX` (ILI9341 compatible)                   |
|                        |                 | CS=GPIO5, DC=GPIO4        |                                                                       |
|                        |                 | Reset=GPIO48 (inverted)   |                                                                       |
| **Backlight**          | PWM (LEDC)      | GPIO47                    |                                       |
| **Touchscreen**        | I²C (bus A)     | SDA=GPIO8, SCL=GPIO18     | GT911 controller, address `0x5D`                                     |
|                        |                 | Interrupt=GPIO3           |                                                                       |
| **I2S Audio Bus**      | I2S             | BCLK=GPIO17, LRCLK=GPIO45 | Shared between microphone and speaker                                |
|                        |                 | MCLK=GPIO2                | Master clock for audio codecs                                        |
| **Microphone**         | I2S (input)     | DIN=GPIO16                | ES7210 ADC, I²C controlled (bus A)                                   |
| **Speaker**            | I2S (output)    | DOUT=GPIO15               | ES8311 DAC, I²C controlled (bus A)                                   |
| **Audio ADC (ES7210)** | I²C (bus A)     | Address `0x40`? (default) | Microphone front-end, 16-bit, 16 kHz sample rate                     |
| **Audio DAC (ES8311)** | I²C (bus A)     | Address `0x18`? (default) | Speaker amplifier, 16-bit, 48 kHz sample rate                        |
| **Physical Button**    | GPIO input      | GPIO0                     | Top‑left button, internal pull‑up, inverted                          |
| **Radar Presence**     | GPIO input      | GPIO21                    | Occupancy sensor (HLK-LD2410 ?)                                  |
| **Speaker Enable**     | GPIO output     | GPIO46                    | Switch to enable/disable external speaker amp                        |
| **Temperature/Humidity**| I²C (bus B)    | SDA=GPIO41, SCL=GPIO40    | AHT20 sensor, address `0x38` (default)                               |
| **Battery Voltage**    | ADC1            | GPIO10                    | Measures battery voltage via voltage divider (multiply by 4.11)      |
| **I²C Bus A**          | I²C             | SDA=GPIO8, SCL=GPIO18     | 100 kHz, pull‑ups enabled – connects touch, ES7210, ES8311           |
| **I²C Bus B**          | I²C             | SDA=GPIO41, SCL=GPIO40    | 50 kHz, pull‑ups enabled – connects AHT20 sensor                     |
| **USB‑Serial‑JTAG**    | USB             | -                         | Built‑in, used for logging        |
| **PSRAM**              | -               | -                         | Octal PSRAM, 8 MB                    |
| **WiFi/Bluetooth**     | -               | -                         | Integrated                                          |


<br>


<br><br>


## **☕**

[![Sponsors](https://img.shields.io/github/sponsors/QuackHack-McBlindy?logo=githubsponsors&label=Sponsor&style=flat&labelColor=ff1493&logoColor=fff&color=rgba(234,74,170,0.5) "")](https://github.com/sponsors/QuackHack-McBlindy) [![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-Sponsor?style=flat&logo=buymeacoffee&logoColor=fff&labelColor=ff1493&color=ff1493)](https://buymeacoffee.com/quackhackmcblindy)
> 🦆🧑‍🦯 says ⮞ Hi! I'm QuackHack-McBlindy!  
> Like my work?  
> Buy me a coffee, or become a sponsor.  
> Thanks for supporting open source/hungry developers ♥️🦆!   

♥️₿ *Wallet:* `pungkula.x`  
<a href="https://www.buymeacoffee.com/quackhackmcblindy" target="_blank"><img src="https://cdn.buymeacoffee.com/buttons/v2/default-yellow.png" alt="Buy Me A Coffee" style="height: 60px !important;width: 217px !important;" ></a>



<br>

## **License**

This project is licensed under the terms of the MIT license.  
See the `LICENSE` file in the repository for full details.  
