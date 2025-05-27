#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::{
    gpio::{Input, Output, Level, Pull},
    i2c::{self, I2c, Blocking},
    peripherals::I2C0,
    init,
};
use embassy_time::{Timer, Delay};
use hd44780_driver::{
    HD44780,
    Display,
    bus::I2CBus,
};
use heapless::{String, Vec};
use core::fmt::Write;

async fn blink_led(led: &mut Output<'_>, _delay: &mut Delay) {
    for _ in 0..3 {
        led.set_high();
        Timer::after_millis(100).await;
        led.set_low();
        Timer::after_millis(100).await;
    }
}

async fn game_over_animation(led: &mut Output<'_>, buzzer: &mut Output<'_>, _delay: &mut Delay) {
    for _ in 0..3 {
        led.set_high();
        buzzer.set_high();
        Timer::after_millis(300).await;
        led.set_low();
        buzzer.set_low();
        Timer::after_millis(300).await;
    }
}

async fn countdown(
    lcd: &mut HD44780<I2CBus<I2c<'static, I2C0, Blocking>>>,
    delay: &mut Delay,
) {
    lcd.clear(delay).unwrap();
    lcd.write_str("Starting in 3", delay).unwrap();
    Timer::after_millis(1000).await;

    lcd.clear(delay).unwrap();
    lcd.write_str("Starting in 2", delay).unwrap();
    Timer::after_millis(1000).await;

    lcd.clear(delay).unwrap();
    lcd.write_str("Starting in 1", delay).unwrap();
    Timer::after_millis(1000).await;

    lcd.clear(delay).unwrap();
    lcd.write_str("Go!", delay).unwrap();
    Timer::after_millis(500).await;

    lcd.clear(delay).unwrap();
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = init(Default::default());

    let mut delay = Delay;
    let i2c_peripheral = I2c::new_blocking(p.I2C0, p.PIN_1, p.PIN_0, i2c::Config::default());
    let mut lcd = HD44780::new_i2c(i2c_peripheral, 0x27, &mut delay).unwrap();

    lcd.reset(&mut delay).unwrap();
    lcd.clear(&mut delay).unwrap();
    lcd.set_display(Display::On, &mut delay).unwrap();

    let keys: [[char; 4]; 4] = [
        ['1', '2', '3', 'A'],
        ['4', '5', '6', 'B'],
        ['7', '8', '9', 'C'],
        ['*', '0', '#', 'D'],
    ];

    let mut rows = [
        Output::new(p.PIN_2, Level::High), Output::new(p.PIN_3, Level::High),
        Output::new(p.PIN_4, Level::High), Output::new(p.PIN_5, Level::High),
    ];

    let cols = [
        Input::new(p.PIN_6, Pull::Up), Input::new(p.PIN_7, Pull::Up),
        Input::new(p.PIN_8, Pull::Up), Input::new(p.PIN_9, Pull::Up),
    ];

    let mut leds = [
        Output::new(p.PIN_10, Level::Low), Output::new(p.PIN_11, Level::Low),
        Output::new(p.PIN_12, Level::Low), Output::new(p.PIN_13, Level::Low),
        Output::new(p.PIN_14, Level::Low), Output::new(p.PIN_15, Level::Low),
        Output::new(p.PIN_16, Level::Low), Output::new(p.PIN_17, Level::Low),
        Output::new(p.PIN_18, Level::Low),
    ];

    let mut led_verde = Output::new(p.PIN_27, Level::Low);
    let mut led_rosu = Output::new(p.PIN_28, Level::Low);
    let mut buzzer = Output::new(p.PIN_19, Level::Low);

    let mut sequence: Vec<usize, 32> = Vec::new();
    let mut score_to_display_on_menu = 0;
    let mut first_run = true;

    loop {
        lcd.clear(&mut delay).unwrap();
        if first_run {
            lcd.write_str("Simon Says!", &mut delay).unwrap();
            lcd.set_cursor_pos(0x40, &mut delay).unwrap();
            lcd.write_str("Press * to Play", &mut delay).unwrap();
            first_run = false;
        } else {
            let mut msg_line1: String<32> = String::new();
            let _ = write!(msg_line1, "NOO, SCORE: {}", score_to_display_on_menu);
            lcd.write_str(&msg_line1, &mut delay).unwrap();

            lcd.set_cursor_pos(0x40, &mut delay).unwrap();
            lcd.write_str("Press * to Play", &mut delay).unwrap();
        }

        let rng_seed = get_entropy_from_keys(&mut rows, &cols, &keys).await;
        let mut rng = rng_seed;

        countdown(&mut lcd, &mut delay).await;

        sequence.clear();
        let mut current_game_score = 0;

        loop {
            lcd.clear(&mut delay).unwrap();
            lcd.write_str("Watch!", &mut delay).unwrap();

            rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
            let next_led_index = (rng as usize) % 9;
            sequence.push(next_led_index).unwrap();

            for &led_idx_to_light in sequence.iter() {
                leds[led_idx_to_light].set_high();
                Timer::after_millis(400).await;
                leds[led_idx_to_light].set_low();
                Timer::after_millis(300).await;
            }

            lcd.clear(&mut delay).unwrap();
            let mut score_msg: String<32> = String::new();
            let _ = write!(score_msg, "Score: {}", current_game_score);
            lcd.write_str(&score_msg, &mut delay).unwrap();
            lcd.set_cursor_pos(0x40, &mut delay).unwrap();
            lcd.write_str("Your turn!", &mut delay).unwrap();

            let mut round_correct = true;

            for &expected_led_index in sequence.iter() {
                let pressed_led_index = loop {
                    if let Some(key_char) = read_key(&mut rows, &cols, &keys).await {
                        if let Some(idx) = char_to_index(key_char) {
                            break idx;
                        }
                    }
                    Timer::after_millis(20).await;
                };

                if pressed_led_index != expected_led_index {
                    round_correct = false;
                    break;
                }

                leds[pressed_led_index].set_high();
                Timer::after_millis(200).await;
                leds[pressed_led_index].set_low();
                Timer::after_millis(100).await;
            }

            if !round_correct {
                score_to_display_on_menu = current_game_score;
                game_over_animation(&mut led_rosu, &mut buzzer, &mut delay).await;
                break;
            } else {
                current_game_score += 1;
                blink_led(&mut led_verde, &mut delay).await;
                Timer::after_millis(1000).await;
            }
        }
    }
}

fn char_to_index(c: char) -> Option<usize> {
    match c {
        '1' => Some(0), '2' => Some(1), '3' => Some(2),
        '4' => Some(3), '5' => Some(4), '6' => Some(5),
        '7' => Some(6), '8' => Some(7), '9' => Some(8),
        _ => None,
    }
}

async fn read_key<'d>(
    rows: &mut [Output<'d>; 4],
    cols: &[Input<'d>; 4],
    keys: &[[char; 4]; 4],
) -> Option<char> {
    for (r, row_pin) in rows.iter_mut().enumerate() {
        row_pin.set_low();
        for (c, col_pin) in cols.iter().enumerate() {
            if col_pin.is_low() {
                Timer::after_millis(10).await;
                if col_pin.is_low() {
                    while col_pin.is_low() {
                        Timer::after_millis(10).await;
                    }
                    row_pin.set_high();
                    return Some(keys[r][c]);
                }
            }
        }
        row_pin.set_high();
    }
    None
}

async fn get_entropy_from_keys<'d>(
    rows: &mut [Output<'d>; 4],
    cols: &[Input<'d>; 4],
    keys: &[[char; 4]; 4],
) -> u32 {
    let mut counter = 0u32;

    loop {
        Timer::after_millis(10).await;
        counter = counter.wrapping_add(1);

        if let Some(c) = read_key(rows, cols, keys).await {
            if c == '*' {
                break;
            }
        }
    }

    counter
}
