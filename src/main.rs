use std::{env, fs::{self, File}, io::{self, BufReader, Read}, mem::size_of, path::Path};
use anyhow::*;
use byteorder::{ReadBytesExt, LE};
use image::{Rgba, RgbaImage, GenericImageView};
use walkdir::WalkDir;

fn main() -> Result<()> {
    let celeste_path = env::args().nth(1).context("First parameter must be celeste dir")?;
    let celeste_path = Path::new(&celeste_path);
    let output_dir = env::args().nth(2).context("Second parameter must be output dir")?;
    let output_dir = Path::new(&output_dir);

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

    let gameplay_meta = load_meta_from_file(celeste_path.join("Content/Graphics/Atlases/Gameplay.meta"))?;
    split_atlas(output_dir.join("Content/Graphics/Atlases/Gameplay0.png"), &gameplay_meta[0])?;

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
    let has_alpha = reader.read_u8()? != 0;
    let image_size = (width * height) as usize * size_of::<Rgba<u8>>();
    let mut image = Vec::with_capacity(image_size);

    loop {
        let run_length = match reader.read_u8() {
            Ok(run_length) => run_length,
            Err(err) if err.kind() == io::ErrorKind::UnexpectedEof => break,
            Err(err) => return Err(err.into()),
        };

        let (a, b, g, r);

        if has_alpha {
            a = reader.read_u8()?;

            if a == 0 {
                b = 0;
                g = 0;
                r = 0;
            } else {
                b = reader.read_u8()?;
                g = reader.read_u8()?;
                r = reader.read_u8()?;
            }
        } else {
            a = u8::MAX;
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

    let image = RgbaImage::from_vec(width, height, image)
        .context("Image does not contain enough pixels")?;

    Ok(image)
}

fn load_meta_from_file(path: impl AsRef<Path>) -> Result<Vec<DataFile>> {
    let file = File::open(path)?;
    let mut file = BufReader::new(file);
    let image = load_meta(&mut file)?;

    Ok(image)
}

fn load_meta<R: Read>(reader: &mut R) -> Result<Vec<DataFile>> {
    let unknown1 = reader.read_u32::<LE>()?; // format version?
    println!("unknown 1: {}", unknown1);

    let unknown2 = read_string(reader)?;
    println!("unknown 2: {}", unknown2);

    let unknown3 = reader.read_u32::<LE>()?;
    println!("unknown3: {:032b}", unknown3);

    let num_datafiles = reader.read_u16::<LE>()?;
    println!("num_datafiles: {}", num_datafiles);

    let mut data_files = Vec::new();

    for _ in 0..num_datafiles {
        let data_file_path = read_string(reader)?;
        let num_sprites = reader.read_u16::<LE>()?;

        println!("data_file_path: {}", data_file_path);
        println!("num_sprites: {}", num_sprites);

        let mut sprites = Vec::new();

        for _ in 0..num_sprites {
            let sprite = Sprite::from_reader(reader)?;

            println!("{:#?}", sprite);

            sprites.push(sprite);
        }

        let data_file = DataFile {
            path: data_file_path,
            sprites,
        };

        data_files.push(data_file);
    }

    Ok(data_files)
}

fn read_string<R: Read>(reader: &mut R) -> Result<String> {
    let len = read_variable_usize(reader)?;
    let mut value = String::with_capacity(len);

    reader.take(len as u64).read_to_string(&mut value)?;

    Ok(value)
}

fn read_variable_usize<R: Read>(reader: &mut R) -> Result<usize> {
    let mut res = 0;
    let mut count = 0;

    loop {
        let byte = reader.read_u8()?;
        res += (byte as usize & 127) << (count * 7);
        count += 1;

        if (byte >> 7) == 0 {
            return Ok(res);
        }
    }
}

struct DataFile {
    path: String,
    sprites: Vec<Sprite>,
}

#[derive(Debug)]
struct Sprite {
    path: String,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
    offset_x: u16,
    offset_y: u16,
    real_width: u16,
    real_height: u16,
}

impl Sprite {
    fn from_reader<R: Read>(reader: &mut R) -> Result<Self> {
        Ok(Self {
            path: read_string(reader)?.replace("\\", "/"),
            x: reader.read_u16::<LE>()?,
            y: reader.read_u16::<LE>()?,
            width: reader.read_u16::<LE>()?,
            height: reader.read_u16::<LE>()?,
            offset_x: reader.read_u16::<LE>()?,
            offset_y: reader.read_u16::<LE>()?,
            real_width: reader.read_u16::<LE>()?,
            real_height: reader.read_u16::<LE>()?,
        })
    }
}

fn split_atlas(atlas_png: impl AsRef<Path>, data_file: &DataFile) -> Result<()> {
    let atlas_png = atlas_png.as_ref();
    let atlas = image::open(atlas_png)?;
    let base = atlas_png.parent()
        .context("atlas has no parent dir")?;
    let base = base.join(&data_file.path);

    for sprite in &data_file.sprites {
        let mut path = base.join(&sprite.path);
        path.set_extension("png");

        let base = path.parent()
            .context("sprite has no base")?;

        println!("Processing {}", path.display());

        fs::create_dir_all(base)?;

        let sprite = atlas.view(
            sprite.x as u32,
            sprite.y as u32,
            sprite.width as u32,
            sprite.height as u32,
        );

        sprite.to_image().save(path)?;
    }

    Ok(())
}
