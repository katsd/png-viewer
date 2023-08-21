use std::env;
use std::fs::File;
use std::io::{BufReader, Read};
use std::ops::Range;
use colored::Colorize;
use flate2::read::{ZlibDecoder};
use pixels::{Pixels, SurfaceTexture};
use winit::dpi::LogicalSize;
use winit::event::Event;
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

fn main() {
    let args: Vec<String> = env::args().collect();

    let image_path = args.get(1).expect("No image file specified");

    let buf = BufReader::new(File::open(image_path).expect("Failed to open file"));

    let bytes = buf.bytes().flat_map(|b| b).collect::<Vec<u8>>();

    let mut reader = PngReader::new(bytes);

    reader.read();

    init_window(reader.width, reader.height, reader.pixel_data.clone());
}

fn init_window(width: u32, height: u32, pixel_data: Vec<Vec<Pixel>>) {
    let event_loop = EventLoop::new();

    let window = {
        let size = LogicalSize::new(width, height);
        WindowBuilder::new()
            .with_title("png-viewer")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(width, height, surface_texture).unwrap()
    };

    event_loop.run(move |event, _, _| {
        if let Event::RedrawRequested(_) = event {
            println!("RedrawRequested");

            for h in 0..height as usize {
                for w in 0..width as usize {
                    let idx = h * width as usize * 4 + w * 4;
                    pixels.frame_mut()[idx] = pixel_data[h][w].r;
                    pixels.frame_mut()[idx + 1] = pixel_data[h][w].g;
                    pixels.frame_mut()[idx + 2] = pixel_data[h][w].b;
                    pixels.frame_mut()[idx + 3] = pixel_data[h][w].a;
                }
            }

            pixels.render();
        }
    })
}

#[derive(Default, Copy, Clone)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

struct PngReader {
    bytes: Vec<u8>,

    pub width: u32,
    pub height: u32,
    bit_depth: u8,
    colour_type: u8,
    compression_method: u8,
    filter_method: u8,
    interlace_method: u8,

    image_data: Vec<u8>,
    pub pixel_data: Vec<Vec<Pixel>>,
}

impl PngReader {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self {
            bytes,
            width: 0,
            height: 0,
            bit_depth: 0,
            colour_type: 0,
            compression_method: 0,
            filter_method: 0,
            interlace_method: 0,
            image_data: vec![],
            pixel_data: vec![],
        }
    }

    pub fn read(&mut self) {
        let mut idx = 0;

        idx = self.read_signature(idx).expect("Invalid data");

        while idx < self.bytes.len() {
            idx = self.read_chunk(idx).expect("Invalid data");
        }

        self.decode_image_data();
    }

    fn read_signature(&self, idx: usize) -> Result<usize, ()> {
        let sig = &[137, 80, 78, 71, 13, 10, 26, 10];

        if self.bytes[0..sig.len()] != *sig {
            return Err(());
        }

        Self::print("Signature", &self.bytes[0..sig.len()]);

        Ok(sig.len())
    }

    fn read_chunk(&mut self, idx: usize) -> Result<usize, ()> {
        let start_idx = idx;
        let mut idx = idx;

        // length
        let data_len = usize::from_be_bytes([
            0, 0, 0, 0,
            self.bytes[idx],
            self.bytes[idx + 1],
            self.bytes[idx + 2],
            self.bytes[idx + 3],
        ]);
        idx += 4;

        // chunk type
        let chunk_type = std::str::from_utf8(&self.bytes[idx..idx + 4]).unwrap();
        idx += 4;

        // chunk data
        let data_range = idx..idx + data_len;
        let data = &self.bytes[data_range.clone()];
        idx += data_len;

        // crc
        idx += 4;

        Self::print(chunk_type, data);

        match chunk_type {
            "IHDR" => self.read_chunk_ihdr(&data_range),
            "IDAT" => self.read_chunk_idat(&data_range),
            "tEXt" => Self::read_chunk_text(data),
            "tIME" => Self::read_chunk_time(data),
            _ => ()
        };

        println!();

        Ok(idx)
    }

    fn read_chunk_ihdr(&mut self, data_range: &Range<usize>) {
        let data = &self.bytes[data_range.clone()];
        self.width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
        self.height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        self.bit_depth = data[8];
        self.colour_type = data[9];
        self.compression_method = data[10];
        self.filter_method = data[11];
        self.interlace_method = data[12];

        Self::print_content(
            "Image header",
            format!(r#"[Size] {}x{}
[Bit depth] {}
[Colour type] {}
[Compression method] {}
[Filter method] {}
[Interlace method] {}"#,
                    self.width, self.height, self.bit_depth, self.colour_type, self.compression_method, self.filter_method, self.interlace_method),
        );
    }

    fn read_chunk_idat(&mut self, data_range: &Range<usize>) {
        let data = &self.bytes[data_range.clone()];
        self.image_data.append(&mut data.to_vec());

        /*
        let mut decompressed_data = Vec::<u8>::new();
        let data_len = ZlibDecoder::new(data).read_to_end(&mut decompressed_data).unwrap();
        */

        Self::print_content("Image data", format!("{} bytes", data.len()));
    }

    fn read_chunk_text(data: &[u8]) {
        let mut separator_idx: usize = 0;

        for i in 0..data.len() {
            if data[i] == 0 {
                separator_idx = i;
                break;
            }
        }

        let keyword = std::str::from_utf8(&data[0..separator_idx]).unwrap();
        let text = std::str::from_utf8(&data[separator_idx + 1..data.len()]).unwrap();

        Self::print_content("Textual data", format!("[keyword] {}\n[text] {}", keyword, text));
    }

    fn read_chunk_time(data: &[u8]) {
        let year = u16::from_be_bytes([data[0], data[1]]);
        let month = data[2];
        let day = data[3];
        let hour = data[4];
        let minutes = data[5];
        let second = data[6];

        Self::print_content("Image last-modification time", format!("{}/{}/{} {:<02}:{:<02}:{:<02}", year, month, day, hour, minutes, second));
    }

    fn print(title: &str, data: &[u8]) {
        println!("{}\n{:<02x?}\n", title.on_blue().white(), data.iter().take(30).collect::<Vec<_>>());
    }

    fn print_content(title: &str, content: String) {
        println!("{}\n{}\n", title.green(), content);
    }

    fn decode_image_data(&mut self) {
        let mut data = Vec::<u8>::new();
        let data_len = ZlibDecoder::new(self.image_data.as_slice()).read_to_end(&mut data).unwrap();

        let color_len = match self.colour_type {
            0 => 1,
            2 => 3,
            3 => 1,
            4 => 2,
            6 => 4,
            _ => panic!("Invalid colour type")
        };

        for h in 0..self.height as usize {
            self.pixel_data.push(vec![Default::default(); self.width as usize]);

            let mut idx = (self.width as usize * color_len + 1) * h;
            let filter_type = data[idx];
            idx += 1;

            for w in 0..self.width as usize {
                let a = if w == 0 { Default::default() } else { self.pixel_data[h][w - 1] };
                let b = if h == 0 { Default::default() } else { self.pixel_data[h - 1][w] };
                let c = if w == 0 || h == 0 { Default::default() } else { self.pixel_data[h - 1][w - 1] };

                match self.colour_type {
                    0 | 4 => {
                        let pixel_r = Self::remove_filter(filter_type, data[idx], a.r, b.r, c.r);
                        let pixel_a = if self.colour_type == 0 { 0xFF } else {
                            Self::remove_filter(filter_type, data[idx + 1], a.a, b.a, c.a)
                        };

                        self.pixel_data[h][w] = Pixel {
                            r: pixel_r,
                            g: pixel_r,
                            b: pixel_r,
                            a: pixel_a,
                        };
                    }
                    2 | 6 => {
                        let pixel_a = if self.colour_type == 2 { 0xFF } else {
                            Self::remove_filter(filter_type, data[idx + 3], a.a, b.a, c.a)
                        };

                        self.pixel_data[h][w] = Pixel {
                            r: Self::remove_filter(filter_type, data[idx], a.r, b.r, c.r),
                            g: Self::remove_filter(filter_type, data[idx + 1], a.g, b.g, c.g),
                            b: Self::remove_filter(filter_type, data[idx + 2], a.b, b.b, c.b),
                            a: pixel_a,
                        };
                    }
                    _ => {}
                }


                idx += color_len;
            }
        }
    }

    fn remove_filter(filter_type: u8, x: u8, a: u8, b: u8, c: u8) -> u8 {
        match filter_type {
            0 => {
                x
            }

            1 => {
                (x as i32 + a as i32) as u8
            }
            2 => {
                (x as i32 + b as i32) as u8
            }

            3 => {
                (x as i32 + ((a as i32 + b as i32) / 2)) as u8
            }

            4 => {
                (x as i32 + Self::paeth(a, b, c) as i32) as u8
            }

            _ => 0
        }
    }

    fn paeth(a: u8, b: u8, c: u8) -> u8 {
        let a = a as i32;
        let b = b as i32;
        let c = c as i32;
        let p = a + b - c;

        let pa = (p - a).abs();
        let pb = (p - b).abs();
        let pc = (p - c).abs();

        return if pa <= pb && pa <= pc {
            a as u8
        } else if pb <= pc {
            b as u8
        } else {
            c as u8
        };
    }
}