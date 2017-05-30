use std::collections::VecDeque;
use std::io::{self, BufRead, Write};
use std::sync::mpsc::{channel, Sender, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use bufstream::BufStream;
use serial::{self, BaudRate, PortSettings, SerialPort};
use svg2polylines::Polyline;

const IBB_WIDTH: f64 = 358.0;
const IBB_HEIGHT: f64 = 123.0;
const TIMEOUT_MS_SERIAL: u64 = 1000;
const TIMEOUT_MS_CHANNEL: u64 = 100;

type Block = Vec<u8>;

pub struct Sketch<'a> {
    buf: Vec<u8>,
    block_size: usize,
    polylines: &'a [Polyline],
}

enum Command {
    BlockStart,
    BlockNumber(u16),
    StartDrawing,
    StopDrawing,
    PenLift,
    PenDown,
    Move(u16, u16),
    Wait(u8),
    Erase,
}

impl Command {
    pub fn to_bytes(&self) -> [u8; 3] {
        match *self {
            Command::BlockStart => [0xfa, 0x9f, 0xa1],
            Command::BlockNumber(num) => {
                if num >= 4000 { panic!("Block number must be <4000"); };
                [
                    0xfa,
                    ((0x09 << 4) | (num >> 8)) as u8,
                    (num & 0xff) as u8,
                ]
            },
            Command::StartDrawing => [0xfa, 0x1f, 0xa1],
            Command::StopDrawing => [0xfa, 0x20, 0x00],
            Command::PenLift => [0xfa, 0x30, 0x00],
            Command::PenDown => [0xfa, 0x40, 0x00],
            Command::Move(x, y) => [
                (x >> 4) as u8,
                (((x << 4) | (y >> 8)) & 0xff) as u8,
                (y & 0xff) as u8,
            ],
            Command::Wait(seconds) => {
                if seconds > 30 {
                    panic!("May not wait longer than 30 seconds");
                };
                [0xfa, 0x60, seconds]
            },
            Command::Erase => [0xfa, 0x50, 0x00],
        }
    }
}

fn invert(y: f64) -> f64 {
    IBB_HEIGHT - y
}

impl<'a> Sketch<'a> {
    pub fn new(polylines: &'a [Polyline]) -> Self {
        Sketch {
            buf: vec![],
            block_size: 768,
            polylines: polylines,
        }
    }

    /// Add a command to the internal command buffer.
    fn add_command(&mut self, command: Command) {
        self.buf.extend_from_slice(&command.to_bytes());
    }

    /// Convert the sketch into one or more byte vectors (blocks), ready to be
    /// sent to the robot via serial.
    pub fn into_blocks(mut self) -> Vec<Block> {
        // First, add all commands to a buffer
        self.add_command(Command::StartDrawing);
        self.add_command(Command::PenLift);
        self.add_command(Command::Move(0, 0));
        for polyline in self.polylines {
            if polyline.len() < 2 {
                warn!("Skipping polyline with less than 2 coordinate pairs");
                continue;
            }

            let start = polyline[0];
            self.add_command(Command::Move((start.x * 10.0) as u16, (invert(start.y) * 10.0) as u16));
            self.add_command(Command::PenDown);
            for point in polyline[1..].iter() {
                self.add_command(Command::Move((point.x * 10.0) as u16, (invert(point.y) * 10.0) as u16));
            }
            self.add_command(Command::PenLift);
        }
        self.add_command(Command::Move(0, 0));
        self.add_command(Command::StopDrawing);

        // Then, divide up the buffer into blocks
        let mut blocks = vec![];
        for (i, chunk) in self.buf.chunks(self.block_size - 6).enumerate() {
            let mut block = vec![];
            block.extend_from_slice(&Command::BlockStart.to_bytes());
            block.extend_from_slice(&Command::BlockNumber((i+1) as u16).to_bytes());
            block.extend_from_slice(chunk);
            blocks.push(block);
        }
        blocks
    }
}

/// Configure the serial port
fn setup_serial<P: SerialPort>(port: &mut P, baud_rate: BaudRate) -> io::Result<()> {
    port.configure(&PortSettings {
        baud_rate: baud_rate,
        char_size: serial::Bits8,
        parity: serial::ParityNone,
        stop_bits: serial::Stop1,
        flow_control: serial::FlowNone,
    })?;
    port.set_timeout(Duration::from_millis(TIMEOUT_MS_SERIAL))?;
    Ok(())
}

/// Spawn a thread that communicates with the robot over serial.
///
/// The return value is the sending end of a channel. Over this channel, a list
/// of polylines can be sent.
pub fn communicate(device: &str, baud_rate: BaudRate) -> Sender<Vec<Polyline>> {
    // Connect to serial device
    println!("Connecting to {} with baud rate {}...", device, baud_rate.speed());
    let mut port = serial::open(device)
        .expect(&format!("Could not open serial device {}", device));
    setup_serial(&mut port, baud_rate)
        .expect("Could not configure serial port");

    // Wrap port into a buffered stream
    let mut ser = BufStream::new(port);
    let mut buf = String::new();

    // Main loop
    let (tx, rx) = channel();
    thread::spawn(move || {
        let mut blocks_queue: VecDeque<Block> = VecDeque::new();
        loop {
            // Receive printing task
            let task: Result<Vec<Polyline>, RecvTimeoutError> =
                rx.recv_timeout(Duration::from_millis(TIMEOUT_MS_CHANNEL));
            match task {
                Err(RecvTimeoutError::Timeout) => {},
                Err(RecvTimeoutError::Disconnected) => {
                    println!("Disconnected from robot");
                    break;
                },
                Ok(polylines) => {
                    println!("Received task");
                    let sketch = Sketch::new(&polylines);
                    for block in sketch.into_blocks() {
                        println!("Block: {:?}", block);
                        for command in block.chunks(3) {
                            println!("  Command: {} {}",
                                     (command[0] as u16) << 4 | (command[1] as u16) >> 4,
                                     ((command[1] as u16) & 0b1111) << 8 | command[2] as u16);
                        }
                        blocks_queue.push_back(block);
                    }
                    println!("{} block(s) in queue", blocks_queue.len());
                },
            };

            // Talk to robot over serial
            if let Ok(_) = ser.read_line(&mut buf) {
                let line = buf.trim();
                println!("< {}", line);
                if blocks_queue.len() > 0 && (line == "CL STATUS=READY" || line == "CL STATUS=ACK&NUM=1") {
                    println!("> GOGO Draw");
                    let block = blocks_queue.pop_front().expect("Could not pop block from non-empty queue");
                    ser.write_all(&block)
                        .unwrap_or_else(|e| error!("Could not write data to serial: {}", e));
                    ser.flush()
                        .unwrap_or_else(|e| error!("Could not flush serial buffer: {}", e));
                }
            }
            buf.clear();
        }
    });
    tx
}


#[cfg(test)]
mod test {
    use svg2polylines::{Polyline, CoordinatePair};
    use super::*;

    #[test]
    fn test_empty_sketch() {
        let polylines: Vec<Polyline> = vec![];
        let sketch = Sketch::new(&polylines);
        let blocks = sketch.into_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], vec![
            0xfa, 0x9f, 0xa1, // Block start
            0xfa, 0x90, 0x01, // Block number 1
            0xfa, 0x1f, 0xa1, // Start drawing
            0xfa, 0x30, 0x00, // Pen lift
            0x00, 0x00, 0x00, // Move to 0,0
            0x00, 0x00, 0x00, // Move to 0,0
            0xfa, 0x20, 0x00, // Stop drawing
        ]);
    }

    #[test]
    fn test_simple_block() {
        let polylines: Vec<Polyline> = vec![
            vec![
                CoordinatePair::from((12.3, 45.6)),
                CoordinatePair::from((14.3, 47.6)),
            ]
        ];
        let sketch = Sketch::new(&polylines);
        let blocks = sketch.into_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0], vec![
            0xfa, 0x9f, 0xa1, // Block start
            0xfa, 0x90, 0x01, // Block number 1
            0xfa, 0x1f, 0xa1, // Start drawing
            0xfa, 0x30, 0x00, // Pen lift
            0x00, 0x00, 0x00, // Move to 0,0
            0x07, 0xb1, 0xc8, // Move to 123,456
            0xfa, 0x40, 0x00, // Pen down
            0x08, 0xf1, 0xdc, // Move to 143,476
            0xfa, 0x30, 0x00, // Pen lift
            0x00, 0x00, 0x00, // Move to 0,0
            0xfa, 0x20, 0x00, // Stop drawing
        ]);
    }

    #[test]
    fn test_full_block() {
        let mut polyline = vec![CoordinatePair::from((1.0, 1.0))];
        for _ in 0..123 {
            polyline.push(CoordinatePair::from((5.0, 10.0)));
            polyline.push(CoordinatePair::from((2.0, 4.0)));
        }
        let polylines = vec![polyline];
        let sketch = Sketch::new(&polylines);
        let blocks = sketch.into_blocks();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].len(), 768);
    }

    #[test]
    fn test_two_blocks() {
        let mut polyline = vec![CoordinatePair::from((1.0, 1.0))];
        for _ in 0..124 {
            polyline.push(CoordinatePair::from((5.0, 10.0)));
            polyline.push(CoordinatePair::from((2.0, 4.0)));
        }
        let polylines = vec![polyline];
        let sketch = Sketch::new(&polylines);
        let blocks = sketch.into_blocks();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].len(), 768);
        assert_eq!(blocks[1].len(), 12);
        assert_eq!(blocks[0][3..6], [0xfa, 0x90, 0x01]); // Block 1
        assert_eq!(blocks[1][3..6], [0xfa, 0x90, 0x02]); // Block 2
    }

}
