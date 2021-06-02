use std::{fs::{self, File}, io::{self, BufReader, Read}, mem::size_of, path::Path};
use anyhow::*;
use byteorder::{ReadBytesExt, LE};
use image::{Rgba, RgbaImage};
use walkdir::WalkDir;

fn main() -> Result<()> {
    let home = std::env::home_dir().unwrap();
    let celeste_path = home.join(".steam/steam/steamapps/common/Celeste/");
    let output_dir = Path::new("/tmp/out");

    for entry in WalkDir::new(&celeste_path) {
        let entry = entry?;
        
        if !entry.file_type().is_file() {
            continue
        }

        let path = entry.path();

        if path.extension() != Some("data".as_ref()) {
            continue
        }

        println!("Processing {}", path.display());

        let image = match load_image_from_path(&path) {
            Ok(image) => image,
            Err(err) => {
                println!("{}", err);
                continue
            }
        };

        let path = path.strip_prefix(&celeste_path)?;
        let mut path = output_dir.join(path);
        
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        path.set_extension("png");

        image.save(path)?;
    }

    Ok(())
}

fn load_image_from_path(path: impl AsRef<Path>) -> Result<RgbaImage> {
    let file = File::open(path)?;
    let mut file = BufReader::new(file);
    let image = load_image(&mut file)?;

    Ok(image)
}

fn load_image<R: Read>(reader: &mut R) -> Result<RgbaImage> {
    let width = reader.read_u32::<LE>()?;
    let height = reader.read_u32::<LE>()?;
    let is_transparent = reader.read_u8()? != 0;
    let image_size = (width * height) as usize * size_of::<Rgba<u8>>();
    let mut image = Vec::with_capacity(image_size);

    loop {
        let run_length = match reader.read_u8() {
            Ok(run_length) => run_length,
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        };

        if run_length == u8::MAX {
            break;
        }

        let (a, b, g, r);

        if is_transparent {
            a = reader.read_u8()?;
        } else {
            a = u8::MAX;
        }

        if a == 0 {
            b = 0;
            g = 0;
            r = 0;
        } else {
            b = reader.read_u8()?;
            g = reader.read_u8()?;
            r = reader.read_u8()?;
        }

        for _ in 0..run_length {
            image.push(r);
            image.push(g);
            image.push(b);
            image.push(a);
        }
    }

    while image.len() < image.capacity() {
        image.push(0);
    }

    let image = RgbaImage::from_vec(width, height, image)
        .context("Image does not contain enough pixels")?;

    Ok(image)
}
