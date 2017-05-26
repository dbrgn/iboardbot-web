use svg2polylines::Polyline;

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
    pub fn into_blocks(mut self) -> Vec<Vec<u8>> {
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
            self.add_command(Command::Move((start.x * 10.0) as u16, (start.y * 10.0) as u16));
            self.add_command(Command::PenDown);
            for point in polyline[1..].iter() {
                self.add_command(Command::Move((point.x * 10.0) as u16, (point.y * 10.0) as u16));
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

pub fn print_polylines(polylines: &[Polyline]) {
    let sketch = Sketch::new(polylines);
    let blocks = sketch.into_blocks();
    println!("Blocks: {:?}", blocks);
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
