# Space Invaders acceptance checks (Issue #12)

- The classic formation contains two alien rows; the lower green octopus row from the sprite sheet is not rendered. The separate green player cannon remains at the bottom.
- No green bunker/shield blocks are rendered.
- Enemy projectiles are selected only from living aliens and begin at the bottom pixel of the selected alien sprite.
- Marching formations step downward on each edge reversal and reset to the top before reaching the player's cannon. An emptied formation also restarts.
- Host tests verify the removed sprites, remaining player cannon, shot origin, pre-render laser collision, and descent/reset behavior. This is simulator/unit evidence only; no ESP32-S3 hardware gameplay test was performed.
