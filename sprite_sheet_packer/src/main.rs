use sprite_sheet_util;
use sprite_sheet_util::{pack_audio_sprite_sheet, Sound};

fn main() {
    let digital_ui = Sound {
        source: "freesound.org/564474__gerainsan__digital-ui-buttons-errors-switches-sounds-gs.wav",
        author: Some("gerainsan"),
        url: Some("https://freesound.org/people/gerainsan/sounds/564474/"),
        volume: 0.0,
        pitch: 0.0,
        ..Default::default()
    };

    let sounds = vec![
        Sound {
            name: "success",
            start: 5.662,
            end: 10.19,
            ..digital_ui
        },
        Sound {
            name: "pain",
            start: 10.443,
            end: 13.639,
            ..digital_ui
        },
        Sound {
            name: "loss",
            start: 13.969,
            end: 16.769,
            ..digital_ui
        },
        Sound {
            name: "event",
            start: 40.720,
            end: 41.2,
            ..digital_ui
        },
        Sound {
            name: "ping",
            start: 51.4,
            end: 52.0,
            ..digital_ui
        },
        Sound {
            name: "music",
            source: "timbeek.com/music.wav",
            author: Some("Tim Beek"),
            music: true,
            looping: true,
            loop_start: Some(30.719),
            end: 168.959,
            volume: -1.0,
            ..Default::default()
        },
    ];

    pack_audio_sprite_sheet(
        sounds,
        1,
        44100,
        "../assets/audio",
        "../client/audio",
        "../client/src/audio",
        "../assets/audio/README",
    );
}
