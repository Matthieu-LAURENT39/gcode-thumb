use std::path::Path;
use std::{fs, hint::black_box};

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use gcode_thumbnailer::RenderThumbnailVisitor;

fn benchmark_render_thumbnail(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_thumbnail");

    // Try against some real-world G-code files
    // It's a 3DBenchy model, so it's not the biggest print
    // TODO: add a bigger model too
    let inputs = [
        Path::new("benches/data/cura_3DBenchy_skirt_support.gcode"),
        Path::new("benches/data/orca_3DBenchy_default.gcode"),
    ];
    // Try various common thumbnail sizes
    let sizes = [128, 256, 512, 1024];

    for gcode_path in inputs {
        let content = fs::read_to_string(gcode_path)
            .unwrap_or_else(|err| panic!("Failed to read {}: {err}", gcode_path.display()));
        // Measure the throughput in lines and bytes, to get an idea of how the rendering time scales with the input size
        // Lines is a pretty good mapping to the number of commands to process.
        group.throughput(Throughput::ElementsAndBytes {
            elements: content.lines().count() as u64,
            bytes: content.len() as u64,
        });

        for size in sizes {
            let bench_label = format!(
                "{}_{size}",
                gcode_path.file_stem().unwrap().to_string_lossy()
            );

            group.bench_with_input(
                BenchmarkId::new("render_thumbnail", bench_label),
                &content,
                |b, data| {
                    b.iter(|| {
                        let mut visitor = RenderThumbnailVisitor::new(true, true);
                        gcode::core::parse(black_box(data), &mut visitor);
                        let image = visitor
                            .render(tiny_skia::Color::from_rgba8(0, 0, 0, 0), size)
                            .expect("Failed to render thumbnail");
                        black_box(image);
                    });
                },
            );
        }
    }

    group.finish();
}

criterion_group!(benches, benchmark_render_thumbnail);
criterion_main!(benches);
