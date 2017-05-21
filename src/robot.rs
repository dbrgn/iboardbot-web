use svg2polylines::Polyline;

pub struct Sketch<'a> {
    buf: Vec<u8>,
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
            polylines: polylines,
        }
    }

    /// Add a command to the internal command buffer.
    fn add_command(&mut self, command: Command) {
        self.buf.extend_from_slice(&command.to_bytes());
    }

    /// Convert the sketch into a byte vector, ready to be sent to the robot
    /// via serial.
    pub fn into_bytes(mut self) -> Vec<u8> {
        self.add_command(Command::BlockStart);
        self.add_command(Command::BlockNumber(1));
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
        self.buf
    }
}

pub fn print_polylines(polylines: &[Polyline]) {
    let sketch = Sketch::new(polylines);
    let bytes = sketch.into_bytes();
    println!("Bytes: {:?}", bytes);
}


#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_empty_sketch() {
        let polylines: Vec<Polyline> = vec![];
        let sketch = Sketch::new(&polylines);
        assert_eq!(sketch.buf.len(), 0);
        let bytes = sketch.into_bytes();
        assert_eq!(bytes, vec![
            0xfa, 0x9f, 0xa1, // Block start
            0xfa, 0x90, 0x01, // Block number 1
            0xfa, 0x1f, 0xa1, // Start drawing
            0xfa, 0x30, 0x00, // Pen lift
            0x00, 0x00, 0x00, // Move to 0,0
            0x00, 0x00, 0x00, // Move to 0,0
            0xfa, 0x20, 0x00, // Stop drawing
        ]);
    }
}
