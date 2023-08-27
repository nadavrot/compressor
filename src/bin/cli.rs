//! This is the command line tool that loads an input file and either compresses
//! or decompresses it.

extern crate clap;
extern crate env_logger;
extern crate log;

use clap::{Arg, ArgAction, Command};
use compressor::full::{FullDecoder, FullEncoder};
use compressor::lz::{LZ4Decoder, LZ4Encoder};
use compressor::utils::signatures::{FILE_EXTENSION, FULL_SIG, LZ4_SIG};
use compressor::{Decoder, Encoder};

use std::{fs, time::Instant};
use std::{fs::File, io::Write};

fn save_file(data: &[u8], path: &str) {
    let mut f = File::create(path).expect("Can't create file");
    f.write_all(data).expect("Unable to write data");
    log::info!("Wrote {}.", &path);
}

/// A scoped utility struct for measuring and reporting time.
struct Timer {
    start: std::time::Instant,
}

impl Timer {
    fn new() -> Self {
        Self {
            start: Instant::now(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        let now = Instant::now();
        if let Some(duration) = now.checked_duration_since(self.start) {
            log::info!(
                "Operation completed in {:03} seconds",
                duration.as_secs_f32()
            );
        }
    }
}

fn handle_buffers(
    is_compress: bool,
    is_full: bool,
    input: &[u8],
    output: &mut Vec<u8>,
) -> Option<(usize, usize)> {
    if is_compress {
        if is_full {
            log::info!("Compressing using the Full compressor");
            let mut encoder = FullEncoder::new(input, output);
            let written = encoder.encode();
            return Some((input.len(), written));
        }

        log::info!("Compressing using the LZ4 compressor");
        output.extend(LZ4_SIG);
        let mut encoder = LZ4Encoder::new(input, output);
        let written = encoder.encode();
        return Some((input.len(), written));
    }

    // Try to decompress.
    if input.starts_with(&LZ4_SIG) {
        log::info!("Decompressing LZ4 compression");
        let mut decoder = LZ4Decoder::new(&input[LZ4_SIG.len()..], output);
        let stat = decoder.decode();
        return stat;
    }

    if input.starts_with(&FULL_SIG) {
        log::info!("Decompressing the Full compression");
        let mut decoder = FullDecoder::new(input, output);
        let stat = decoder.decode();
        return stat;
    }

    None
}

fn main() {
    let matches = Command::new("CLI")
        .version("1.x")
        .arg(
            Arg::new("checked")
                .long("check")
                .help("Enables checked-mode")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("decompress")
                .short('d')
                .long("decompress")
                .help("Try to decompress the input")
                .action(ArgAction::SetTrue)
                .conflicts_with("compress"),
        )
        .arg(
            Arg::new("compress")
                .short('c')
                .long("compress")
                .help("Compress the input")
                .conflicts_with("decompress")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("output")
                .short('o')
                .long("output")
                .value_name("FILE")
                .help("Path of the output file")
                .num_args(1),
        )
        .arg(
            Arg::new("mode")
                .long("mode")
                .value_name("mode")
                .help("The algorithm used for compression.")
                .value_parser(["lz4", "full"])
                .num_args(1),
        )
        .arg(
            Arg::new("INPUT")
                .help("Sets the input file to use")
                .required(true)
                .index(1),
        )
        .get_matches();

    env_logger::builder().format_timestamp(None).init();

    let mut cli_compress = matches.get_flag("compress");
    let cli_decompress = matches.get_flag("decompress");
    let cli_checked_mode = matches.get_flag("checked");
    let mut cli_output_path = matches.get_one::<String>("output").cloned();
    let cli_mode = matches
        .get_one::<String>("mode")
        .cloned()
        .unwrap_or_else(|| String::from("full"));

    let input_path = matches.get_one::<String>("INPUT").unwrap();
    let input = fs::read(input_path).expect("Can't open the input file");

    // The user did not specify if this is compress of decompress. Try to figure
    // out using the extension.
    let ends_with_ext = input_path.ends_with(FILE_EXTENSION);
    if !cli_compress && !cli_decompress && !ends_with_ext {
        cli_compress = true;
    }

    // Come up with a file name.
    if cli_output_path.is_none() {
        if input_path.ends_with(FILE_EXTENSION) {
            // remove the extension.
            let end = input_path.len() - FILE_EXTENSION.len();
            cli_output_path = Some(String::from(&input_path[0..end]));
        } else {
            // Add the extension.
            cli_output_path = Some(input_path.clone() + FILE_EXTENSION);
        }
    }

    let mode = cli_mode == "full";
    let out = &cli_output_path.unwrap();
    let mut dest = Vec::new();
    let x = Timer::new();

    if cli_compress {
        if let Some((from, to)) = handle_buffers(true, mode, &input, &mut dest)
        {
            log::info!("Compressed from {} to {} bytes.", from, to);
            log::info!("Compression ratio is {:.4}x.", from as f64 / to as f64);
            save_file(&dest, out);
        } else {
            log::info!("Compression failed");
            return;
        }

        if cli_checked_mode {
            let mut decoded = Vec::new();

            if let Some((from, to)) =
                handle_buffers(false, mode, &dest, &mut decoded)
            {
                log::info!("Decompressed from {} to {} bytes.", from, to);
                if input == decoded {
                    log::info!("Correct!");
                    return;
                } else {
                    log::info!("Incorrect!");
                    return;
                }
            } else {
                log::info!("Could not decompress the file!");
                return;
            }
        }

        return;
    }

    if let Some((from, to)) = handle_buffers(false, mode, &input, &mut dest) {
        log::info!("Decompressed from {} to {} bytes.", from, to);
        save_file(&dest, out);
    } else {
        log::info!("Decompression failed");
    }

    drop(x);
}
