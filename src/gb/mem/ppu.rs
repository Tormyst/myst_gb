
macro_rules! get_bit {
    ($data:expr, $bit:expr) => {
        (($data >> $bit) & 0x01) > 0
    }
}

const LINE_CYCLE: usize = 456;
const LYMAX: u8 = 153;


pub struct PPU {
    lcdc: u8, // 40
    stat: u8, // 41

    pub scy: u8, // 42
    pub scx: u8, // 43

    ly: u8, // 44
    lx: usize, // Hidden value used to tell where we are in the write cycle
    lx_sent: bool,
    vblank_interupt_buffered: bool,
    stat_interupt_buffered: bool,
    lyc: u8, // 45

    wy: u8, //4A
    wx: u8, //4B
}

enum State {
    HBlank,
    VBlank,
    Oam,
    Vram,
}

impl PPU {
    pub fn new() -> Self {
        PPU {
            lcdc: 0,
            stat: 0,
            scy: 0,
            scx: 0,
            ly: 0,
            lx: 0,
            lx_sent: false,
            vblank_interupt_buffered: false,
            stat_interupt_buffered: false,
            lyc: 0,
            wy: 0,
            wx: 0,
        }
    }
    pub fn read(&self, addr: u16) -> Option<u8> {
        //println!("Reading from PPU at {:04X}", addr);
        match addr {
            0xFF40 => Some(self.lcdc),
            0xFF41 => Some(self.stat),
            0xFF42 => Some(self.scy),
            0xFF43 => Some(self.scx),
            0xFF44 => Some(self.ly),
            0xFF45 => Some(self.lyc),
            0xFF4A => Some(self.wy),
            0xFF4B => Some(self.wx),
            _ => None,
        }
    }
    pub fn write(&mut self, addr: u16, data: u8) -> bool {
        //println!("Writing to PPU at {:4X} the value {:2X}", addr, data);
        match addr {
            0xFF40 => {self.lcdc_set(data)},
            0xFF41 => {self.stat = data & 0xF8 | self.stat & 0x7; true},
            0xFF42 => {self.scy = data; true},
            0xFF43 => {self.scx = data; true},
            0xFF44 => {self.ly = 0; true}, // LY is read only.  Writing resets the value
            0xFF45 => {self.lyc; true},
            0xFF4A => {self.wy; true},
            0xFF4B => {self.wx; true},
            _ => false,
        }
    }

    fn lcdc_set(&mut self, data: u8) -> bool{
        // If bit 7 is set, then we have to set up for display.  This cannot be done during a
        // frame, only during vblank.
        if get_bit!(self.lcdc, 7) == false && get_bit!(data, 7) == true {
            self.ly = 0;
            self.lx = 0;
            self.set_state();
        }
        else if get_bit!(self.lcdc, 7) == true && get_bit!(data, 7) == false {
            match self.state() {
                State::VBlank => panic!("Turning off screen during vblank.  Gameboy crashes."),
                _ => {}
            }
        }
        self.lcdc = data;
        self.print_lcdc();
        true
    }

    pub fn lcdc_get(&self, offset: u8) -> bool {
        self.lcdc & (0x01 << offset) != 0
    }

    fn print_lcdc(&self) {
        println!("LCDC: ");

        if self.lcdc_get(7) {
            if self.lcdc_get(0) {
                println!("Background on using map {}", match
                       self.lcdc_get(3) {true=> "9C00-9FFF", false=> "9800-9BFF"}
                       );
                if self.lcdc_get(5) {
                    println!("Window on using map {}", match
                        self.lcdc_get(6) {true=> "9C00-9FFF", false=> "9800-9BFF"}
                        );
                }
                else {
                    println!("Window off");
                }
                println!("Tilemap: {}", match
                        self.lcdc_get(4) {true=> "8000-8FFF", false=> "8800-97FF"}
                        );
                if self.lcdc_get(1) {
                    println!("{} OBJ on", match
                             self.lcdc_get(2) {true=> "8x16", false=> "8x8"});
                }
                else {
                    println!("OBJ off");
                }
            }
            else { println!("Background and Window off"); }
        }
        else
        {
            println!("Screen off");
        }
    }

    fn stat_get(&self, offset: u8) -> bool {
        self.stat & (0x01 << offset) != 0
    }

    pub fn time_passes(&mut self, time: usize) -> Option<Vec<u8>>{
        if !self.lcdc_get(7) { None }
        else {
            self.lx += time;
            if self.lx >= 248 {
                let mut ret = vec![];
                if !self.lx_sent { ret.push(self.ly); self.lx_sent = true;}
                while self.lx > LINE_CYCLE {
                    self.ly += 1;
                    self.lx -= 456;
                    if self.ly > LYMAX { self.ly = 0; }
                    self.buffer_interupts();
                    if self.lx >= 248 { ret.push(self.ly); self.lx_sent = true;}
                    else { self.lx_sent = false; }
                }
                self.set_state();
                Some(ret)
            }
            else {
                self.set_state();
                None
            }
        }
    }

    fn buffer_interupts(&mut self) {
        if self.ly == 144 { self.vblank_interupt_buffered = true }
        if self.stat_get(6) {
            if match self.stat_get(2) {
                true => self.lyc == self.ly,
                false => self.lyc != self.ly,
            }{
                self.stat_interupt_buffered = true;
            }
        }
    }

    pub fn interupt_update(&mut self) -> u8 {
        let mut val = 0;
        if self.vblank_interupt_buffered {
            self.vblank_interupt_buffered = false;
            val |= 0x01;
        }
        if self.stat_interupt_buffered {
            self.stat_interupt_buffered = false;
            val |= 0x02;
        }
        val
    }

    fn set_state(&mut self) {
        if self.ly >= 144 {
            self.stat = (self.stat & 0xF8) | 0x01; // Vblank
        }
        else {
            self.stat = match self.lx {
                0...77 => self.stat & 0xF8 | 0x02,
                78...247 => self.stat & 0xF8 | 0x03,
                248...456 => self.stat & 0xF8 | 0x00,
                _ => unreachable!("There should be no value of lx great then 456"),
            }
        }
    }

    fn state(&self) -> State {
        match 0x02 & self.stat {
            0 => State::HBlank,
            1 => State::VBlank,
            2 => State::Oam,
            3 => State::Vram,
            _ => unreachable!("There are only 4 2bit states."),
        }
    }
}
