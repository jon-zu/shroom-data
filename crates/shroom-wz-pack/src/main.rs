use std::{
    collections::VecDeque,
    fs::File,
    io::{Cursor, Read},
    path::{Path, PathBuf},
};

use anyhow::Context;
use clap::{Parser, Subcommand};
use image::ImageFormat;
use shroom_wz::{
    file::{WzIO, WzImgReader},
    l0::WzImgHeader,
    l1::canvas::WzCanvas,
    val::WzValue,
    version::{WzRegion, WzVersion},
    WzConfig, WzReader,
};
use glob::glob;

use rayon::prelude::*;

struct ImgUnpacker<R> {
    root: WzValue,
    img_rdr: WzImgReader<R>,
    path: PathBuf,
}

impl<R: WzIO> ImgUnpacker<R> {
    fn new(mut img_rdr: WzImgReader<R>, path: impl AsRef<Path>) -> anyhow::Result<Self> {
        std::fs::create_dir_all(&path)?;
        let root = WzValue::read(&mut img_rdr)?;
        Ok(Self {
            img_rdr,
            path: path.as_ref().to_path_buf(),
            root,
        })
    }

    fn write_canvas(
        r: &mut WzImgReader<R>,
        mut path: PathBuf,
        canvas: &WzCanvas,
    ) -> anyhow::Result<()> {
        let file = path.with_extension("png");
        path.pop();
        std::fs::create_dir_all(&path)?;
        let mut file = std::fs::File::create(file)?;
        let img = r.read_canvas(canvas)?;
        let img = img.to_raw_rgba_image()?;
        img.write_to(&mut file, ImageFormat::Png)?;
        Ok(())
    }

    fn unpack_media(&mut self) -> anyhow::Result<()> {
        let mut q = VecDeque::new();
        let p = self.path.join("data");
        q.push_back((p, &self.root));

        while let Some((p, obj)) = q.pop_front() {
            match obj {
                WzValue::Object(v) => {
                    for (name, val) in v.0.iter() {
                        q.push_back((p.join(name), val));
                    }
                }
                WzValue::Canvas(val) => {
                    Self::write_canvas(&mut self.img_rdr, p.clone(), &val.canvas)
                        .context(anyhow::format_err!("err: {p:?}"))?;
                }
                _ => {}
            }

            //println!("unpack: {p:?}");
        }

        Ok(())
    }

    fn write_json(&self) -> anyhow::Result<()> {
        let mut file = std::fs::File::create(self.path.join("img.json"))?;
        serde_json::to_writer_pretty(&mut file, &self.root)?;
        Ok(())
    }
}

fn unpack_img<R: WzIO>(
    img_reader: WzImgReader<R>,
    path: String,
    //img: WzImgHeader,
    out_dir: &Path,
) -> anyhow::Result<()> {
    let path = path.strip_prefix("/root/").unwrap_or(&path);
    let path = out_dir.join(path);

    let p = format!("{path:?}");
    let mut unpacker = ImgUnpacker::new(img_reader, path.clone()).context(p)?;

    unpacker.write_json()?;
    unpacker.unpack_media()?;

    println!("Unpacked: {path:?}");
    Ok(())
}

fn unpack_wz_img<R: WzIO>(
    mut r: WzReader<R>,
    path: String,
    img: WzImgHeader,
    out_dir: &Path,
) -> anyhow::Result<()> {
    let img_reader = r.img_reader(&img)?;
    unpack_img(img_reader, path, out_dir)
}

fn unpack<R: WzIO + Clone + Send + Sync>(
    file: WzReader<R>,
    out_dir: impl AsRef<Path>,
) -> anyhow::Result<()> {
    let out_dir = out_dir.as_ref();
    let mut file = file;
    let imgs = file.traverse_images().collect::<anyhow::Result<Vec<_>>>()?;

    let errs = imgs
        .into_iter()
        .par_bridge()
        .flat_map(|(path, img)| unpack_wz_img(file.clone(), path, img, out_dir).err())
        .collect::<Vec<anyhow::Error>>();

    if !errs.is_empty() {
        println!("Errors:");
        for err in errs {
            println!("{:?}", err);
        }
    }

    Ok(())
}

fn img_file_unpack(file: impl AsRef<Path>, out_dir: PathBuf, cfg: WzConfig) -> anyhow::Result<()> {
    let mut data = vec![];
    let mut img_buf = File::open(file.as_ref())?;
    img_buf.read_to_end(&mut data)?;

    let r = Cursor::new(&data);
    let mut r = WzReader::open_img(r, cfg);

    let img_r = r.root_img_reader()?;
    std::fs::create_dir_all(&out_dir)?;
    unpack_img(img_r, "".to_string(), &out_dir)?;

    Ok(())
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum Region {
    Gms,
    Ems,
    Other,
    BmsSrv
}

impl Region {
    pub fn into_wz(&self) -> WzRegion {
        match self {
            Region::Gms => WzRegion::GMS,
            Region::Ems => WzRegion::SEA,
            Region::Other=> WzRegion::Other,
            Region::BmsSrv => WzRegion::BmsSrv
        }
    }
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[arg(short = 'v')]
    wz_version: Option<u16>,
    #[arg(short = 'r')]
    region: Option<Region>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Pack {
        #[arg(short, long, value_name = "file")]
        target_file: PathBuf,
        #[arg(short, long, value_name = "dir")]
        src_dir: PathBuf,
    },
    Unpack {
        #[arg(short, long, value_name = "dir")]
        target_dir: PathBuf,
        #[arg(short, long, value_name = "file")]
        src_file: PathBuf,
    },
    UnpackImg {
        #[arg(short, long, value_name = "dir")]
        target_dir: PathBuf,
        #[arg(short, long, value_name = "file")]
        src_file: PathBuf,
    },
    UnpackImgDir {
        #[arg(short, long, value_name = "dir")]
        target_dir: PathBuf,
        #[arg(short, long, value_name = "file")]
        src_dir: PathBuf,
    },
}

fn main() -> anyhow::Result<()> {
    let cmd = Cli::parse();
    let version = WzVersion(cmd.wz_version.unwrap_or(95));
    let region = cmd.region.unwrap_or(Region::Gms);
    let cfg = WzConfig::new(region.into_wz(), version.0);

    match cmd.command {
        Commands::Pack {
            target_file,
            src_dir,
        } => {
            println!("pack: {target_file:?}, {src_dir:?}");
            unimplemented!("packing not supported yet")
        }
        Commands::Unpack {
            target_dir,
            src_file,
        } => {
            let file = WzReader::open_file_mmap_shared(src_file, cfg)?;
            std::fs::create_dir_all(&target_dir)?;
            unpack(file, target_dir)?;
        }
        Commands::UnpackImg {
            target_dir,
            src_file,
        } => {
            img_file_unpack(&src_file, target_dir.clone(), cfg)?;
        }

        Commands::UnpackImgDir {
            target_dir,
            src_dir,
        } => {
            glob(&format!("{src_dir}/**/*.img", src_dir = src_dir.display()))?.par_bridge()
                .for_each(|img| {

                    let src_file = img.unwrap();
                    let dir = src_file.strip_prefix(&src_dir).unwrap();
                    if let Err(err) = img_file_unpack(&src_file, target_dir.join(dir), cfg) {
                        println!("Error: {err:?}");
                    }
                });
        }
    };

    Ok(())
}
