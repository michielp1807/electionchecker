mod data;

pub use data::*;
use std::iter;

pub fn allocate_single_step<Quality: Ord>(
    votes: &[Votes],
    seats: &mut [Seats],
    available_seats: &mut Seats,
    criterion: impl Fn(Votes, Seats) -> Option<Quality>,
) -> Option<()> {
    let qualities = iter::zip(votes, seats.iter())
        .map(|(votes, seats)| {
            if seats.has_candidates() {
                criterion(*votes, *seats)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();

    let max_quality = qualities.iter().max().unwrap().as_ref()?;

    let awarded = iter::zip(qualities.iter(), seats)
        .filter_map(|(quality, seat)| (quality.as_ref() == Some(max_quality)).then_some(seat))
        .collect::<Vec<_>>();

    for seat in ballotted(awarded, available_seats.count()) {
        seat.transfer(available_seats);
    }

    Some(())
}

pub fn absolute_majority_check(votes: &[Votes], seats: &mut [Seats], prev_seats: Vec<Seats>) {
    let total_votes = votes.iter().map(|Votes(count)| count).sum::<Count>();
    let total_seats = seats.iter().map(|count| count.count()).sum::<Count>();

    let mut correction = Seats::filled(1);

    let absolute_majority = |count, total| 2 * count > total;

    if let Some((_, winner_seat)) =
        iter::zip(votes, seats.iter_mut()).find(|(Votes(cur_vote), cur_seat)| {
            cur_seat.has_candidates()
                && absolute_majority(*cur_vote, total_votes)
                && !absolute_majority(cur_seat.count(), total_seats)
        })
    {
        #[cfg(feature = "chatty")]
        eprintln!("an absolute majority correction was performed");
        winner_seat.transfer(&mut correction);
        let winner_seat = *winner_seat;

        let last_winners = iter::zip(seats.iter_mut(), prev_seats)
            .filter_map(|(x, y)| (*x > y && *x != winner_seat).then_some(x))
            .collect::<Vec<_>>();

        let loser_seat = ballotted(last_winners, 1).next().unwrap();
        correction.transfer(loser_seat);
    }
}

#[cfg(feature = "chatty")]
pub fn whole_seats_available(votes: &[Votes], seats: &[Seats], seats_awarded: Seats) -> bool {
    let total_seats = seats_awarded.count() + seats.iter().map(|x| x.count()).sum::<Count>();
    let total_votes = votes.iter().map(|Votes(x)| x).sum::<Count>();
    iter::zip(votes, seats).any(|(Votes(cur_vote), cur_seat)| {
        frac(*cur_vote, cur_seat.count() + 1) >= frac(total_votes, total_seats)
    })
}

#[cfg(feature = "chatty")]
fn debug_results<'a>(seats: impl Iterator<Item = &'a Seats>) {
    for seat in seats {
        eprint!("{seat}, ");
    }
    eprintln!();
}

pub fn allocate_seats<Quality: Ord>(
    votes: &[Votes],
    seats: &mut [Seats],
    available_seats: &mut Seats,
    method: impl Fn(Votes, Seats) -> Option<Quality> + Copy,
) {
    let mut last_winners = seats.to_owned();
    #[cfg(feature = "chatty")]
    let mut printed = false;
    while available_seats.count() > 0 {
        #[cfg(feature = "chatty")]
        if !(whole_seats_available(votes, seats, *available_seats) || printed) {
            printed = true;
            eprintln!("rest seats");
        }

        last_winners.copy_from_slice(seats);

        if allocate_single_step(votes, seats, available_seats, method).is_none() {
            return;
        }

        #[cfg(feature = "chatty")]
        debug_results(seats.iter());
    }

    absolute_majority_check(votes, seats, last_winners);
}

pub fn allocate_per_average(mut total_seats: Seats, votes: Vec<Votes>, seats: &mut [Seats]) {
    allocate_seats(
        &votes,
        seats,
        &mut total_seats,
        |Votes(cur_vote), cur_seat| Some(frac(cur_vote, cur_seat.count() + 1)),
    );
}

pub fn allocate_per_surplus(mut total_seats: Seats, votes: Vec<Votes>, seats: &mut [Seats]) {
    let vote_count = votes.iter().map(|Votes(count)| count).sum::<Count>();
    let seat_count = total_seats.count();

    let has_surplus =
        |cur_vote, cur_seat| frac(cur_vote, 1) >= frac(cur_seat * vote_count, seat_count);

    allocate_seats(
        &votes,
        seats,
        &mut total_seats,
        move |Votes(cur_vote), cur_seat| {
            let cur_seat = cur_seat.count();
            (has_surplus(cur_vote, cur_seat)
                && frac(cur_vote, 1) >= frac(3 * vote_count, 4 * seat_count))
            .then(|| cur_vote * seat_count - cur_seat * vote_count)
        },
    );

    if total_seats.count() > 0 {
        #[cfg(feature = "chatty")]
        eprintln!("continuing by averages");
        allocate_seats(
            &votes,
            seats,
            &mut total_seats,
            |Votes(cur_vote), cur_seat| {
                let cur_seat = cur_seat.count();
                if frac(cur_vote, 1) >= frac(3 * vote_count, 4 * seat_count) {
                    has_surplus(cur_vote, cur_seat - 1)
                } else {
                    has_surplus(cur_vote, cur_seat)
                }
                .then_some(frac(cur_vote, cur_seat + 1))
            },
        );
    }
}

pub fn allocate(total_seats: Seats, votes: Vec<Votes>, seats: &mut [Seats]) {
    if total_seats.count() >= 19 {
        allocate_per_average(total_seats, votes, seats);
    } else {
        allocate_per_surplus(total_seats, votes, seats);
    }
}

pub fn allocate_national(mut total_seats: Seats, votes: Vec<Votes>, seats: &mut [Seats]) {
    let vote_count = votes.iter().map(|Votes(count)| count).sum::<Count>();
    let seat_count = total_seats.count();

    allocate_seats(
        &votes,
        seats,
        &mut total_seats,
        |Votes(cur_vote), cur_seat| {
            (frac(cur_vote, 1) >= frac(vote_count, seat_count))
                .then_some(frac(cur_vote, cur_seat.count() + 1))
        },
    );
}
