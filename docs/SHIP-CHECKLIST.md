# Shipping HOLDFAST to itch.io

The final checklist. Ordered so that anything which could stop the launch comes
before anything which merely improves it.

Status as of the end of the twelve-round playtest programme: 74 commits, 467
tests passing, `cargo clippy --all-targets -- -D warnings` clean, and
`holdfast-web.zip` building at 8.8 MB.

---

## 1. Blockers — do not publish without these

- [ ] **Run the game in a browser.** `bash tools/build-web.sh --serve`, open
      `localhost:8080`, click, and play two minutes.
      **Nobody has ever done this.** The wasm builds, the bundle serves, the
      binary starts with the right magic bytes — none of which proves it runs.
      Check specifically: input reaches the canvas, audio starts after the
      click, the save survives a reload (it uses `localStorage`), and the frame
      rate is playable.
- [ ] **Play one full run to a death, in the browser.** The desktop build is
      well tested; the web build has never had a single run played on it.
- [ ] **Check it at the embed size you will actually use.** 960×600 is the itch
      default. The UI scales to the window now, but that was verified on desktop.

## 2. Store page — needed for the page to exist

Copy is drafted in `docs/ITCH-PAGE.md`; assets are in `dist/press/`.

- [ ] Create the project. Kind: **HTML**. Upload `holdfast-web.zip`.
- [ ] Tick **"This file will be played in the browser"**.
- [ ] Viewport **1280 × 720**, fullscreen button on.
- [ ] Leave **Mobile friendly off** — keyboard only, and it means it.
- [ ] Leave **"Automatically start on page load" off** — audio needs the click.
- [ ] Paste the title, tagline and description from `docs/ITCH-PAGE.md`.
      Edit the prose; it is drafted to be edited, not to be final.
- [ ] Upload `dist/press/cover.png` (630 × 500) as the cover.
- [ ] Upload the three screenshots from `dist/press/`.
- [ ] **Put the control list on the page.** A keyboard-only game with no mouse
      fallback surprises people, and the surprise arrives as a bad first
      impression rather than a question.
- [ ] Genre Strategy; tags as listed in `docs/ITCH-PAGE.md`.

## 3. Known defects a player can hit

None of these stop a launch. All of them are things a review might mention, so
decide deliberately rather than by omission.

- [ ] **A long run may vanish with no death and no error.** Seen at level 154,
      and twice more in agent runs. Live enemies reach ~400 against a cap of 320,
      so `Director::alive` still under-counts and something accumulates. The
      bridge reports a `population` block to diagnose the next occurrence. This
      is the most likely thing a dedicated player will hit.
- [ ] **Spawners cannot be destroyed in practice.** They became damageable at
      all only in the last day; three separate attempts got within metres and
      failed, because weapons take the nearest target and a fort's garrison is
      always closer. The design says clearing nests is "the only way to reduce
      ambient pressure", so a player who reads that will try and fail.
- [ ] **Four of five worlds have unmeasured contrast and lighting.** Only the
      desk was retuned — floor darkened and ambient dropped so the lamps read.
      Forest, rooftop and arcane run ambient 230–250 and their monsters were
      measured at 1.06:1 to 1.29:1 against their own ground, where 3:1 is the
      minimum for a graphic that means anything. Grid at 150 is closest to right.
- [ ] **The fog boundary terraces.** `veil()` returns three discrete alphas and
      each quad takes one uniform value, so the veil steps rather than graduates.
      More visible now the desk floor is darker.
- [ ] **The player's toolkit stops growing at 230 seconds** while the wave budget
      grows without bound. Players will feel this as "it gets samey and then kills
      me". The largest open design question and deliberately not patched — see
      `docs/PLAYBOOK.md`.
- [ ] **44 dead-code items** remain in `docs/DEAD-CODE-AUDIT.md`, several of
      which are advertised features that do nothing — the Highlighter's mastery
      sets a burn that nothing reads, and ally and structure blurbs are never
      shown, so the player is never told what a Shocker or a Generator does.

## 4. Before the *next* build, not this one

- [ ] Adopt the audit's structural fix: make everything `pub(crate)` except
      `run()` and the two FFI entry points, move `tests/simulation.rs` into
      `threat.rs`, and run `cargo clippy -- -D warnings` **without**
      `--all-targets`. In a lib crate `pub` exempts an item from `dead_code` and
      `--all-targets` counts a test's use as a use, which is why eleven of this
      session's defects were invisible to a clean lint.
- [ ] The remaining UX items in `docs/UX-CRITIQUE.md`, in its order.

## 5. Not in scope for an itch.io launch

- Touch controls. iOS and Android compile but have no wrapper projects and no
  touch input design. Phones are a later release.
- Controller support. Does not exist.
- The native LLM tactician bridges. `holdfast_set_model_bridge` is written and
  tested; the Swift and Kotlin sides that would call it are not. Ollama works
  for desktop development and is optional everywhere.

---

## Rebuilding

    bash tools/build-web.sh          # ~9 minutes, produces dist/ and the zip
    bash tools/build-web.sh --serve  # ...then serves it on :8080

Nothing else ships. Every mesh, material, sound and glyph is generated at
runtime, so the zip is the whole game.
