use std::{
    collections::BTreeMap,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, ensure};
use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use image_rs::{Rgba, RgbaImage};
use tracel_xtask_utils::environment::Environment;

use crate::context::Context;

const DEFAULT_SIZES: &str = "16,32,48,64,128,256,512,1024";
const MAX_ICO_SIZE: u32 = 256;
const MAX_PNG_SIZE: u32 = 4096;
const SVG_NAMESPACE: &str = "http://www.w3.org/2000/svg";

#[tracel_xtask_macros::declare_command_args(None, None)]
pub struct IconsCmdArgs {
    /// SVG file to render.
    #[arg(value_name = "SVG")]
    pub source: PathBuf,

    /// Directory for generated files. Defaults to the SVG file's directory.
    #[arg(short, long, alias = "output", value_name = "DIR")]
    pub output_dir: Option<PathBuf>,

    /// Place the SVG on a rounded, beveled 3D application-icon tile.
    #[arg(long)]
    pub decorated: bool,

    /// Comma-separated square PNG sizes in pixels.
    #[arg(
        long,
        value_name = "PIXELS,PIXELS,...",
        value_delimiter = ',',
        default_value = DEFAULT_SIZES
    )]
    pub sizes: Vec<u32>,
}

pub fn handle_command(args: IconsCmdArgs, _env: Environment, _ctx: Context) -> anyhow::Result<()> {
    let outputs = generate_outputs(&args)?;
    let (written, unchanged) = write_outputs(&outputs)?;
    let output_dir = output_dir(&args);
    println!(
        "Icons: wrote {written}, unchanged {unchanged}, output {}",
        output_dir.display()
    );
    Ok(())
}

fn generate_outputs(args: &IconsCmdArgs) -> anyhow::Result<BTreeMap<PathBuf, Vec<u8>>> {
    let sizes = normalize_sizes(&args.sizes)?;
    let svg =
        fs::read(&args.source).with_context(|| format!("reading SVG {}", args.source.display()))?;
    let options = svg_options(&args.source);
    let tree = resvg::usvg::Tree::from_data(&svg, &options)
        .with_context(|| format!("parsing SVG {}", args.source.display()))?;
    validate_text_fonts(&svg, &options, &args.source)?;
    let stem = args.source.file_stem().with_context(|| {
        format!(
            "SVG path has no file name to use for generated icons: {}",
            args.source.display()
        )
    })?;
    let output_dir = output_dir(args);
    let mut outputs = BTreeMap::new();
    let mut icon = IconDir::new(ResourceType::Icon);

    for size in sizes {
        let foreground_size = if args.decorated {
            decorated_foreground_size(size)
        } else {
            size
        };
        let foreground = render_svg(&tree, foreground_size)?;
        ensure!(
            foreground.data().chunks_exact(4).any(|pixel| pixel[3] != 0),
            "SVG rendered no visible pixels at {size}x{size}; check its artwork: {}",
            args.source.display()
        );
        let pixmap = if args.decorated {
            decorate_icon(foreground, size)?
        } else {
            foreground
        };
        let png = pixmap
            .encode_png()
            .with_context(|| format!("encoding {size}x{size} PNG"))?;
        outputs.insert(
            output_dir.join(output_name(stem, &format!("-{size}.png"))),
            png,
        );

        if size <= MAX_ICO_SIZE {
            // tiny-skia renders premultiplied RGBA. ICO expects straight-alpha RGBA.
            let image = IconImage::from_rgba_data(size, size, pixmap.take_demultiplied());
            let entry = IconDirEntry::encode_as_png(&image)
                .with_context(|| format!("encoding {size}x{size} ICO entry"))?;
            icon.add_entry(entry);
        }
    }

    let mut ico = Vec::new();
    icon.write(&mut ico)
        .context("encoding multi-resolution ICO")?;
    outputs.insert(output_dir.join(output_name(stem, ".ico")), ico);
    Ok(outputs)
}

fn svg_options(source: &Path) -> resvg::usvg::Options<'static> {
    let mut options = resvg::usvg::Options {
        resources_dir: source.parent().map(Path::to_path_buf),
        ..resvg::usvg::Options::default()
    };
    let fontdb = options.fontdb_mut();
    fontdb.load_system_fonts();

    // The database's default serif family is not installed on every platform.
    // Keep explicit SVG font families intact while providing an installed fallback.
    let fallback = [
        "Times New Roman",
        "DejaVu Serif",
        "Liberation Serif",
        "Noto Serif",
        "Arial",
        "Helvetica",
        "DejaVu Sans",
        "Liberation Sans",
        "Noto Sans",
    ]
    .into_iter()
    .find(|candidate| {
        fontdb
            .faces()
            .any(|face| face.families.iter().any(|(family, _)| family == candidate))
    })
    .map(str::to_owned)
    .or_else(|| {
        fontdb
            .faces()
            .find_map(|face| face.families.first().map(|(family, _)| family.clone()))
    });

    if let Some(fallback) = fallback {
        fontdb.set_serif_family(fallback.clone());
        options.font_family = fallback;
    }
    options
}

fn validate_text_fonts(
    svg: &[u8],
    options: &resvg::usvg::Options<'_>,
    source: &Path,
) -> anyhow::Result<()> {
    if !options.fontdb.is_empty() {
        return Ok(());
    }

    let decompressed = if svg.starts_with(&[0x1f, 0x8b]) {
        Some(resvg::usvg::decompress_svgz(svg).context("decompressing SVGZ data")?)
    } else {
        None
    };
    let xml = std::str::from_utf8(decompressed.as_deref().unwrap_or(svg))
        .context("SVG data is not UTF-8")?;
    let parsing_options = resvg::usvg::roxmltree::ParsingOptions {
        allow_dtd: true,
        ..Default::default()
    };
    let document = resvg::usvg::roxmltree::Document::parse_with_options(xml, parsing_options)
        .with_context(|| format!("parsing SVG {}", source.display()))?;
    let contains_text = document
        .descendants()
        .any(|node| node.has_tag_name((SVG_NAMESPACE, "text")));

    ensure!(
        !contains_text,
        "SVG contains text but no system fonts are available; install a font or convert text to paths: {}",
        source.display()
    );
    Ok(())
}

fn normalize_sizes(sizes: &[u32]) -> anyhow::Result<Vec<u32>> {
    ensure!(!sizes.is_empty(), "--sizes must contain at least one size");
    ensure!(
        sizes.iter().all(|size| (1..=MAX_PNG_SIZE).contains(size)),
        "--sizes values must be between 1 and {MAX_PNG_SIZE} pixels"
    );
    ensure!(
        sizes.iter().any(|size| *size <= MAX_ICO_SIZE),
        "--sizes must contain at least one value no larger than {MAX_ICO_SIZE} pixels for the ICO"
    );

    let mut sizes = sizes.to_vec();
    sizes.sort_unstable();
    sizes.dedup();
    Ok(sizes)
}

fn render_svg(tree: &resvg::usvg::Tree, size: u32) -> anyhow::Result<resvg::tiny_skia::Pixmap> {
    let svg_size = tree.size();
    let scale = (size as f32 / svg_size.width()).min(size as f32 / svg_size.height());
    let x = (size as f32 - svg_size.width() * scale) / 2.0;
    let y = (size as f32 - svg_size.height() * scale) / 2.0;
    let transform = resvg::tiny_skia::Transform::from_row(scale, 0.0, 0.0, scale, x, y);
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)
        .with_context(|| format!("allocating {size}x{size} icon canvas"))?;
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

fn decorated_foreground_size(size: u32) -> u32 {
    (size * 19 / 25).max(1)
}

fn decorate_icon(
    foreground: resvg::tiny_skia::Pixmap,
    size: u32,
) -> anyhow::Result<resvg::tiny_skia::Pixmap> {
    const TILE_INSET_RATIO: f64 = 23.0 / 512.0;
    const TILE_RADIUS_RATIO: f64 = 100.0 / 466.0;
    const SUPERSAMPLE_GRID: u32 = 4;

    let extent = f64::from(size);
    let inset = extent * TILE_INSET_RATIO;
    let tile_size = extent - 2.0 * inset;
    let radius = tile_size * TILE_RADIUS_RATIO;
    let min_center = inset + radius;
    let max_center = extent - inset - radius;
    let samples = SUPERSAMPLE_GRID * SUPERSAMPLE_GRID;

    let mut face = RgbaImage::from_fn(size, size, |x, y| {
        let mut covered = 0_u32;
        for sample_y in 0..SUPERSAMPLE_GRID {
            for sample_x in 0..SUPERSAMPLE_GRID {
                let px = f64::from(x) + (f64::from(sample_x) + 0.5) / f64::from(SUPERSAMPLE_GRID);
                let py = f64::from(y) + (f64::from(sample_y) + 0.5) / f64::from(SUPERSAMPLE_GRID);
                let nearest_x = px.clamp(min_center, max_center);
                let nearest_y = py.clamp(min_center, max_center);
                let dx = px - nearest_x;
                let dy = py - nearest_y;
                if dx * dx + dy * dy <= radius * radius {
                    covered += 1;
                }
            }
        }
        let alpha = u8::try_from((covered * u32::from(u8::MAX) + samples / 2) / samples)
            .expect("supersampled alpha is bounded to one byte");
        Rgba([0, 0, 0, alpha])
    });
    render_decorated_tile_surface(&mut face, size, min_center, max_center, radius);

    let mut shadow = image_rs::imageops::blur(&face, size as f32 * 6.0 / 512.0);
    for pixel in shadow.pixels_mut() {
        pixel.0[..3].fill(0);
        pixel.0[3] = u8::try_from(u16::from(pixel.0[3]) * 90 / 255).expect("shadow alpha fits u8");
    }

    let mut canvas = RgbaImage::new(size, size);
    image_rs::imageops::overlay(&mut canvas, &shadow, 0, i64::from(size * 6 / 512));
    image_rs::imageops::overlay(&mut canvas, &face, 0, 0);

    let foreground_width = foreground.width();
    let foreground_height = foreground.height();
    let foreground = RgbaImage::from_raw(
        foreground_width,
        foreground_height,
        foreground.take_demultiplied(),
    )
    .context("converting rendered SVG for icon decoration")?;
    let offset_x = i64::from((size - foreground_width) / 2);
    let offset_y = i64::from((size - foreground_height) / 2);
    image_rs::imageops::overlay(&mut canvas, &foreground, offset_x, offset_y);

    // The rounded tile is the icon background, not transparency. Integer
    // compositing must not leave nearly-opaque pixels in its interior.
    for (pixel, face_pixel) in canvas.pixels_mut().zip(face.pixels()) {
        if face_pixel.0[3] == u8::MAX {
            pixel.0[3] = u8::MAX;
        }
    }

    rgba_image_to_pixmap(canvas)
}

fn render_decorated_tile_surface(
    face: &mut RgbaImage,
    size: u32,
    min_center: f64,
    max_center: f64,
    radius: f64,
) {
    const BEVEL_WIDTH_RATIO: f64 = 14.0 / 512.0;
    const RIM_WIDTH_RATIO: f64 = 4.0 / 512.0;
    const EDGE_GUARD_RATIO: f64 = 1.0 / 512.0;
    const FACE_FLOOR: u8 = 5;
    const FACE_GRADIENT: u8 = 57;
    const FACE_FALLOFF_POWER: f64 = 1.1;
    const BEVEL_STRENGTH: u8 = 64;
    const RIM_STRENGTH: u8 = 14;
    const MAX_CHARCOAL: u8 = 80;
    const LIGHT_X: f64 = 0.8;
    const LIGHT_Y: f64 = 0.6;

    let extent = f64::from(size);
    let bevel_width = extent * BEVEL_WIDTH_RATIO;
    let rim_width = extent * RIM_WIDTH_RATIO;
    let edge_guard = extent * EDGE_GUARD_RATIO;
    if bevel_width <= edge_guard {
        return;
    }

    for (x, y, pixel) in face.enumerate_pixels_mut() {
        // Keeping partially transparent contour pixels black prevents a pale
        // halo when the icon is composited over a light background.
        if pixel.0[3] != u8::MAX {
            continue;
        }

        let px = f64::from(x) + 0.5;
        let py = f64::from(y) + 0.5;
        let broad_light = (1.0 - (px + py) / extent)
            .clamp(0.0, 1.0)
            .powf(FACE_FALLOFF_POWER);
        let mut requested = f64::from(FACE_FLOOR) + f64::from(FACE_GRADIENT) * broad_light;

        let nearest_x = px.clamp(min_center, max_center);
        let nearest_y = py.clamp(min_center, max_center);
        let dx = px - nearest_x;
        let dy = py - nearest_y;
        let radial_distance = dx.hypot(dy);
        if radial_distance > f64::EPSILON {
            let inward_distance = radius - radial_distance;
            let normal_light = (-(LIGHT_X * dx + LIGHT_Y * dy) / radial_distance).clamp(0.0, 1.0);
            if inward_distance > edge_guard && inward_distance < bevel_width {
                let position = (inward_distance - edge_guard) / (bevel_width - edge_guard);
                let profile = 4.0 * position * (1.0 - position);
                requested += f64::from(BEVEL_STRENGTH) * normal_light * profile;
            }
            if inward_distance > edge_guard && inward_distance < rim_width {
                let position = (inward_distance - edge_guard) / (rim_width - edge_guard);
                let profile = 4.0 * position * (1.0 - position);
                requested += f64::from(RIM_STRENGTH) * (1.0 - normal_light) * profile;
            }
        }

        let charcoal = (1..=MAX_CHARCOAL)
            .rev()
            .find(|level| requested >= f64::from(*level) - 0.5)
            .unwrap_or(0);
        pixel.0[..3].fill(charcoal);
    }
}

fn rgba_image_to_pixmap(image: RgbaImage) -> anyhow::Result<resvg::tiny_skia::Pixmap> {
    let mut pixmap = resvg::tiny_skia::Pixmap::new(image.width(), image.height())
        .context("allocating decorated icon canvas")?;
    for (destination, source) in pixmap.pixels_mut().iter_mut().zip(image.pixels()) {
        *destination = resvg::tiny_skia::ColorU8::from_rgba(
            source.0[0],
            source.0[1],
            source.0[2],
            source.0[3],
        )
        .premultiply();
    }
    Ok(pixmap)
}

fn output_dir(args: &IconsCmdArgs) -> PathBuf {
    args.output_dir.clone().unwrap_or_else(|| {
        args.source
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf()
    })
}

fn output_name(stem: &OsStr, suffix: &str) -> OsString {
    let mut name = stem.to_os_string();
    name.push(suffix);
    name
}

fn write_outputs(outputs: &BTreeMap<PathBuf, Vec<u8>>) -> anyhow::Result<(usize, usize)> {
    let mut written = 0;
    let mut unchanged = 0;

    for (path, bytes) in outputs {
        if fs::read(path).ok().as_deref() == Some(bytes) {
            unchanged += 1;
            continue;
        }
        let parent = path
            .parent()
            .with_context(|| format!("generated path has no parent: {}", path.display()))?;
        fs::create_dir_all(parent).with_context(|| format!("creating {}", parent.display()))?;
        fs::write(path, bytes).with_context(|| format!("writing {}", path.display()))?;
        written += 1;
    }

    Ok((written, unchanged))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use clap::Parser as _;
    use tempfile::tempdir;

    use super::*;

    const SQUARE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <circle cx="50" cy="50" r="37.5" fill="#e94b9c"/>
</svg>"##;

    fn args(source: PathBuf, output_dir: PathBuf, sizes: Vec<u32>) -> IconsCmdArgs {
        IconsCmdArgs {
            source,
            output_dir: Some(output_dir),
            decorated: false,
            sizes,
        }
    }

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(flatten)]
        icons: IconsCmdArgs,
    }

    #[test]
    fn parses_defaults_and_the_output_alias() {
        let defaults = TestCli::try_parse_from(["test", "assets/app.svg"]).unwrap();
        assert_eq!(
            defaults.icons.sizes,
            vec![16, 32, 48, 64, 128, 256, 512, 1024]
        );
        assert_eq!(defaults.icons.output_dir, None);
        assert!(!defaults.icons.decorated);
        assert_eq!(output_dir(&defaults.icons), PathBuf::from("assets"));

        let custom = TestCli::try_parse_from([
            "test",
            "app.svg",
            "--output",
            "generated",
            "--decorated",
            "--sizes",
            "64,16,16",
        ])
        .unwrap();
        assert_eq!(custom.icons.output_dir, Some(PathBuf::from("generated")));
        assert!(custom.icons.decorated);
        assert_eq!(normalize_sizes(&custom.icons.sizes).unwrap(), vec![16, 64]);
    }

    #[test]
    fn generates_sorted_pngs_and_a_multisize_ico() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("app-icon.svg");
        let output = temp.path().join("generated");
        fs::write(&source, SQUARE_SVG).unwrap();
        let args = args(source, output.clone(), vec![512, 32, 16, 64, 32]);

        let generated = generate_outputs(&args).unwrap();
        assert_eq!(
            generated.keys().collect::<Vec<_>>(),
            [
                output.join("app-icon-16.png"),
                output.join("app-icon-32.png"),
                output.join("app-icon-512.png"),
                output.join("app-icon-64.png"),
                output.join("app-icon.ico"),
            ]
            .iter()
            .collect::<Vec<_>>()
        );

        for size in [16, 32, 64, 512] {
            let path = output.join(format!("app-icon-{size}.png"));
            let image = resvg::tiny_skia::Pixmap::decode_png(&generated[&path]).unwrap();
            assert_eq!((image.width(), image.height()), (size, size));
            assert!(image.data().chunks_exact(4).any(|pixel| pixel[3] == 0));
            assert!(image.data().chunks_exact(4).any(|pixel| pixel[3] == 255));
            assert!(
                image
                    .data()
                    .chunks_exact(4)
                    .any(|pixel| (1..=254).contains(&pixel[3]))
            );
        }

        let icon = IconDir::read(Cursor::new(&generated[&output.join("app-icon.ico")])).unwrap();
        let dimensions = icon
            .entries()
            .iter()
            .map(|entry| {
                assert!(entry.is_png());
                assert_eq!(entry.bits_per_pixel(), 32);
                (entry.width(), entry.height())
            })
            .collect::<Vec<_>>();
        assert_eq!(dimensions, vec![(16, 16), (32, 32), (64, 64)]);

        assert_eq!(write_outputs(&generated).unwrap(), (5, 0));
        assert_eq!(write_outputs(&generated).unwrap(), (0, 5));
    }

    #[test]
    fn preserves_aspect_ratio_with_transparent_padding() {
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 200 100">
  <rect width="200" height="100" fill="#ffffff"/>
</svg>"##;
        let options = resvg::usvg::Options::default();
        let tree = resvg::usvg::Tree::from_data(svg, &options).unwrap();
        let image = render_svg(&tree, 32).unwrap();

        assert!(image.data()[3] == 0);
        let center = ((16 * 32 + 16) * 4 + 3) as usize;
        assert_eq!(image.data()[center], 255);
    }

    #[test]
    fn renders_svg_text_or_reports_missing_system_fonts() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("text.svg");
        let output = temp.path().join("generated");
        let fonts_available = !svg_options(&source).fontdb.is_empty();

        fs::write(
            &source,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <text x="50" y="72" text-anchor="middle" font-size="72" fill="#202020">A</text>
</svg>"##,
        )
        .unwrap();

        let generated = generate_outputs(&args(source, output.clone(), vec![64]));
        if fonts_available {
            let generated = generated.unwrap();
            let image =
                resvg::tiny_skia::Pixmap::decode_png(&generated[&output.join("text-64.png")])
                    .unwrap();
            assert!(image.data().chunks_exact(4).any(|pixel| pixel[3] != 0));
        } else {
            assert!(
                generated
                    .unwrap_err()
                    .to_string()
                    .contains("no system fonts")
            );
        }
    }

    #[test]
    fn rejects_an_svg_with_no_visible_pixels() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("empty.svg");
        fs::write(
            &source,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100"/>"##,
        )
        .unwrap();

        let error = generate_outputs(&args(
            source.clone(),
            temp.path().join("generated"),
            vec![16],
        ))
        .unwrap_err()
        .to_string();

        assert!(error.contains("no visible pixels"));
        assert!(error.contains(&source.display().to_string()));
    }

    #[test]
    fn rejects_each_blank_requested_size() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("fine-detail.svg");
        fs::write(
            &source,
            r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect x="49.75" y="49.75" width="0.5" height="0.5" fill="#202020"/>
</svg>"##,
        )
        .unwrap();

        let options = svg_options(&source);
        let tree = resvg::usvg::Tree::from_data(&fs::read(&source).unwrap(), &options).unwrap();
        assert!(
            render_svg(&tree, 1)
                .unwrap()
                .data()
                .chunks_exact(4)
                .all(|pixel| pixel[3] == 0)
        );
        assert!(
            render_svg(&tree, 64)
                .unwrap()
                .data()
                .chunks_exact(4)
                .any(|pixel| pixel[3] != 0)
        );

        let error = generate_outputs(&args(
            source.clone(),
            temp.path().join("generated"),
            vec![1, 64],
        ))
        .unwrap_err()
        .to_string();
        assert!(error.contains("1x1"));
        assert!(error.contains(&source.display().to_string()));
    }

    #[test]
    fn rejects_text_with_an_empty_font_database_even_when_paths_are_visible() {
        let source = Path::new("mixed.svg");
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100">
  <rect width="100" height="100" fill="#ffffff"/>
  <text x="50" y="50">A</text>
</svg>"##;
        let options = resvg::usvg::Options::default();

        let error = validate_text_fonts(svg, &options, source)
            .unwrap_err()
            .to_string();
        assert!(error.contains("no system fonts"));
        assert!(error.contains(&source.display().to_string()));
    }

    #[test]
    fn ignores_text_elements_outside_the_svg_namespace() {
        let source = Path::new("metadata.svg");
        let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:meta="urn:meta" viewBox="0 0 100 100">
  <metadata><meta:text>descriptive metadata</meta:text></metadata>
  <rect width="100" height="100" fill="#ffffff"/>
</svg>"##;
        let options = resvg::usvg::Options::default();

        validate_text_fonts(svg, &options, source).unwrap();
    }

    #[test]
    fn rejects_sizes_that_cannot_produce_all_formats() {
        assert!(
            normalize_sizes(&[])
                .unwrap_err()
                .to_string()
                .contains("at least one")
        );
        assert!(
            normalize_sizes(&[0, 16])
                .unwrap_err()
                .to_string()
                .contains("between 1")
        );
        assert!(
            normalize_sizes(&[512, 1024])
                .unwrap_err()
                .to_string()
                .contains("for the ICO")
        );
        assert!(
            normalize_sizes(&[MAX_PNG_SIZE + 1, 16])
                .unwrap_err()
                .to_string()
                .contains(&MAX_PNG_SIZE.to_string())
        );
    }

    #[test]
    fn reports_invalid_svg_with_its_path() {
        let temp = tempdir().unwrap();
        let source = temp.path().join("broken.svg");
        fs::write(&source, "not svg").unwrap();
        let error = generate_outputs(&args(
            source.clone(),
            temp.path().join("generated"),
            vec![16],
        ))
        .unwrap_err()
        .to_string();

        assert!(error.contains("parsing SVG"));
        assert!(error.contains(&source.display().to_string()));
    }
}
