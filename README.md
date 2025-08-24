# envy

`envy` is a render-backend agnostic 2D layouting library, primarily intended for use with game UI.

Other libraries, such as `egui`, `iced`, `gpui`, and more are much more well-fitted for traditional UI/UX development, but game UI is a little bit different.

Game UI is typically designed less around portability and more around a static experience that is fit for a certain aspect ratio (typically 16:9).

This library is currently a WIP, and is not as optimal as it could be (well that depends a little bit on the backend implementation). The assumptions and restrictions
around the way nodes are implemented is because `envy` was originally designed as a layouting library to run on the Nintendo Switch by hijacking Super Smash Bros. Ultimate's
rendering engine and the access to certain shaders there is limited.
