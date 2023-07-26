mod cli;
mod color;
mod config;
mod config_io;
mod highlight_processor;
mod highlight_utils;
mod highlighters;
mod less;
mod line_info;
mod tail;

use rand::random;
use std::fs;
use std::fs::File;
use std::io::{stdin, BufWriter, IsTerminal};
use std::path::PathBuf;
use std::process::exit;
use tokio::sync::oneshot;

#[tokio::main]
async fn main() {
    let args = cli::get_args();

    if args.generate_completions_or_man_pages.is_some() {
        cli::print_completions_or_man_pages_to_stdout();

        exit(0);
    }

    let follow = should_follow(args.follow, args.tail_command.is_some());
    let is_stdin = !stdin().is_terminal();

    if args.create_default_config {
        config_io::create_default_config();

        exit(0);
    }

    if args.show_default_config {
        let default_config = config_io::default_config();

        println!("{}", default_config);

        exit(0);
    }

    let file_path = match args.file_path {
        Some(path) => path,
        None => {
            if !is_stdin && args.tail_command.is_none() {
                println!("Missing filename (`spin --help` for help) ");

                exit(0);
            }

            "".to_string()
        }
    };

    let config_path = args.config_path.clone();
    let config = config_io::load_config(config_path);

    let highlighter = highlighters::Highlighters::new(config);
    let highlight_processor = highlight_processor::HighlightProcessor::new(highlighter);

    let (_temp_dir, output_path, output_writer) = create_temp_file();
    let (reached_eof_tx, reached_eof_rx) = oneshot::channel::<()>();

    if is_stdin {
        tokio::spawn(async move {
            tail::tail_stdin(
                output_writer,
                highlight_processor,
                follow,
                Some(reached_eof_tx),
            )
            .await
            .expect("Failed to tail file");
        });
    } else if args.tail_command.is_some() {
        tokio::spawn(async move {
            tail::tail_command_output(
                output_writer,
                highlight_processor,
                Some(reached_eof_tx),
                args.tail_command.unwrap().as_str(),
            )
            .await
            .expect("Failed to tail file");
        });
    } else {
        tokio::spawn(async move {
            tail::tail_file(
                &file_path,
                follow,
                output_writer,
                highlight_processor,
                Some(reached_eof_tx),
            )
            .await
            .expect("Failed to tail file");
        });
    }

    reached_eof_rx
        .await
        .expect("Could not receive EOF signal from oneshot channel");

    if args.to_stdout {
        let contents = fs::read_to_string(&output_path).unwrap();
        println!("{}", contents);
    } else {
        less::open_file(output_path.to_str().unwrap(), follow);
    }

    cleanup(output_path);
}

fn should_follow(follow: bool, has_follow_command: bool) -> bool {
    if has_follow_command {
        return true;
    }

    follow
}

fn create_temp_file() -> (tempfile::TempDir, PathBuf, BufWriter<File>) {
    let unique_id: u32 = random();
    let filename = format!("tailspin.temp.{}", unique_id);

    let temp_dir = tempfile::tempdir().unwrap();

    let output_path = temp_dir.path().join(filename);
    let output_file = File::create(&output_path).unwrap();
    let output_writer = BufWriter::new(output_file);

    (temp_dir, output_path, output_writer)
}

fn cleanup(output_path: PathBuf) {
    if let Err(err) = fs::remove_file(output_path) {
        eprintln!("Failed to remove the temporary file: {}", err);
    }
}
