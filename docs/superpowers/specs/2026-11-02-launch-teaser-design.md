# ChromaDeck launch teaser — approved design

## Intent
Produce a 30-second portrait SaaS launch teaser and an editable offline Remotion project following the user's detailed timeline and the repository DESIGN.md. End-card URL: https://chromadeck.github.io/.

## Approved approach
Create a self-contained launch-teaser/ project inside this repository. Do not alter the application, its dependencies, or existing uncommitted changes. Integrating Remotion into the application was rejected as unnecessary coupling. User approved this approach and the assumptions below on 2026-11-02.

## Visual and motion authority
Read all of ../../DESIGN.md before implementation. The user's stricter video rules override exceptions: no shadows even on the drag ghost; no gradients, glow, blur, glass, or light-mode surfaces. Use the exact four dark elevations, bone foreground, mint accent, hairline borders, Geist and JetBrains Mono offline packages, 8px scaled card geometry, flat nested card architecture, and documented state treatments. Only transforms and opacity animate; use specified cubic-bezier curves. Centralize copy, assets, colors, layout, motion and timing in src/config.ts.

## Deliverables
launch-teaser/out/chromadeck-launch.mp4 (1080x1920, 30 fps, 900 frames, H.264 CRF 16), out/captions.srt, complete TypeScript Remotion source, README, batch stills and final contact sheet. Generate local placeholder audio if feasible.

## Assumptions approved
- Missing src/assets/chromadeck-brand.svg: draw seven rounded C tiles in code, one mint tile.
- Keep requested hook 3 and v1.0 launch chip with confirmation TODOs; actual application package version is 0.8.2.
- Enlarge metadata typography to at least 48px effective in the portrait render, rather than literally preserving 9–11px UI labels at 2.4–3x.
- Preserve desktop-style components, using crops rather than a full application window.

## Timeline
0–2 hook 1; 2–3.8 hook 2; 3.8–5.4 hook 3 and markers; 5.4–8 Meet ghost echoes; 8–10 logo assembly/ring/wordmark; 10–12 presets value; 12–14 drag/drop value; 14–17 carousel; 17–20.5 drag/drop/apply; 20.5–23 reliability/footer; 23–25 Identify; 25–27 closing 1; 27–28.2 closing 2; 28.2–29.2 mint flash/logo; 29.2–30 final wordmark/URL. Exact copy and interaction details are in the user brief and must be implemented without additional claims.

## Verification
Node v24.2.0, FFmpeg 8.0, ffprobe 8.0 already verified. Render and inspect representative stills after each of three scene batches; inspect contact sheet from final MP4; verify final dimensions, fps, duration, codec with ffprobe. Run project typecheck/tests and existing application build/tests. Any unavailable checks must be reported honestly. No commits without user request.
