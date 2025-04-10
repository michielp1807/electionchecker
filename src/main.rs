#[cfg(feature = "validate")]
use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use kiesraad_model::*;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run some example elections
    Demo,
    /// Run an election with the provided number of seats and votes
    Allocate(AllocateArgs),
    /// Validate election results from CSV file(s)
    #[cfg(feature = "validate")]
    Validate { files: Vec<PathBuf> },
}

#[derive(Args)]
struct AllocateArgs {
    /// Total number of seats to allocate
    seats: u64,
    /// Number of votes per party
    #[arg(short, long)]
    #[clap(num_args = 1.., value_delimiter=',')]
    votes: Vec<u64>,
    /// Number of candidates per party
    #[arg(short, long, default_value=None)]
    #[clap(num_args = 1.., value_delimiter=',')]
    candidates: Option<Vec<u64>>,
    /// Use a voting threshold of one whole seat, as used in Dutch national elections
    #[arg(short, long)]
    national: bool,
}

fn main() {
    println!(
        "Copyright (C) 2025  Marc Schoolderman
This program comes with ABSOLUTELY NO WARRANTY
This is free software, and you are welcome to redistribute it
under certain conditions, see the file LICENSE
"
    );

    let cli = Cli::parse();

    match &cli.command {
        Command::Demo => demo(),
        Command::Allocate(args) => {
            let votes = args.votes.iter().map(|v| Votes(*v)).collect::<Vec<Votes>>();
            println!(
                "running an election for {} seats, parties: {votes:?}, using largest {}",
                args.seats,
                if args.national {
                    "averages (with voting threshold of one whole seat)"
                } else if args.seats >= 19 {
                    "averages"
                } else {
                    "surpluses"
                }
            );
            let mut seats = if let Some(cands) = &args.candidates {
                cands.iter().cloned().map(Seats::limited).collect()
            } else {
                vec![Seats::unlimited(); votes.len()]
            };
            if args.national {
                allocate_national(Seats::filled(args.seats), &votes, &mut seats);
            } else {
                allocate(Seats::filled(args.seats), &votes, &mut seats);
            }
            print_seats(seats.into_iter());
        }
        #[cfg(feature = "validate")]
        Command::Validate { files } => {
            println!("Validating {} files...", files.len());
            validate(files);
        }
    }
}

fn print_seats(seats: impl Iterator<Item = Seats>) {
    print!("result = ");
    for seat in seats {
        print!("{seat}, ");
    }
    println!();
}

fn demo() {
    macro_rules! votes {
    ($($x: expr),* $(,)?) => {
        vec![$(Votes($x),)*]
    }
    }

    fn run_election(target: Count, votes: Vec<Votes>) {
        println!(
            "running an election for {target} seats, parties: {votes:?}, using largest {}",
            if target >= 19 {
                "averages"
            } else {
                "surpluses"
            }
        );
        let mut seats = vec![Seats::unlimited(); votes.len()];
        allocate(Seats::filled(target), &votes, &mut seats);
        print_seats(seats.into_iter());
        println!("======");
    }

    run_election(19, votes![40, 30, 20, 10]);
    run_election(24, votes![21, 20]);
    run_election(20, votes![51, 25, 25]);
    run_election(50, votes![26, 25]);
    run_election(10, votes![30, 12, 8, 8]);

    run_election(5, votes![19, 19, 19, 19, 15, 9, 9]);
    run_election(18, votes![100, 16, 6, 5, 5, 5, 5, 4]);

    fn run_national_election(votes: Vec<Votes>) {
        println!("running an election for Tweede Kamer");
        let mut seats = vec![Seats::unlimited(); votes.len()];
        allocate_national(Seats::filled(150), &votes, &mut seats);
        print_seats(seats.into_iter());
        println!("======");
    }

    #[rustfmt::skip]
    run_national_election(votes![
        2_450_878,
        1_643_073,
        1_589_519,
        1_343_287,
          656_292,
          485_551,
          345_822,
          328_225,
          246_765,
          235_148,
          232_963,
          217_270,
          212_532,
          178_802,
           71_345,
           52_913,
           51_043,
           44_253,
           12_838,
            9_117,
            5_487,
            5_325,
            5_122,
            4_152,
            3_966,
            1_038,
    ]);

    println!("a corner case in our national voting system");
    let votes = votes![33, 7];
    let mut seats = vec![Seats::limited(2), Seats::limited(13)];
    allocate(Seats::filled(4), &votes, &mut seats);
    print_seats(seats.into_iter());

    println!("a weird consequence of a little sentence in the law");
    let votes = votes![33, 7, 0];
    let mut seats = vec![Seats::limited(2), Seats::limited(12), Seats::limited(2)];
    allocate(Seats::filled(4), &votes, &mut seats);
    print_seats(seats.into_iter());
}

#[cfg(feature = "validate")]
fn validate(data_sources: &Vec<PathBuf>) {
    for data_source in data_sources {
        let records = csv::ReaderBuilder::new()
            .has_headers(true)
            .delimiter(b';')
            .from_path(data_source)
            .unwrap()
            .records()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        let ignored = |s: &str| {
            [
                "AantalBlancoStemmen",
                "AantalGeldigeStemmen",
                "AantalOngeldigeStemmen",
                "Kiesgerechtigden",
                "Opkomst",
            ]
            .contains(&s)
        };

        let records = records.chunk_by(|x, y| x[1] == y[1]).map(|record| {
            (
                &record[0][0],
                record
                    .iter()
                    .filter_map(|x| {
                        (!ignored(&x[2])).then_some(Votes(x[4].parse().unwrap_or_default()))
                    })
                    .collect::<Vec<_>>(),
                record
                    .iter()
                    .filter_map(|x| {
                        (!ignored(&x[2])).then_some(Seats::filled(x[5].parse().unwrap_or_default()))
                    })
                    .collect::<Vec<_>>(),
                record
                    .iter()
                    .filter_map(|x| {
                        (!ignored(&x[2])).then_some(
                            x[6].parse()
                                .map(Seats::limited)
                                .unwrap_or(Seats::unlimited()),
                        )
                    })
                    .collect::<Vec<_>>(),
            )
        });

        for (id, ref votes, outcome, candidates) in records {
            #[cfg(feature = "rand-validate")]
            let (ref votes, outcome): (Vec<_>, Vec<_>) = {
                use rand::seq::SliceRandom;
                let mut mingle = std::iter::zip(votes, outcome).collect::<Vec<_>>();
                mingle.shuffle(&mut rand::rng());
                mingle.into_iter().unzip()
            };

            let total_seats = outcome.iter().map(|x| x.count()).sum();
            println!("checking {}:{id}", data_source.display());

            let mut seats = candidates;

            let file_name = data_source.file_name().unwrap().to_string_lossy();
            if file_name.starts_with("uitslag_TK") || file_name.starts_with("uitslag_EP") {
                match &file_name[10..14] {
                    "1918" => allocate_1918(Seats::filled(total_seats), votes, &mut seats),
                    "1922" => allocate_1922(Seats::filled(total_seats), votes, &mut seats),
                    "1925" | "1929" | "1933" => {
                        allocate_bongaerts(Seats::filled(total_seats), votes, &mut seats)
                    }
                    _ => allocate_national(Seats::filled(total_seats), votes, &mut seats),
                }
            } else {
                allocate(Seats::filled(total_seats), votes, &mut seats);
            }

            assert_eq!(
                seats.iter().map(|x| x.count()).collect::<Vec<_>>(),
                outcome.iter().map(|x| x.count()).collect::<Vec<_>>()
            );
        }
    }
}
