// ★ ─────────────────────────────────────────────────────────────────────── ★
//! ESP32-S3-BOX-3-rs ⮞ https://github.com/QuackHack-McBlindy/ESP32-S3-BOX-3-rs
//!  BARE METAL RUST        - HARDWARE ABSTRACTION LAYER: `esp-hal`
//!   VOICE ASSISTANT FW   - BY QuackHack-McBLindy 🦆🧑‍🦯
// ★ ─────────────────────────────────────────────────────────────────────── ★
//! “A powerful voice assistant can make a huge difference for blind people.
//!   Imagine yourself stumbling blindly across the room looking for the TV remote,
//!   meanwhile, I call the remote using only my voice.
//!   Just to find it and throw it out the window -- because I won't ever need it.“
// ★ ─────────────────────────────────────────────────────────────────────── ★

#![no_std]
#![no_main]

#![allow(
    non_snake_case,
    dead_code,
    unused,
    private_interfaces,
    clippy::large_stack_frames,
    reason = "NOBODY TELLS ME WHAT TO DO!"
)]

use esp_println as _;

// PANIC HANDLER
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    defmt::error!("⚠️ PANIC: {}", defmt::Debug2Format(info));
    defmt::error!("⚠️ REBOOT DEVICE!");
    loop {} // REBOOT BY HOLDING POWER BUTTON!
}

// MEMORY
extern crate alloc;

// BOOTLOADER (REQUIRED TO BOOT WITHOUT ESP-IDF)
esp_bootloader_esp_idf::esp_app_desc!();


// LOAD MODULES
mod state;
mod components;
mod base;
mod gui;
mod applications;


// SHARED RESOURCES
pub static ES7210: critical_section::Mutex<core::cell::RefCell<core::option::Option<es7210::Es7210>>> =
    critical_section::Mutex::new(core::cell::RefCell::new(core::option::Option::None));
pub static ES8311: critical_section::Mutex<core::cell::RefCell<core::option::Option<es8311::Es8311>>> =
    critical_section::Mutex::new(core::cell::RefCell::new(core::option::Option::None));
pub static I2C_BUS: critical_section::Mutex<core::cell::RefCell<core::option::Option<I2cBus>>> = 
    critical_section::Mutex::new(core::cell::RefCell::new(core::option::Option::None));
pub static AMP_PIN: critical_section::Mutex<core::cell::RefCell<core::option::Option<esp_hal::gpio::Output<'static>>>> = 
    critical_section::Mutex::new(core::cell::RefCell::new(core::option::Option::None));

type I2cBus = esp_hal::i2c::master::I2c<'static, esp_hal::Blocking>;


// CHANNELS
pub static DISPLAY_CMD: embassy_sync::channel::Channel<embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, DisplayCommand, 1>
    = embassy_sync::channel::Channel::new();

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayCommand {
    Start,
    Stop,
}


// ───────────────────────────────────────────────────────────────────────
// CONSTRUCT THE VOICE HANDLER
struct VoiceHandler;

// CONFIGURE VOICE EVENTS (WAKE-WORD DETECTION)
// BACKEND RESPONDS WITH SINGLE-BYTE EVENT CODES
// FOR A LOW-OVERHEAD EMBEDDED NETWORK PROTOCOL
impl yo_esp::CommandHandler for VoiceHandler {
    // 0x01 === WAKE WORD DETECTED
    fn on_detected(&mut self) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + '_>> {
        alloc::boxed::Box::pin(async {
            // TURN ON DISPLAY & PLAY SOUND
            crate::components::display::display_on();
            yo_esp::play_ding().await;            
        })
    }

    // 0x02 === SERVER STARTED TRANSCRIPTION
    fn on_thinking(&mut self) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + '_>> {
        alloc::boxed::Box::pin(async {
            // FLASH DISPLAY WHILE “THINKING“
           crate::components::display::flash(10);
        })
    }

    // 0x03 === COMMAND EXECUTED SUCCESSFULLY
    fn on_executed(&mut self, _ms: core::option::Option<u64>) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + '_>> {
        alloc::boxed::Box::pin(async move {
            // PLAY SUCCESS SOUND
            yo_esp::play_done().await;
        })
    }

    // 0x04 === FAILED COMMAND EXECUTION
    fn on_failed(&mut self, _ms: core::option::Option<u64>) -> core::pin::Pin<alloc::boxed::Box<dyn core::future::Future<Output = ()> + '_>> {
        alloc::boxed::Box::pin(async move {
            // LET THE 🦆 SAY “FUCK!“
            yo_esp::play_fail().await;
        })
    }
}


// ───────────────────────────────────────────────────────────────────────
// FUNCTION TO CONTROL SPEAKER VOLUME (0-100%)
pub fn set_speaker_volume(volume: u8) {
    let volume = volume.min(100);
    crate::store!(crate::state::SPEAKER_VOLUME, volume);
    let was_muted = crate::load!(crate::state::SPEAKER_MUTED);

    if volume == 0 {
        // MIGHT AS WELL MUTE THE ES8311 CODEC HERE - POWER SAVER!
        critical_section::with(|cs| {
            let mut bus = crate::I2C_BUS.borrow_ref_mut(cs);
            let mut codec = crate::ES8311.borrow_ref_mut(cs);
            if let (core::option::Option::Some(i2c), core::option::Option::Some(es8311)) = (bus.as_mut(), codec.as_mut()) {
                let _ = es8311.mute(i2c, true);
                // AND SET CODEC TO FULL STANDBY (~0 µA)
                let _ = es8311.set_power_mode(i2c, es8311::PowerMode::Standby);
            }
        });
        // WE DON'T NEED AMPLIFIER ON IF WE ARE NOT OUPUTTING SOUND
        amp_off();
        defmt::info!("🔇 Speaker MUTED!");
        crate::store!(crate::state::SPEAKER_MUTED, true);
    } else { // ABOVE ZERO
        if was_muted { // CHECK IF LAST STATE WAS MUTE 
            // WAKE UP FROM STANDBY IF PREVIOUSLY MUTED 
            critical_section::with(|cs| {
                let mut bus = crate::I2C_BUS.borrow_ref_mut(cs);
                let mut codec = crate::ES8311.borrow_ref_mut(cs);
                if let (core::option::Option::Some(i2c), core::option::Option::Some(es8311)) = (bus.as_mut(), codec.as_mut()) {
                    // REBUILD CODEC CONFIG
                    let mclk_freq = crate::state::I2S_SAMPLE_RATE * 256;  // mclk_ratio = 256, as in main
                    let clock_cfg = es8311::ClockConfig {
                        mclk_inverted: false,
                        sclk_inverted: false,
                        mclk_from_mclk_pin: true,
                        mclk_frequency: mclk_freq,
                        sample_frequency: crate::state::I2S_SAMPLE_RATE,
                    };
                    let resolution = match crate::state::I2S_BIT_WIDTH {
                        16 => es8311::Resolution::Bits16,
                        24 => es8311::Resolution::Bits24,
                        32 => es8311::Resolution::Bits32,
                        _ => es8311::Resolution::Bits16,
                    };
                    let mut delay = esp_hal::delay::Delay::new();
                    // RE-INIT CODEC
                    if let Err(e) = es8311.init(i2c, &clock_cfg, resolution, resolution, &mut delay) {
                        defmt::error!("ES8311 wake‑up failed: {:?}", defmt::Debug2Format(&e));
                        return;
                    }
                    // UNMUTE
                    let _ = es8311.mute(i2c, false);
                }
            });
            // TURN ON AMPLIFIER AFTER CODEC IS READY
            amp_on();
            crate::store!(crate::state::SPEAKER_MUTED, false);
        }

        // SET NEW VOLUME
        critical_section::with(|cs| {
            let mut bus = crate::I2C_BUS.borrow_ref_mut(cs);
            let mut codec = crate::ES8311.borrow_ref_mut(cs);
            if let (core::option::Option::Some(i2c), core::option::Option::Some(es8311)) = (bus.as_mut(), codec.as_mut()) {
                let _ = es8311.volume_set(i2c, volume, core::option::Option::None);
            }
        });
        defmt::info!("🔊 Volume {}%", volume);
    }
}

// ───────────────────────────────────────────────────────────────────────
// SCHEDULE A MUTE AFTER A GIVEN NUMBER OF SECONDS
pub async fn mute_in(seconds: u64) {
    embassy_time::Timer::after(embassy_time::Duration::from_secs(seconds)).await;
    set_speaker_volume(0);
}

// ───────────────────────────────────────────────────────────────────────
// FUNCTION TO CONTROL MICROPHONE GAIN (0-100%)
pub fn set_mic_gain(percent: u8) {
    let percent = percent.min(100);
    crate::store!(crate::state::MIC_VOLUME, percent);
    // 0  % === -95 dB
    // 100% === +32 dB
    let db = -95.0 + (127.0 * percent as f32 / 100.0);
    let db_i8 = db as i8;

    critical_section::with(|cs| {
        let mut bus = crate::I2C_BUS.borrow_ref_mut(cs);
        let mut codec = crate::ES7210.borrow_ref_mut(cs);

        if let (core::option::Option::Some(i2c), core::option::Option::Some(es7210)) = (bus.as_mut(), codec.as_mut()) {
            if percent == 0 { // MIGHT AS WELL MUTE THE ES7210 CODEC HERE - SAVES US A FEW mV
                defmt::info!("🎙️⛔ Mic MUTED!");
            } else { defmt::info!("🎙️ Gain {}%", percent); }
            let _ = es7210.gain_set(i2c, db_i8);
        }
    });
}

// ───────────────────────────────────────────────────────────────────────
// FUNCTIONS TO TURN ON/OFF THE AMPLIFIER
pub fn amp_on() {
    critical_section::with(|cs| {
        if let core::option::Option::Some(pin) = AMP_PIN.borrow_ref_mut(cs).as_mut() {
            pin.set_high();
        }
    }); defmt::info!("📢 ☑️");
    crate::store!(crate::state::AMPLIFIER_STATE, true);
}

pub fn amp_off() {
    critical_section::with(|cs| {
        if let core::option::Option::Some(pin) = AMP_PIN.borrow_ref_mut(cs).as_mut() {
            pin.set_low();
        }
    }); defmt::info!("📢 ❌");
    crate::store!(crate::state::AMPLIFIER_STATE, false);
}


// ───────────────────────────────────────────────────────────────────────
// DISPLAY TASK
use embassy_time::{Duration, Instant, Timer};
use embedded_graphics_core::pixelcolor::RgbColor;
use embedded_graphics_core::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Point},
    pixelcolor::Rgb565,
    primitives::Rectangle,
};
use esp_hal::ledc::channel::ChannelIFace;
use embedded_graphics_core::pixelcolor::raw::RawU16;


#[embassy_executor::task]
async fn display_task(
    mut fb: crate::components::framebuffer::Framebuffer,
    mut display: mipidsi::Display<
        display_interface_spi::SPIInterface<
            embedded_hal_bus::spi::ExclusiveDevice<
                esp_hal::spi::master::Spi<'static, esp_hal::Blocking>,
                esp_hal::gpio::Output<'static>,
                esp_hal::delay::Delay,
            >,
            esp_hal::gpio::Output<'static>,
        >,
        mipidsi::models::ST7789,
        esp_hal::gpio::Output<'static>,
    >,
    backlight: &'static mut esp_hal::ledc::channel::Channel<'static, esp_hal::ledc::LowSpeed>,
) {
    let mut current_brightness: u8 = 0;
    backlight.set_duty(0).ok();
    crate::store!(crate::state::DISPLAY_STATE, false);

    loop {
        crate::store!(crate::state::DISPLAY_STATE, false);
        loop {
            match crate::DISPLAY_CMD.receive().await {
                DisplayCommand::Start => break,
                _ => continue,
            }
        }

        let init_brightness = crate::load!(crate::state::DISPLAY_BRIGHTNESS);
        backlight.set_duty(init_brightness).ok();
        current_brightness = init_brightness;
        crate::store!(crate::state::DISPLAY_STATE, true);

        let timeout_secs = crate::load!(crate::state::DISPLAY_TIMEOUT_SECS) as u64;
        let mut render_deadline = Instant::now() + Duration::from_secs(timeout_secs);
        let mut last_page: Option<crate::gui::pages::Page> = None;

        loop {
            // CHECK FOR STOP COMMAND
            if let Ok(DisplayCommand::Stop) = crate::DISPLAY_CMD.try_receive() {
                break;
            }

            if crate::load!(crate::state::DISPLAY_TOUCH_ACTIVITY) {
                render_deadline = Instant::now() + Duration::from_secs(timeout_secs);
                crate::store!(crate::state::DISPLAY_TOUCH_ACTIVITY, false);
            }

            // UPDATE BRIGHTNESS IF CHANGED
            let desired = crate::load!(crate::state::DISPLAY_BRIGHTNESS);
            if desired != current_brightness {
                backlight.set_duty(desired).ok();
                current_brightness = desired;
            }

            let page = crate::gui::pages::current_page();


            let redraw = Some(page) != last_page;

            if redraw {
                fb.clear_color(Rgb565::BLACK);
                fb.buffer_mut().fill(0xF800);
                DrawTarget::fill_contiguous(
                    &mut display,
                    &Rectangle::new(Point::new(0, 0), OriginDimensions::size(&fb)),
                    fb.buffer().iter().map(|&p| Rgb565::from(RawU16::new(p))),
                ).unwrap();
                last_page = Some(page);
            }

            crate::dirty!();
            
            let delay_fut = Timer::after(Duration::from_millis(200));
            let cmd_fut = crate::DISPLAY_CMD.receive();
            match embassy_futures::select::select(delay_fut, cmd_fut).await {
                embassy_futures::select::Either::Second(DisplayCommand::Stop) => break,
                _ => {}
            }
            

            if Instant::now() >= render_deadline {
                break;
            }
        }

        backlight.set_duty(0).ok();
        current_brightness = 0;
        crate::store!(crate::state::DISPLAY_STATE, false);
    }
}


// ───────────────────────────────────────────────────────────────────────
// MAIN
#[allow(clippy::large_stack_frames)]
#[esp_rtos::main]
async fn main(spawner: embassy_executor::Spawner) -> ! {
    // WE WILL CONTROL CPU CLOCK LATER (ALSO AVAILABLE VIA GUI)
    let config = esp_hal::Config::default().with_cpu_clock(esp_hal::clock::CpuClock::max());
    let peripherals = esp_hal::init(config);

    // ALLOCATE EXTERNAL PSEUDO STATIC RANDOM ACCESS MEMORY
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram);

    // INTERNAL DRAM HEAP
    esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);

    let mut storage = esp_storage::FlashStorage::new();

    // MEDIA PLAYERS HAS HEAVY SPLIT ANIMATION & MULTIPLE IMAGES
    // CACHE & INIT THE MEDIA PLAYERS EARLY (BEFORE I2S)
   // crate::gui::duck_tv::init();
    // & SET A DEFAULT TV IP
    crate::set_string!(crate::state::TV_IP, "192.168.1.224");

    // SOFTWARE INTERRUPT SETUP
    let _sw_ints = esp_hal::interrupt::software::SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);
    let sw_int0 = unsafe { esp_hal::interrupt::software::SoftwareInterrupt::steal() };
    let timg0 = esp_hal::timer::timg::TimerGroup::new(peripherals.TIMG0);
    esp_rtos::start(timg0.timer0, sw_int0);

    // TRACK TIME SINCE BOOT FOR DEVICE UPTIME CALCULATION
    let boot_time = embassy_time::Instant::now();

    // RANDOM
    let rng = esp_hal::rng::Rng::new();
    let seed: u64 = (u64::from(rng.random())) << 32 | u64::from(rng.random());
    crate::base::rng::init(rng);


    // ───────────────────────────────────────────────────────────────────────
    // BUTTONS
    let button_top_left = esp_hal::gpio::Input::new(
        peripherals.GPIO0,
        esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up)
    );

    // DISABLE (LOW) POWER AMPLIFIER 
    // WE ENABLE (HIGH) LATER
    // TO AVOID THE SCARY POP SOUND
    let mut amp = esp_hal::gpio::Output::new(
        peripherals.GPIO46,
        esp_hal::gpio::Level::Low,
        esp_hal::gpio::OutputConfig::default()
    );
    
    // LET'S MAKE THE AMP A SHARED RESOURCE
    // SO THE PUBLIC FUNCTIONS CAN SET HIGH/LOW
    critical_section::with(|cs| {
        *AMP_PIN.borrow_ref_mut(cs) = core::option::Option::Some(amp);
    });


    // GPIO38 IS A TOUCH INTERUPT PIN - HIGH (PULL-UP) BY DEFAULT
    // PULLED LOW BY THE TOUCH CONTROLLER UPON TOUCH
    // WHEN A FINGER IS ON THE SCREEN WE USE IT AS AN WAKE-UP CALL
    let mut touch_int = esp_hal::gpio::Input::new(
        peripherals.GPIO38,
        esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Up)
    ); 
    
    let occupancy = esp_hal::gpio::Input::new(
        peripherals.GPIO21,
        esp_hal::gpio::InputConfig::default().with_pull(esp_hal::gpio::Pull::Down)
    );

    // ───────────────────────────────────────────────────────────────────────
    // ADC / BATTERY
    let mut adc_config = esp_hal::analog::adc::AdcConfig::new();
    let battery_pin = peripherals.GPIO10;
    let mut adc_pin = adc_config.enable_pin(battery_pin, esp_hal::analog::adc::Attenuation::_0dB);
    let mut adc = esp_hal::analog::adc::Adc::new(peripherals.ADC1, adc_config);

    // ───────────────────────────────────────────────────────────────────────
    // I2C BUS A - AUDIO CODECS & TOUCH 
    let i2c_a = esp_hal::i2c::master::I2c::new(
        peripherals.I2C0,
        esp_hal::i2c::master::Config::default()
            .with_frequency(esp_hal::time::Rate::from_khz(100)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO8)
    .with_scl(peripherals.GPIO18);
    
    // STOREE BUS GLOBALLY
    critical_section::with(|cs| {
        *crate::I2C_BUS.borrow(cs).borrow_mut() = Some(i2c_a);
    });


    // ───────────────────────────────────────────────────────────────────────
    // I2C BUS B - AHT20
    let i2c_b = esp_hal::i2c::master::I2c::new(
        peripherals.I2C1,
        esp_hal::i2c::master::Config::default()
            .with_frequency(esp_hal::time::Rate::from_khz(50)),
    )
    .unwrap()
    .with_sda(peripherals.GPIO41)
    .with_scl(peripherals.GPIO40);
    
    // STORE FOR AHT20
    let i2c_b_mutex = alloc::boxed::Box::leak(alloc::boxed::Box::new(
        critical_section::Mutex::new(core::cell::RefCell::new(i2c_b)),
    ));
    
    // AUDIO CODEC & TOUCH CONTROLLER INIT
    let es7210 = es7210::Es7210::new(0x40);
    let es8311 = es8311::Es8311::new(0x18);
    let touch_int = peripherals.GPIO3;

    
    // I2C INIT INSIDE CS, BORROW GLOBAL BUS
    critical_section::with(|cs| {
        let mut i2c_ref = crate::I2C_BUS.borrow_ref_mut(cs);
        let i2c = i2c_ref.as_mut().expect("I2C bus not available during init");
    
        // ES7210 (ADC)
        let codec_cfg = es7210::CodecConfig {
            sample_rate_hz: 16000,
            mclk_ratio: 256,
            i2s_format: es7210::I2sFormat::I2S,
            bit_width: es7210::I2sBits::Bits16,
            mic_bias: es7210::MicBias::V2_87,
            mic_gain: es7210::MicGain::Gain30dB,
            tdm_enable: false,
        };
        match es7210.config_codec(i2c, &codec_cfg) {
            Ok(()) => defmt::info!("ES7210 initialized successfully"),
            Err(e) => defmt::info!("ES7210 init failed: {:?}", defmt::Debug2Format(&e)),
        }
        if let Err(e) = es7210.gain_set(i2c, 20) {
            defmt::info!("ES7210 volume set failed: {:?}", defmt::Debug2Format(&e));
        }
        if let Err(e) = es7210.set_mute(i2c, false) {
            defmt::info!("Failed to configure ES7210 mute status {:?}", defmt::Debug2Format(&e));
        }
    
        // ES8311 (DAC)
        let clock_cfg = es8311::ClockConfig {
            mclk_inverted: false,
            sclk_inverted: false,
            mclk_from_mclk_pin: true,
            mclk_frequency: 4096000,
            sample_frequency: 16000,
        };
        let mut delay = esp_hal::delay::Delay::new();
        match es8311.init(
            i2c,
            &clock_cfg,
            es8311::Resolution::Bits16,
            es8311::Resolution::Bits16,
            &mut delay,
        ) {
            Ok(()) => defmt::info!("ES8311 initialised successfully"),
            Err(e) => defmt::info!("ES8311 init failed: {:?}", defmt::Debug2Format(&e)),
        }
        let _ = es8311.volume_set(i2c, 80, core::option::Option::None);
        let _ = es8311.mute(i2c, false);



        // GT911 (TOUCH)
      //  let touch = crate::components::gt911::Gt911Blocking::default();
       // touch.init(i2c).unwrap();
    
        // STORE INIT CODECS IN GLOBALS
        *crate::ES7210.borrow(cs).borrow_mut() = Some(es7210);
        *crate::ES8311.borrow(cs).borrow_mut() = Some(es8311);
    });
    
  
    // ───────────────────────────────────────────────────────────────────────
    // LEDC / BACKLIGHT
    use esp_hal::ledc::channel::ChannelIFace;
    let backlight = peripherals.GPIO47;

    let ledc = mk_static!(esp_hal::ledc::Ledc, esp_hal::ledc::Ledc::new(peripherals.LEDC));
    ledc.set_global_slow_clock(esp_hal::ledc::LSGlobalClkSource::APBClk);
    
    // LOW SPEED TIMER FOR 24 kHz WITH 10‑BIT DUTY RESOLUTION
    let lstimer0 = mk_static!(
        esp_hal::ledc::timer::Timer<'static, esp_hal::ledc::LowSpeed>,
        ledc.timer::<esp_hal::ledc::LowSpeed>(esp_hal::ledc::timer::Number::Timer0)
    );
    esp_hal::ledc::timer::TimerIFace::configure(
        lstimer0,
        esp_hal::ledc::timer::config::Config {
            duty: esp_hal::ledc::timer::config::Duty::Duty10Bit,
            clock_source: esp_hal::ledc::timer::LSClockSource::APBClk,
            frequency: esp_hal::time::Rate::from_khz(24),
        },
    )
    .unwrap();
    
    // CREATE A CHANNEL AND ASSIGN IT TO THE TIMER AND GPIO 47
    let mut channel0 = ledc.channel(
        esp_hal::ledc::channel::Number::Channel0,
        backlight,
    );
    channel0
        .configure(esp_hal::ledc::channel::config::Config {
            timer: lstimer0,
            duty_pct: 0,
            drive_mode: esp_hal::gpio::DriveMode::PushPull,
        })
        .unwrap();
    
    // LEAK THE CHANNEL TO GET STATIC MUT
    let backlight_channel: &'static mut _ = alloc::boxed::Box::leak(alloc::boxed::Box::new(channel0));  


    // ───────────────────────────────────────────────────────────────────────
    // DISPLAY 
    let spi = esp_hal::spi::master::Spi::new(
        peripherals.SPI2,
        esp_hal::spi::master::Config::default()
            .with_frequency(esp_hal::time::Rate::from_mhz(40))
            .with_mode(esp_hal::spi::Mode::_0),
    )
    .unwrap()
    .with_sck(peripherals.GPIO7)
    .with_mosi(peripherals.GPIO6)
    .with_miso(esp_hal::gpio::NoPin);

    let cs = esp_hal::gpio::Output::new(peripherals.GPIO5, esp_hal::gpio::Level::High, esp_hal::gpio::OutputConfig::default());
    let dc = esp_hal::gpio::Output::new(peripherals.GPIO4, esp_hal::gpio::Level::Low, esp_hal::gpio::OutputConfig::default());
    let rst = esp_hal::gpio::Output::new(peripherals.GPIO48, esp_hal::gpio::Level::Low, esp_hal::gpio::OutputConfig::default());

    let spi_dev = embedded_hal_bus::spi::ExclusiveDevice::new(spi, cs, esp_hal::delay::Delay::new()).unwrap();
    let mut delay = esp_hal::delay::Delay::new();

    let interface = display_interface_spi::SPIInterface::new(spi_dev, dc);


    let mut display = mipidsi::Builder::new(mipidsi::models::ST7789, interface)
        .reset_pin(rst) 
        .orientation(mipidsi::options::Orientation::new()
            .rotate(mipidsi::options::Rotation::Deg0))
        .init(&mut delay)
        .unwrap();

    let mut fb = crate::components::framebuffer::Framebuffer::new();


    esp_hal::ledc::channel::ChannelIFace::set_duty(backlight_channel, 100);

    // ───────────────────────────────────────────────────────────────────────
    // SETUP WIFI (ON LOW-POWER MODE)
    let backend_port: u16 = crate::state::BACKEND_TCP_PORT_STR.parse().expect("Invalid BACKEND_TCP_PORT");    
    let stack = crate::base::wifi::init(&spawner, peripherals.WIFI, backend_port).await;
         
    // WIFI CONFIGURED TO SIT IDLE AND AWAIT START/STOP COMMANDS
    // VOICE COMMUNICATION REQUIRES LOCAL NETWORK - START IT UP! (CAN BE TOGGLED AT RUNTIME)
    crate::base::wifi::WIFI_CMD.send(crate::base::wifi::WifiCommand::Enable).await;
    crate::store!(crate::state::WIFI_STATE, true);


    // ───────────────────────────────────────────────────────────────────────
    // I2S AUDIO SETUP 
    let (_rx_buffer, rx_descriptors, tx_buffer, tx_descriptors) = esp_hal::dma_buffers!(crate::state::I2S_BUFFER_SIZE);

    let i2s = esp_hal::i2s::master::I2s::new(
        peripherals.I2S0,
        peripherals.DMA_CH0,
        esp_hal::i2s::master::Config::new_tdm_philips()
            .with_signal_loopback(true)
            .with_sample_rate(esp_hal::time::Rate::from_hz(16000))
            .with_data_format(esp_hal::i2s::master::DataFormat::Data16Channel16)
            .with_endianness(esp_hal::i2s::master::Endianness::LittleEndian) 
            .with_channels(esp_hal::i2s::master::Channels::STEREO),            
    )
    .unwrap()
    .into_async()
    .with_mclk(peripherals.GPIO2);

    // AUDIO OUTPUT
    // BUILD I2S TX (MASTER) WITH BCLK, LRCLK AND DIGITAL OUT PINS 
    let i2s_tx = i2s.i2s_tx
        .with_bclk(peripherals.GPIO17)
        .with_ws(peripherals.GPIO45)
        .with_dout(peripherals.GPIO15)
        .build(tx_descriptors);

    // AUDIO INPUT
    // BUILD I2S RX (SLAVE) WITH DIGITAL-IN PIN 
    let i2s_rx = i2s
        .i2s_rx
        .with_din(peripherals.GPIO16)
        .build(rx_descriptors);

    // I2S TX CIRCULAR WRITE
    // CONTINUOSLY WRITE TO I2S TX TO KEEP CLOCKS UP FOR RX (SLAVE)
    let tx_transfer = match i2s_tx.write_dma_circular_async(tx_buffer) {
        Ok(t) => t,
        Err(e) => {
            defmt::error!("I2S circular TX failed: {:?}", defmt::Debug2Format(&e));
            panic!("I2S setup error");
        }
    };
    
    // INIT YO HANDLER (OUR VOICE COMMAND HANDLER)
    let handler: alloc::boxed::Box<dyn yo_esp::CommandHandler> = alloc::boxed::Box::new(VoiceHandler);

    // ───────────────────────────────────────────────────────────────────────
    // INIT ENDPOINT ROUTES FOR THE INTERNAL API
    crate::base::api::init_routes().await;

    // ───────────────────────────────────────────────────────────────────────
    // BOOT PROCESS COMPLETE
    // PRINT OS INFORMATION & INIT TASKS
    defmt::info!("╬═══════════════════════════════╬");
    defmt::info!("╬ STARTED {} v{} ╬",
        crate::state::PROJECT_NAME,
        crate::state::FW_VERSION
    ); defmt::info!("╬═══════════════════════════════╬");

    // ───────────────────────────────────────────────────────────────────────
    // TASKS
  
    // SPEAKER TASK (WRITES AUDIO DATA INTO PIPE + KEEP CLOCKS UP FOR MIC)
    // TASK STARTS IDLE AND WAITS FOR A COMMAND 
    crate::spawn!(spawner, yo_esp::speaker_task(tx_transfer));
    // WE START IT HERE - TO AVOID LATE DMA!
    yo_esp::SPEAKER_CMD.send(yo_esp::SpeakerCommand::Start).await;
    crate::store!(crate::state::SPEAKER_TASK_STATE, true);
    
    // STREAMING SPEAKER TASK (STREAM AUDIO TO THE SPEAKER OVER TCP PORT 12345)
    // (IDLE - SEND START/STOP COMMAND)
    // AUTO STARTED ON WIFI CONNECTION
    crate::spawn!(spawner, yo_esp::stream_speaker(stack, backend_port));
        
    // MICROPHONE TASK (STREAMS AUDIO TO BACKEND OVER TCP PORT 12345)
    // (SLEEPS UNLESS WAKE-WORD ENABLED/BUTTON IS PRESSED)
    crate::spawn!(spawner, yo_esp::audio_capture_task(i2s_rx, stack, crate::state::BACKEND_TCP_HOST, backend_port, "esp", handler));

    // HTTP API & WEB SERVER TASK (PORT 80)
    // (IDLE - SEND START/STOP COMMAND)
    // AUTO STARTED ON WIFI CONNECTION
    crate::spawn!(spawner, tinyapi::web_server_task(stack));
     
    // SENSOR TASK (TEMPERATURE/HUMIDITY)
    crate::spawn!(spawner, crate::components::aht20::sensor_task(i2c_b_mutex));

    // PRESENCE SENSOR TASK
    crate::spawn!(spawner, crate::components::presence::occupancy_task(occupancy));

    // START TINYWEATHER TASK IN THE BACKGROUND
    crate::spawn!(spawner, crate::applications::tinyweather::weather_task(stack));

    // START THE SMART HOME TASK
    crate::spawn!(spawner, crate::applications::zigduck::smart_home_task(stack));
    
    // START THE DUCK-TV TASK
    crate::spawn!(spawner, crate::applications::duck_tv::tv_task(stack));

    // BUTTON MONITORING TASK
    crate::spawn!(spawner, crate::components::buttons::button_task(button_top_left));

    // DISPLAY TASK
    crate::spawn!(spawner, display_task(fb, display, backlight_channel));

    // TOUCH TASK
    crate::spawn!(spawner, crate::gui::pages::touch_task());



    // IT'S NOW SAFE TO CRANK UP THE AMP
    // WITH NO LOAD POPPIN' NOISE
    crate::amp_on();

    // ───────────────────────────────────────────────────────────────────────
    crate::delay_s!(2);    
 

    // MAIN LOOP 
    loop { // CALCULATE UPTIME
        let elapsed = embassy_time::Instant::now() - boot_time;
        let uptime_secs = elapsed.as_secs() as u32;
        let days = elapsed.as_secs() / 86400;
        let hours = (elapsed.as_secs() % 86400) / 3600;
        let minutes = (elapsed.as_secs() % 3600) / 60;
        // & STORE IT
        crate::store!(crate::state::UPTIME_SECS, uptime_secs);

        // +1 MINUTE TO CURRENT TIME
        critical_section::with(|cs| {
            let time_cell = crate::state::CURRENT_TIME.borrow(cs);
            if let Some(mut dt) = time_cell.get() {
                crate::base::time::up_one_min(&mut dt);
                time_cell.set(Some(dt));
            }
        });

        // PRINT TIME + UPTIME
        if days > 0 {
            if hours > 0 {
                defmt::info!("⏱️  {}D {:02}H {:02}M uptime", days, hours, minutes);
            } else { defmt::info!("⏱️  {}D {:02}M uptime", days, minutes); }
        } else if hours > 0 {
            defmt::info!("⏱️  {:02}H {:02}M uptime", hours, minutes);
        } else { defmt::info!("⏱️  {:02}M uptime", minutes); }
        let maybe_time = critical_section::with(|cs| crate::state::CURRENT_TIME.borrow(cs).get());
        if let Some(dt) = maybe_time { defmt::info!("⏰ {:02}:{:02}", dt.hours, dt.minutes); }    
    
        // CALIBRATE BATTERY READ WITH PREDEFINED MIN/MAX mV VALUES
        const EMPTY_BATTERY: u32 = 1200;
        const FULL_BATTERY: u32 = 1980;
        const EMPTY_BATTERY_CHARGING: u32 = 4200;
        const FULL_BATTERY_CHARGING: u32 = 4521;

        // READ ADC
        let raw = adc.read_blocking(&mut adc_pin);
        let pin_voltage = raw as f32 * 1100.0 / 4095.0 / 1000.0;
        let battery_voltage = pin_voltage * 4.11;
        let voltage_mv = (battery_voltage * 1000.0) as u32;

        let usb_connected = battery_voltage > 4.2;

        let (empty, full) = if usb_connected {
            (EMPTY_BATTERY_CHARGING, FULL_BATTERY_CHARGING)
        } else { (EMPTY_BATTERY, FULL_BATTERY) };

        let raw_pct = (voltage_mv as i32 - empty as i32) * 100
                    / (full as i32 - empty as i32);
        let percentage = raw_pct.clamp(0, 100) as u8;

        // STORE ATOMIC VARS
        crate::store!(crate::state::BATTERY_VOLTAGE, voltage_mv);
        crate::store!(crate::state::BATTERY_PERCENT, percentage);
        crate::store!(crate::state::BATTERY_USB_CONNECTED, usb_connected);
        crate::store!(crate::state::BATTERY_FULL, percentage == 100);
        crate::store!(crate::state::BATTERY_NEED_CHARGING, percentage < 25);



        // PRINT WIFI SIGNAL & BATTERY INFO
        let emoji = match (percentage, usb_connected) {
            (0..=10, false) => "🪫⚠️",
            (0..=10, true)  => "🪫⚡",
            (11..=29, false) => "🪫",
            (11..=29, true)  => "🪫⚡",
            (30..=70, false) => "🔋",
            (30..=70, true)  => "🔋⚡",
            (_, false)       => "🔋",
            (_, true)        => "🔋⚡",
        };        

        if crate::load!(crate::state::WIFI_CONNECTED) {
            let rssi = crate::load!(crate::state::RSSI);
            let rssi_percent = (rssi + 100) * 100 / 70;
            let rssi_percent = rssi_percent.clamp(0, 100);
            defmt::info!("🛜 {} dBm ({}%)", rssi, rssi_percent);
        }; defmt::info!("{} {}% ({} mV)", emoji, percentage, voltage_mv);
        
        // DISPLAY IS NOW DIRTY
        crate::dirty!();

        // SLEEP 60 SECONDS AND RERUN LOOP
        crate::delay_s!(60);
        // THE END!
    } // 🦆🧑‍🦯 thank you for quackin' along!
    // if you found this helpful - please concider buying me a coffee 
} // ☕ ⮞ https://buymeacoffee.com/quackhackmcblindy

