// STATE.RS
// STATE MACHINE 
// CHIP GPIO, CONFIGURATION DEFINITIONS
// ++ CURRENT STATES AS ATOMIC VARIABLES 


crate::init_bool!(BOX1, false);
crate::init_bool!(BOX2, true);
crate::init_bool!(BOX3, false);
crate::init_bool!(BOX4, true);
crate::init_bool!(MEDIA_IS_LIKED, false);

// ───────────────────────────────────────────────────────────────────────
// THIS FIRMWARE
pub const FW_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PROJECT_NAME: &str = env!("CARGO_PKG_NAME");


// ───────────────────────────────────────────────────────────────────────
// TIME RELATED
pub static CURRENT_TIME: critical_section::Mutex<core::cell::Cell<Option<crate::base::time::DateTime>>> =
    critical_section::Mutex::new(core::cell::Cell::new(None));

crate::init_u32!(UPTIME_SECS, 0);      // SECONDS SINCE BOOT
crate::init_u32!(CURRENT_TIME_SECS, 0);// SECONDS SINCE MIDNIGHT


// ───────────────────────────────────────────────────────────────────────
// NETWORK RELATED
//pub static CONNECTED_SSID: embassy_sync::mutex::Mutex<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, Option<&'static str>> =
//    embassy_sync::mutex::Mutex::new(None);
//pub static CONNECTED_SSID: embassy_sync::mutex::Mutex<
//    embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex,
//    Option<heapless::String<32>>,
//> = embassy_sync::mutex::Mutex::new(None);
crate::init_string!(CONNECTED_SSID, 32);


crate::init_u32!(CURRENT_IP, 0);
crate::init_i32!(RSSI, 0);
crate::init_bool!(WIFI_CONNECTED, false);
crate::init_bool!(WIFI_STATE, false);
crate::init_bool!(API_STATE, true);


// WIFI - COMPILE-TIME ENVIRONMENT VARIABLES
pub const SSID: &str = env!("WIFI_SSID");
pub const PASSWORD: &str = env!("WIFI_PASSWORD");

// OPTIONAL MORE WIFI
// ADD AS MANY AS NEEDED
pub const WIFI_CREDENTIALS: &[(&str, &str)] = &[
    (SSID, PASSWORD),
    (env!("WIFI_SSID2"), env!("WIFI_PASSWORD2")),
    (env!("WIFI_SSID3"), env!("WIFI_PASSWORD3")),
];


// BLUETOOTH
crate::init_bool!(BLUETOOTH_STATE, false); 

// BACKEND
pub const BACKEND_TCP_HOST: &str = env!("BACKEND_TCP_HOST");
pub const BACKEND_TCP_PORT_STR: &str = env!("BACKEND_TCP_PORT");
pub const ZIGDUCK_BASE_URL: Option<&str> = option_env!("ZIGDUCK_BASE_URL");
pub const ZIGDUCK_API_PASSWORD: &str = env!("ZIGDUCK_API_PASSWORD");
pub static YO_HOSTS: [embassy_net::IpAddress; 4] = [
    embassy_net::IpAddress::v4(192,168,1,111),// DESKTOP
    embassy_net::IpAddress::v4(192,168,1,221),// HOMESERVER
    embassy_net::IpAddress::v4(192,168,1,222),// LAPTOP
    embassy_net::IpAddress::v4(192,168,1,28), // NAS
  //embassy_net::IpAddress::v4(192,168,1,23), // PINEPHONE
  //embassy_net::IpAddress::v4(192,168,1,13), // iPHONE
  //embassy_net::IpAddress::v4(192,168,1,x),  // Pi 4B
  //embassy_net::IpAddress::v4(192,168,1,x),  // ESP32S3-BOX-3
];

// SSH
crate::init_bool!(SSH_STATE, false);
pub const SSH_USER: &str = env!("SSH_USER");
pub const SSH_PASSWORD: &str = env!("SSH_PASSWORD");
pub const SSH_HOSTKEY_HEX: Option<&str> = option_env!("SSH_HOSTKEY");
pub const SSH_PRIVATE_KEY_HEX: Option<&str> = option_env!("SSH_PRIVATE_KEY");
// AUTHORIZED KEYS
pub const MAX_KEYS: usize = 3; // INCREMMENT IF USING MORE KEYS! 
pub const SSH_PUBKEY: Option<&str> = option_env!("SSH_PUBKEY");
pub const SSH_PUBKEY2: Option<&str> = option_env!("SSH_PUBKEY2");
pub const SSH_PUBKEY3: Option<&str> = option_env!("SSH_PUBKEY3");
// KNOWN HOSTS
pub const SSH_KNOWN_HOST: Option<&str> = option_env!("SSH_KNOWN_HOST");
pub const SSH_KNOWN_HOST2: Option<&str> = option_env!("SSH_KNOWN_HOST2");
pub const SSH_KNOWN_HOST3: Option<&str> = option_env!("SSH_KNOWN_HOST3");
// REMOTE SSH
crate::init_string!(SSH_REMOTE_IP, 16);
crate::init_string!(SSH_REMOTE_COMMAND, 64);

// WIREGUARD
pub const WG_PRIVATE_KEY_HEX: Option<&str> = option_env!("WG_PRIVATE_KEY");
pub const WG_SERVER_PUB_KEY: Option<&str> = option_env!("WG_SERVER_PUBLIC_KEY");
pub const WG_ENDPOINT: Option<&str> = option_env!("WG_ENDPOINT");
crate::init_bool!(WG_STATE, false);
crate::init_bool!(VPN_ACTIVE, false);

// ───────────────────────────────────────────────────────────────────────
// CPU RELATED
crate::init_u16!(CPU_FREQ, 240);


// ───────────────────────────────────────────────────────────────────────
// DISPLAY RELATED
// DISPLAY - SPI (ILI9341)
pub const LCD_MOSI: u8 = 6;
pub const LCD_CLK:  u8 = 7;
pub const LCD_CS:   u8 = 5;
pub const LCD_DC:   u8 = 4;
pub const LCD_RST:  u8 = 48;   // INVERTED
pub const LCD_BL:   u8 = 47;   // LEDC

pub const LCD_WIDTH:  u16 = 320;
pub const LCD_HEIGHT: u16 = 240;
pub const LCD_COL_OFFSET: u16 = 0;
pub const LCD_ROW_OFFSET: u16 = 0;

// TE (TEARING EFFECT SYNC)
crate::init_u8!(LCD_TE, 13);

pub static DELAYED_DIRTY_TIME: critical_section::Mutex<core::cell::Cell<Option<embassy_time::Instant>>> =
    critical_section::Mutex::new(core::cell::Cell::new(None));
    
crate::init_bool!(DISPLAY_STATE, false);
crate::init_bool!(DISPLAY_DIRTY, false);
crate::init_bool!(DISPLAY_LOOP_DIRTY, false);
crate::init_u8!(DISPLAY_BRIGHTNESS, 35);
crate::init_bool!(DISPLAY_TOUCH_ACTIVITY, false);
crate::init_u32!(DISPLAY_TIMEOUT_SECS, 35);

// GALLERY INDEX
crate::init_u8!(GALLERY_INDEX, 1);

// MAX CALLER LENGTH
pub const MAX_DISPLAY_STRING_LEN: usize = 32;

// STORAGE FOR CALLER ID
crate::init_string!(DISPLAY_STRING, MAX_DISPLAY_STRING_LEN);


// ───────────────────────────────────────────────────────────────────────
// I2C Bus A (100 kHz) – touch, audio codecs
pub const I2C_A_SDA: u8 = 8;
pub const I2C_A_SCL: u8 = 18;
pub const I2C_A_FREQ_HZ: u32 = 100_000;

// I2C Bus B (50 kHz) – AHT20
pub const I2C_B_SDA: u8 = 41;
pub const I2C_B_SCL: u8 = 40;
pub const I2C_B_FREQ_HZ: u32 = 50_000;

// ───────────────────────────────────────────────────────────────────────
// TOUCH RELATED
crate::init_u8!(TP_INT, 3);
crate::init_u8!(TP_I2C_ADDR, 0x5D);


// ───────────────────────────────────────────────────────────────────────
// PMU RELATED
crate::init_u8!(PMIC_I2C_ADDR, 0x34);
crate::init_bool!(POWER_STATE, true);
crate::init_bool!(LOW_POWER_MODE, false);
crate::init_u32!(POWERDOWN_TIMEOUT_SECS, 100);

// ───────────────────────────────────────────────────────────────────────
// BATTERY RELATED
crate::init_u8!(BATTERY_PERCENT, 100);
crate::init_u32!(BATTERY_VOLTAGE, 0);
crate::init_bool!(BATTERY_CHARGING, false);
crate::init_bool!(BATTERY_NEED_CHARGING, false);
crate::init_bool!(BATTERY_FULL, false);
crate::init_bool!(BATTERY_USB_CONNECTED, false);


// ───────────────────────────────────────────────────────────────────────
// AHT20 RELATED
crate::init_u8!(HUMIDITY, 0);
crate::init_u8!(TEMPERATURE, 0);

// ───────────────────────────────────────────────────────────────────────
// PRESENCE RELATED
crate::init_bool!(PRESENCE, false);



// ───────────────────────────────────────────────────────────────────────
// RTC RELATED
crate::init_u8!(RTC_I2C_ADDR, 0x51);


// ───────────────────────────────────────────────────────────────────────
// AUDIO RELATED

// I2S AUDIO GPIO
crate::init_u8!(I2S_MCLK, 2);
crate::init_u8!(I2S_BCLK, 17); 
crate::init_u8!(I2S_LRCK, 45); 
crate::init_u8!(I2S_DOUT, 15); 
crate::init_u8!(I2S_DIN, 16); 

// I2S AUDIO CONFIG
pub const I2S_SAMPLE_RATE: u32 = 16000;
pub const I2S_SAMPLE_COUNT: usize = 256;
pub const I2S_BIT_WIDTH: u8 = 16;
//pub const I2S_BUFFER_SIZE: usize = 4 * 4092;
pub const I2S_BUFFER_SIZE: usize = 4 * 16368;
pub const I2S_DATA_FORMAT: esp_hal::i2s::master::DataFormat = esp_hal::i2s::master::DataFormat::Data16Channel16;
pub const I2S_ENDIANNESS: esp_hal::i2s::master::Endianness = esp_hal::i2s::master::Endianness::LittleEndian;
pub const I2S_CHANNELS: esp_hal::i2s::master::Channels = esp_hal::i2s::master::Channels::STEREO;
pub const I2S_SIGNAL_LOOPBACK: bool = true;

// BACKWARD COMPABILITY
pub const SAMPLE_RATE: u32 = 16000;
pub const SAMPLE_COUNT: usize = 256;
pub const BUFFER_SIZE: usize = 4 * 16368;


// SPEAKER / MIC VOLUME CTRL
crate::init_bool!(VOICE_STATE, false);
crate::init_u8!(MIC_VOLUME, 72);
crate::init_u8!(SPEAKER_VOLUME, 58);
crate::init_bool!(MIC_MUTED, false);
crate::init_bool!(SPEAKER_MUTED, false);
crate::init_bool!(MIC_ACTIVE, true);
crate::init_bool!(SPEAKER_TASK_STATE, true);
crate::init_bool!(SPEAKER_ALLOW_STREAMING, true);
crate::init_bool!(AMPLIFIER_STATE, false);
crate::init_bool!(WAKE_WORD_ENABLED, true);
crate::init_bool!(INTERCOM_STATE, false);

// TV
crate::init_bool!(TV_IS_ON, false);
crate::init_bool!(TV_IS_PLAYING, false);
crate::init_u8!(TV_VOLUME, 45);
crate::init_string!(TV_IP, 16);


// MEDIA
crate::init_bool!(MEDIA_IS_PLAYING, false);
crate::init_u8!(MEDIA_COMMAND, 0);

#[derive(Clone, Copy, defmt::Format, PartialEq)]
#[repr(u8)]
pub enum MediaCommand {
    None = 0,
    Prev = 1,
    PlayPause = 2,
    Next = 3,
    Heart = 4,
    Clear = 5,    
}

impl From<u8> for MediaCommand {
    fn from(val: u8) -> Self {
        match val {
            1 => MediaCommand::Prev,
            2 => MediaCommand::PlayPause,
            3 => MediaCommand::Next,
            4 => MediaCommand::Heart,
            5 => MediaCommand::Clear,
            _ => MediaCommand::None,
        }
    }
}


// ───────────────────────────────────────────────────────────────────────
// BUTTONS
crate::init_u8!(BOOT_BUTTON, 0);  
crate::init_u8!(MUTE_BUTTON, 46); 


// ───────────────────────────────────────────────────────────────────────
// CALL RELATED

// MAX CALLER LENGTH
pub const MAX_CALLER_NAME_LEN: usize = 32;

// STORAGE FOR CALLER ID
crate::init_string!(CALLER_NAME, MAX_CALLER_NAME_LEN);
//pub static CALLER_NAME: critical_section::Mutex<core::cell::RefCell<Option<heapless::String<MAX_CALLER_NAME_LEN>>>> = critical_section::Mutex::new(core::cell::RefCell::new(None));


// ───────────────────────────────────────────────────────────────────────
// NOTIFICATION RELATED
crate::init_bool!(ALERT_STATE, false);


// ───────────────────────────────────────────────────────────────────────
// ZIGBEE / HOME AUTOMATION RELATED
crate::init_bool!(LIGHTS_STATE, false);
crate::init_bool!(PC_STATE, false);

