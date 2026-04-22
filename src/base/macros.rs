use critical_section;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use esp_hal::ledc::{LowSpeed, channel::{Channel, ChannelIFace}};


#[macro_export]
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: static_cell::StaticCell<$t> = static_cell::StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write($val);
        x
    }};
}


#[macro_export]
macro_rules! static_mutex {
    ($mutex_type:ty, $value:expr) => {{
        let value = $value;
        let mutex = Box::leak(Box::new(<$mutex_type>::new(value)));
        mutex
    }};
}



#[macro_export]
macro_rules! env_def {
    ($name:expr, $default:expr) => {
        match option_env!($name) {
            Some(val) => val,
            None => $default,
        }
    };
}


#[macro_export]
macro_rules! gpio_input {
    ($pin:expr, $pull:expr) => {{
        use esp_hal::gpio::{Input, InputConfig, Pull};
        Input::new($pin, InputConfig::default().with_pull($pull))
    }};
}


#[macro_export]
macro_rules! gpio_output {
    ($pin:expr, $initial_level:expr) => {{
        use esp_hal::gpio::{Output, OutputConfig, Level};
        Output::new($pin, $initial_level, OutputConfig::default())
    }};
}

macro_rules! display_brightness {
    ($channel:expr, $percent:expr) => {{
        let percent = $percent.clamp(0, 100);
        $channel.set_duty_percent(percent).unwrap();
    }};
}

