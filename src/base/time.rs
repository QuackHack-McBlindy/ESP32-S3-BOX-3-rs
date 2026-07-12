// BASE/TIME
// NTP TIME KEEPING (NO RTC)

#[derive(Debug, Clone, Copy)]
pub struct DateTime {
    pub seconds: u8,
    pub minutes: u8,
    pub hours: u8,
    pub day: u8,
    pub weekday: u8,
    pub month: u8,
    pub year: u8, // 0-99 (2000-2099)
}

impl DateTime {
    pub fn new(year: u8, month: u8, day: u8, hours: u8, minutes: u8, seconds: u8) -> Self {
        Self {
            seconds,
            minutes,
            hours,
            day,
            weekday: 0,
            month,
            year,
        }
    }
}




// NTP SYNC
// SYNCRONIZE RTC TO NTP POOL
pub async fn ntp_sync(stack: &embassy_net::Stack<'static>) -> Result<(), &'static str> {
    let mut rx_meta = [embassy_net::udp::PacketMetadata::EMPTY; 1];
    let mut rx_buf = [0u8; 256];
    let mut tx_meta = [embassy_net::udp::PacketMetadata::EMPTY; 1];
    let mut tx_buf = [0u8; 256];

    let mut socket = embassy_net::udp::UdpSocket::new(
        stack.clone(), &mut rx_meta, &mut rx_buf, &mut tx_meta, &mut tx_buf
    );
    socket.bind(23456).map_err(|_| "bind failed")?;

    let mut ntp_request = [0u8; 48];
    ntp_request[0] = 0x1B;
    let ntp_addr = embassy_net::Ipv4Address::new(216, 239, 35, 0);
    socket.send_to(&ntp_request, (ntp_addr, 123)).await.map_err(|_| "send failed")?;

    let mut response = [0u8; 48];
    match embassy_time::with_timeout(embassy_time::Duration::from_secs(5), socket.recv_from(&mut response)).await {
        Ok(Ok((len, _addr))) if len >= 48 => {
            let ntp_secs = u32::from_be_bytes([response[40], response[41], response[42], response[43]]);
            let unix_secs = ntp_secs.wrapping_sub(2_208_988_800) as i64;
            let local_secs = unix_secs + timezone_offset(unix_secs) as i64;

            let (year, month, day, hour, minute, second) = unix_to_datetime(local_secs);
            defmt::debug!("NTP time: {:04}-{:02}-{:02} {:02}:{:02}:{:02}", year, month, day, hour, minute, second);

            critical_section::with(|cs| {
                let dt = DateTime {
                    seconds: second as u8,
                    minutes: minute as u8,
                    hours: hour as u8,
                    day: day as u8,
                    weekday: 0,
                    month: month as u8,
                    year: (year % 100) as u8,
                };
                crate::state::CURRENT_TIME.borrow(cs).set(Some(dt));
            });
            Ok(())
        }
        _ => Err("NTP response timeout or invalid"),
    }
}

pub fn up_one_min(dt: &mut crate::base::time::DateTime) {
    dt.seconds = 0;
    dt.minutes += 1;
    if dt.minutes < 60 {
        return;
    }
    dt.minutes = 0;
    dt.hours += 1;
    if dt.hours < 24 {
        return;
    }
    dt.hours = 0;
}

// RETURN OFFSET IN SECONDS (POSITIVE FOR UTC+1 OR +2)
fn timezone_offset(unix_secs: i64) -> i32 {
    let (year, month, day, hour, _, _) = unix_to_datetime(unix_secs);
    let is_summer = is_summer_time(year as i32, month as u32, day as u32, hour as u32);
    if is_summer { 7200 } else { 3600 }
}

fn is_summer_time(year: i32, month: u32, day: u32, hour: u32) -> bool {
    let mar_last_sun = last_sunday_of_month(year, 3);
    let oct_last_sun = last_sunday_of_month(year, 10);
    if month > 3 && month < 10 { return true; }
    if month == 3 {
        if day > mar_last_sun { return true; }
        if day == mar_last_sun && hour >= 2 { return true; }
    }
    if month == 10 {
        if day < oct_last_sun { return true; }
        if day == oct_last_sun && hour < 3 { return true; }
    }
    false
}

fn last_sunday_of_month(year: i32, month: u32) -> u32 {
    // HARDCODED FOR 2025‑2027
    match (year, month) {
        (2025, 3) => 30, (2025, 10) => 26,
        (2026, 3) => 29, (2026, 10) => 25,
        (2027, 3) => 28, (2027, 10) => 31,
        _ => 31, // SAFE FALLBACK (LAST DAY OF MARCH/OCTOBER IS AT LEAST 25)
    }
}

fn unix_to_datetime(secs: i64) -> (u32, u32, u32, u32, u32, u32) {
    let days = (secs / 86400) as i32;
    let rem = secs % 86400;
    let hour = (rem / 3600) as u32;
    let minute = ((rem % 3600) / 60) as u32;
    let second = (rem % 60) as u32;
    let (year, month, day) = days_to_date(days);
    (year, month, day, hour, minute, second)
}

// CONVERT DAYS SINCE UNIX EPOCH (1970‑01‑01) TO YEAR, MONTH, DAY.
fn days_to_date(mut days: i32) -> (u32, u32, u32) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year { break; }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap_year(year);
    let month_days = [31, if leap { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0;
    while month < 12 && days >= month_days[month] {
        days -= month_days[month];
        month += 1;
    }
    (year as u32, (month + 1) as u32, (days + 1) as u32)
}

fn is_leap_year(year: i32) -> bool {
    year % 4 == 0 && (year % 100 != 0 || year % 400 == 0)
}
