# Gumball

Welcome to the Gumball project! This emulator is designed to faithfully replicate the functionality of the original Nintendo Game Boy. It is written in Rust and utilizes the SDL2 library for graphics and audio. This is a work in progress. There are a couple of known graphical bugs, and audio support is unfinished. But it works well enough to play Tetris!

## Features

- **CPU Emulation**: Accurate emulation of the Game Boy's Z80-like CPU.
- **PPU Emulation**: Renders graphics as per the original Game Boy's specifications.
- **APU Emulation**: Sound generation and audio playback.
- **Input Handling**: Supports keyboard input for emulating the Game Boy's buttons.

## Prerequisites

- **Rust**: Ensure you have Rust installed. You can install Rust using [rustup](https://rustup.rs/).
- **SDL2**: SDL2 library is required. You can install it using your system's package manager or from [SDL's official site](https://www.libsdl.org/download-2.0.php).
- **SDL2 Mixer**: For audio playback. Installation instructions can be found [here](https://www.libsdl.org/projects/SDL_mixer/).

## Installation

1. **Clone the Repository**:
   ```sh
   git clone https://github.com/yourusername/gumball.git
   cd gumball
   ```

2. **Build the Project**:
   ```sh
   cargo build --release
   ```

## Usage

To run the emulator, you need to provide a ROM file as a command-line argument.

```sh
cargo run --release -- -r path/to/your/game.rom
```

## Controls

- **Up**: `Up`
- **Down**: `Down`
- **Left**: `Left`
- **Right**: `Right`
- **A Button**: `Z`
- **B Button**: `X`
- **Start**: `Enter`
- **Select**: `Right Shift`

## Development

### Adding Features

If you want to contribute or add new features, follow these steps:

1. **Fork the Repository**:
   Click on the `Fork` button at the top right of this page.

2. **Create a New Branch**:
   ```sh
   git checkout -b feature-name
   ```

3. **Make Your Changes**:
   Make sure to write tests if applicable.

4. **Commit and Push**:
   ```sh
   git add .
   git commit -m "Description of your feature"
   git push origin feature-name
   ```

5. **Create a Pull Request**:
   Go to your fork on GitHub and open a pull request.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Acknowledgements

- [Rust](https://www.rust-lang.org/)
- [SDL2](https://www.libsdl.org/)
- [Blargg's Game Boy test ROMs](http://gbdev.gg8.se/files/roms/blargg-gb-tests/)
- [Mooneye GB Tests](https://github.com/Gekkio/mooneye-gb)
