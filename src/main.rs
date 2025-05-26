#![no_std]
#![no_main]

use defmt::*;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_rp::{
    gpio::{Input, Output, Level, Pull},
    i2c::{self, I2c},
    init,
};
use embassy_time::{Timer, Delay};
use hd44780_driver::{HD44780, Display};
use heapless::{String, Vec};
use core::fmt::Write;
use core::write;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = init(Default::default());

    let mut delay = Delay;
    let i2c = I2c::new_blocking(p.I2C0, p.PIN_1, p.PIN_0, i2c::Config::default());
    let mut lcd = HD44780::new_i2c(i2c, 0x27, &mut delay).unwrap();

    lcd.reset(&mut delay).unwrap();
    lcd.clear(&mut delay).unwrap();
    lcd.set_display(Display::On, &mut delay).unwrap();
    lcd.write_str("Simon Says Ready", &mut delay).unwrap();
    Timer::after_millis(1500).await;

    let keys: [[char; 4]; 4] = [
        ['1', '2', '3', 'A'],
        ['4', '5', '6', 'B'],
        ['7', '8', '9', 'C'],
        ['*', '0', '#', 'D'],
    ];

    let mut rows = [
        Output::new(p.PIN_2, Level::High),
        Output::new(p.PIN_3, Level::High),
        Output::new(p.PIN_4, Level::High),
        Output::new(p.PIN_5, Level::High),
    ];

    let cols = [
        Input::new(p.PIN_6, Pull::Up),
        Input::new(p.PIN_7, Pull::Up),
        Input::new(p.PIN_8, Pull::Up),
        Input::new(p.PIN_9, Pull::Up),
    ];

    let mut leds = [
        Output::new(p.PIN_10, Level::Low),
        Output::new(p.PIN_11, Level::Low),
        Output::new(p.PIN_12, Level::Low),
        Output::new(p.PIN_13, Level::Low),
        Output::new(p.PIN_14, Level::Low),
        Output::new(p.PIN_15, Level::Low),
        Output::new(p.PIN_16, Level::Low),
        Output::new(p.PIN_17, Level::Low),
        Output::new(p.PIN_18, Level::Low),
    ];

    let mut buzzer = Output::new(p.PIN_19, Level::Low);
    let mut rng = 12345u32;
    let mut sequence: Vec<usize, 32> = Vec::new();
    let mut score = 0;
    let mut game_started = false;

    loop {
        if !game_started {
            lcd.clear(&mut delay).unwrap();
            lcd.write_str("Press * to start", &mut delay).unwrap();

            loop {
                if let Some(c) = read_key(&mut rows, &cols, &keys).await {
                    if c == '*' {
                        game_started = true;
                        break;
                    }
                }
                Timer::after_millis(20).await;
            }

            lcd.clear(&mut delay).unwrap();
            lcd.write_str("Watch!", &mut delay).unwrap();
            Timer::after_millis(500).await;
        }

        // Generează un nou LED
        rng = rng.wrapping_mul(1664525).wrapping_add(1013904223);
        let next = (rng as usize) % 9;
        sequence.push(next).unwrap();

        for &i in sequence.iter() {
            leds[i].set_high();
            Timer::after_millis(400).await;
            leds[i].set_low();
            Timer::after_millis(300).await;
        }

        lcd.clear(&mut delay).unwrap();
        let mut msg: String<32> = String::new();
        let _ = write!(msg, "Score: {}", score);
        lcd.write_str(&msg, &mut delay).unwrap();
        lcd.set_cursor_pos(0x40, &mut delay).unwrap();
        lcd.write_str("Your turn!", &mut delay).unwrap();

        let mut correct = true;

        for &i in sequence.iter() {
            let pressed = loop {
                if let Some(c) = read_key(&mut rows, &cols, &keys).await {
                    if let Some(idx) = char_to_index(c) {
                        break idx;
                    }
                }
                Timer::after_millis(20).await;
            };

            if pressed != i {
                correct = false;
                break;
            }

            leds[pressed].set_high();
            Timer::after_millis(200).await;
            leds[pressed].set_low();
            Timer::after_millis(100).await;
        }

        if !correct {
            lcd.clear(&mut delay).unwrap();
            let mut msg: String<32> = String::new();
            let _ = write!(msg, "NOO, SCORE: {}", score);
            lcd.write_str(&msg, &mut delay).unwrap();
            buzzer.set_high();
            Timer::after_millis(600).await;
            buzzer.set_low();
            sequence.clear();
            score = 0;
            game_started = false;
            Timer::after_secs(2).await;
        } else {
            score += 1;
            Timer::after_millis(1000).await;
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
    for (r, row) in rows.iter_mut().enumerate() {
        row.set_low();
        for (c, col) in cols.iter().enumerate() {
            if col.is_low() {
                Timer::after_millis(10).await;
                while col.is_low() {
                    Timer::after_millis(10).await;
                }
                row.set_high();
                return Some(keys[r][c]);
            }
        }
        row.set_high();
    }
    None
}
