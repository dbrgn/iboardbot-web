use std::collections::VecDeque;
use std::io::{self, BufRead, Write};
use std::sync::mpsc::{channel, Sender, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use bufstream::BufStream;
use regex::Regex;
use serial::{self, BaudRate, PortSettings, SerialPort};
use svg2polylines::Polyline;

const IBB_WIDTH: u16 = 358;
const IBB_HEIGHT: u16 = 123;
const TIMEOUT_MS_SERIAL: u64 = 1000;
const TIMEOUT_MS_CHANNEL: u64 = 50;

type Block = Vec<u8>;

pub struct Sketch<'a> {
    buf: Vec<u8>,
    block_size: usize,
    polylines: &'a [Polyline],
}

enum Command {
    /// Start of block
    BlockStart,
    /// This is block number n
    BlockNumber(u16),
    /// Start drawing
    StartDrawing,
    /// Stop drawing
    StopDrawing,
    /// Lift up the pen with the servo
    PenLift,
    /// Lower down the pen with the servo. This will disable the eraser.
    PenDown,
    /// Enable the eraser. This will also lift up the pen.
    EnableEraser,
    /// Move to the specified coordinates
    Move(u16, u16),
    /// Wait n seconds
    Wait(u8),
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
            Command::EnableEraser => [0xfa, 0x50, 0x00],
        }
    }
}

/// Clamp x coordinate.
fn fix_x(x: f64) -> f64 {
    if x < 0.0 {
        0.0
    } else if x > (IBB_WIDTH as f64) {
        IBB_WIDTH as f64
    } else {
        x
    }
}

/// Invert and clamp y coordinate.
fn fix_y(y: f64) -> f64 {
    // Invert y coordinate, since SVG uses the top left as 0 coordinate,
    // while the IBB uses the bottom left as 0 coordinate.
    let yy = (IBB_HEIGHT as f64) - y;

    if yy < 0.0 {
        0.0
    } else if yy > (IBB_HEIGHT as f64) {
        IBB_HEIGHT as f64
    } else {
        yy
    }
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

    /// Erase the entire board.
    /// Note that this does not contain the `StartDrawing` and `Stop Drawing`
    /// commands!
    fn erase_all(&mut self) {
        self.add_command(Command::PenLift);
        self.add_command(Command::Move(0, IBB_HEIGHT * 10));
        self.add_command(Command::EnableEraser);
        let mut y = IBB_HEIGHT;
        let y_step = 10;
        loop {
            // Loop until we're at the bottom
            if y <= 0 {
                break;
            }

            // Move to right and step down
            self.add_command(Command::Move(IBB_WIDTH * 10, y * 10));
            if y > y_step {
                y -= y_step;
            } else {
                y = 0;
            }

            // Move back to left and step down
            self.add_command(Command::Move(IBB_WIDTH * 10, y * 10));
            self.add_command(Command::Move(0, y * 10));
            if y > y_step {
                y -= y_step;
            } else {
                y = 0;
            }
            self.add_command(Command::Move(0, y * 10));
        }
        self.add_command(Command::PenLift);
        self.add_command(Command::Move(0, 0));
    }

    /// Convert the sketch into one or more byte vectors (blocks), ready to be
    /// sent to the robot via serial.
    pub fn into_blocks(mut self, erase: bool) -> Vec<Block> {
        // Start a new drawing
        self.add_command(Command::StartDrawing);

        // First, erase the entire board.
        if erase {
            self.erase_all();
        } else {
            // If we used the eraser, we're already at `(0, 0)` coordinates.
            self.add_command(Command::PenLift);
            self.add_command(Command::Move(0, 0));
        }

        // Now add the drawing commands to the buffer
        for polyline in self.polylines {
            if polyline.len() < 2 {
                warn!("Skipping polyline with less than 2 coordinate pairs");
                continue;
            }

            let start = polyline[0];
            self.add_command(Command::Move((fix_x(start.x) * 10.0) as u16, (fix_y(start.y) * 10.0) as u16));
            self.add_command(Command::PenDown);
            for point in polyline[1..].iter() {
                self.add_command(Command::Move((fix_x(point.x) * 10.0) as u16, (fix_y(point.y) * 10.0) as u16));
            }
            self.add_command(Command::PenLift);
        }

        // Move back to start, done
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

    // Regex for recognizing ACK messages
    let ack_re = Regex::new(r"^CL STATUS=ACK&NUM=(\d+)$").expect("Could not compile regex");

    // Main loop
    let (tx, rx) = channel();
    thread::spawn(move || {
        // A queue for blocks that should be printed.
        let mut blocks_queue: VecDeque<Block> = VecDeque::new();

        // The current block number.
        let mut current_block: u32 = 0;

        loop {
            // Check for a new printing task
            let task: Result<Vec<Polyline>, RecvTimeoutError> =
                rx.recv_timeout(Duration::from_millis(TIMEOUT_MS_CHANNEL));
            match task {
                Ok(polylines) => {
                    println!("Received task");
                    let sketch = Sketch::new(&polylines);
                    for block in sketch.into_blocks(true) {
                        blocks_queue.push_back(block);
                    }
                    println!("{} block(s) in queue", blocks_queue.len());
                },
                Err(RecvTimeoutError::Timeout) => {
                    // We didn't get a new task.
                    // Simply ignore it :)
                },
                Err(RecvTimeoutError::Disconnected) => {
                    println!("Disconnected from robot");
                    break;
                },
            };

            // Talk to robot over serial
            if let Ok(_) = ser.read_line(&mut buf) {
                let line = buf.trim();

                // Debug print of all serial input
                println!("< {}", line);

                // If there are blocks to be sent and we got a new CL command
                // from the robot...
                if blocks_queue.len() > 0 && line.starts_with("CL ") {
                    let mut send_next = false;

                    if line == "CL STATUS=READY" {
                        send_next = true;
                    } else if let Some(captures) = ack_re.captures(line) {
                        let number_str = captures.get(1).unwrap().as_str();
                        match number_str.parse::<u32>() {
                            Ok(number) if number == 1 => {
                                // If the acked number went down, that means that a reset
                                // happened in between. Simply continue printing.
                                send_next = true;
                            },
                            Ok(number) if number == current_block => {
                                // Acked number is our current block, so we can safely
                                // send the next one.
                                send_next = true;
                            },
                            Ok(number) if number > current_block => {
                                // Acked number is larger than our current block. That means
                                // that we probably started the server process after a few
                                // blocks were already printed. Reset the number.
                                println!("Reset current block number");
                                current_block = 1;
                                send_next = true;
                            },
                            Ok(number) => {
                                println!("Warning: Got ack for non-current block ({} != {})", number, current_block);
                            },
                            Err(_) => {
                                println!("Could not parse ACK number \"{}\"", number_str);
                            },
                        }
                    }

                    if send_next {
                        println!("> Print a block");
                        let block = blocks_queue.pop_front().expect("Could not pop block from non-empty queue");
                        current_block += 1;
                        ser.write_all(&block)
                            .unwrap_or_else(|e| error!("Could not write data to serial: {}", e));
                        ser.flush()
                            .unwrap_or_else(|e| error!("Could not flush serial buffer: {}", e));
                    }
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
        let blocks = sketch.into_blocks(false);
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
        let blocks = sketch.into_blocks(false);
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
        let blocks = sketch.into_blocks(false);
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
        let blocks = sketch.into_blocks(false);
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].len(), 768);
        assert_eq!(blocks[1].len(), 12);
        assert_eq!(blocks[0][3..6], [0xfa, 0x90, 0x01]); // Block 1
        assert_eq!(blocks[1][3..6], [0xfa, 0x90, 0x02]); // Block 2
    }

}
