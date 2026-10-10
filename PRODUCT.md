# Product

## Register

product

## Users

Multi-monitor Windows power users who calibrate their displays: gamers tuning vibrance and gamma per title, designers and photographers keeping panels consistent, and display enthusiasts running mixed monitor setups. They live in the NVIDIA Control Panel today and want the same engine without its friction. Context: at their desk, mid-workflow, often mid-game. Primary task on any given screen: pick a preset, apply or pin it to the right monitor, get back to what they were doing.

## Product Purpose

ChromaDeck is a per-monitor color profile manager for Windows. It saves color presets (brightness, contrast, gamma, RGB gains, digital vibrance, hue, ICC profiles) and applies, pins, or enforces them per monitor from a single deck-style UI, restoring them automatically when games, HDR toggles, or driver updates stomp them. Success looks like: the user stops opening the NVIDIA Control Panel entirely, and stopped thinking about ChromaDeck at all, because enforcement just holds.

## First run

Fresh installs start with an empty library: no seeded preset, nothing pinned or applied. A preset only exists because the user created it, so highlighting always means a real user action. Upgrades run a one-shot purge of pristine seeded Standard installs; user-tweaked presets are never touched.

## Brand Personality

Precise, technical, confident. A calibration instrument, not a toy and not a marketing page. The interface should feel like it was built by people who reimplemented NVIDIA's transfer math from observed driver behavior: exact numbers, neutral points stated, no hand-waving. Emotion on first successful apply: quiet satisfaction, the tool did exactly what it said.

## Anti-references

What this should NOT look like:

- Generic AI-SaaS dark dashboard: purple/blue gradient wash, glass cards, glowing blobs, eyebrow-kicker section scaffolding, ghost-card borders paired with wide soft shadows.
- Gamer RGB kitsch: rainbow chrome, aggressive neon, esports angular clichés, animated rainbow accents. This is a precision instrument that gamers happen to use, not gaming merch.
- Flat gray enterprise admin: washed-out muted text on tinted surfaces, status communicated by hue alone with no shape or label backup.

## Design Principles

1. **The tool trusts the eye.** This is a color product; the interface practices calibration-grade exactness, numbers with stated neutrals, no decorative color that competes with the user's presets.
2. **Status is shape first, hue second.** Connected, pinned, warning, and offline states carry a distinct shape and label, so meaning survives color blindness and dim rooms.
3. **Density without clutter.** Power users want every control within reach, but every visible element must earn its place; transient feedback appears, confirms, and leaves.
4. **Enforcement is silent, actions are decisive.** Background work never interrupts; foreground actions acknowledge input within ~100ms and settle fast, like a well-built instrument panel.
5. **Contrast is a working condition, not a theme.** The UI lives next to fullscreen games and color-critical work; it stays high-contrast, matte, and out of the way — no glow, no translucency, nothing competing with the display being judged.

## Accessibility & Inclusion

Deferred by explicit decision. Palette work proceeds on identity first; a dedicated audit pass later covers WCAG AA contrast, focus visibility, and non-hue status encoding (note: status already carries shape + label, which is a head start).
