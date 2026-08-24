use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use prodex_context::{
    CriticalSignalLineRangeOptions, count_critical_signals,
    critical_signal_lost_line_ranges_with_options,
};
use std::hint::black_box;

fn context_text_boundary(c: &mut Criterion) {
    let mut sizes = c.benchmark_group("context_text_boundary/mixed");
    for count in [0, 1, 16, 64, 256, 1_024] {
        let (before, after) = corpus(count, Shape::Mixed);
        let expected = critical_signal_lost_line_ranges_with_options(
            &before,
            &after,
            CriticalSignalLineRangeOptions::default(),
        );
        sizes.throughput(Throughput::Bytes((before.len() + after.len()) as u64));
        sizes.bench_with_input(BenchmarkId::from_parameter(count), &count, |b, _| {
            b.iter(|| {
                black_box(critical_signal_lost_line_ranges_with_options(
                    black_box(&before),
                    black_box(&after),
                    CriticalSignalLineRangeOptions::default(),
                ));
            });
        });
        black_box(expected);
    }
    sizes.finish();

    let mut shapes = c.benchmark_group("context_text_boundary/64");
    for shape in [
        Shape::Ascii,
        Shape::Unicode,
        Shape::Duplicates,
        Shape::Long,
        Shape::Adversarial,
    ] {
        let (before, after) = corpus(64, shape);
        let expected = critical_signal_lost_line_ranges_with_options(
            &before,
            &after,
            CriticalSignalLineRangeOptions::default(),
        );
        shapes.throughput(Throughput::Bytes((before.len() + after.len()) as u64));
        shapes.bench_function(shape.label(), |b| {
            b.iter(|| {
                black_box(critical_signal_lost_line_ranges_with_options(
                    black_box(&before),
                    black_box(&after),
                    CriticalSignalLineRangeOptions::default(),
                ));
            });
        });
        black_box(expected);
    }
    shapes.finish();

    let (before, after) = corpus(64, Shape::Mixed);
    c.bench_function("context_text_stages/64/rust_normalize_classify", |b| {
        b.iter(|| {
            black_box((
                count_critical_signals(black_box(&before)),
                count_critical_signals(black_box(&after)),
            ));
        });
    });

    #[cfg(feature = "mojo")]
    mojo_text_ffi_boundary(c);
}

#[cfg(feature = "mojo")]
fn mojo_text_ffi_boundary(c: &mut Criterion) {
    use prodex_mojo_core::context::{ContextSignalLine, prepare_signal_rows};

    let texts = (0..64)
        .map(|index| format!("error: duplicate-{} 账户🙂\0", index % 8))
        .collect::<Vec<_>>();
    let before = texts
        .iter()
        .map(|text| ContextSignalLine {
            text,
            counts: [1, 0, 0, 0, 0, 0, 0],
        })
        .collect::<Vec<_>>();
    let after = before[..63].to_vec();
    let expected = prepare_signal_rows(&before, &after).unwrap();
    c.bench_function("context_text_stages/64/ffi_mojo_reconstruct", |b| {
        b.iter(|| black_box(prepare_signal_rows(black_box(&before), black_box(&after)).unwrap()));
    });
    black_box(expected);
}

#[derive(Clone, Copy)]
enum Shape {
    Mixed,
    Ascii,
    Unicode,
    Duplicates,
    Long,
    Adversarial,
}

impl Shape {
    fn label(self) -> &'static str {
        match self {
            Self::Mixed => "mixed",
            Self::Ascii => "ascii",
            Self::Unicode => "unicode",
            Self::Duplicates => "duplicates",
            Self::Long => "long",
            Self::Adversarial => "adversarial",
        }
    }
}

fn corpus(count: usize, shape: Shape) -> (String, String) {
    if count == 0 {
        return (String::new(), String::new());
    }
    let mut before = Vec::with_capacity(count);
    let mut after = Vec::with_capacity(count);
    for index in 0..count {
        let before_line = line(index, shape);
        after.push(before_line.clone());
        before.push(before_line);
    }
    before[count - 1] = format!("error: boundary-final-{count}");
    after[count - 1] = format!("errorx-adversarial-final-{count}");
    (
        format!("{}\n", before.join("\n")),
        format!("{}\n", after.join("\n")),
    )
}

fn line(index: usize, shape: Shape) -> String {
    match shape {
        Shape::Mixed => match index % 5 {
            0 => format!("error: duplicate-{} 账户🙂", index % 8),
            1 => format!("fatal: e\u{301}-東京-{index}"),
            2 => format!("src/火.rs:{}:2", index + 1),
            3 if index == 3 => format!("error: {}🔥", "x".repeat(8 * 1024)),
            _ => format!("errorx-prefix-{index}\0"),
        },
        Shape::Ascii => format!("error: ascii-{index}"),
        Shape::Unicode => format!("error: 账户🙂résumé-東京-δοκιμή-e\u{301}-{index}"),
        Shape::Duplicates => format!("error: duplicate-{}", index % 4),
        Shape::Long => format!("error: {}-{index}", "x".repeat(8 * 1024)),
        Shape::Adversarial => format!(
            "error{} fatal{} panic{} failed{}",
            index, index, index, index
        ),
    }
}

criterion_group!(benches, context_text_boundary);
criterion_main!(benches);
