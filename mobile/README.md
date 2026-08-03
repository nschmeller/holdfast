# HOLDFAST on phones

The game is one library. `src/lib.rs` exposes `run()`, and each platform has a
thin entry point beside it:

| Platform | Symbol | Built by |
| --- | --- | --- |
| Desktop / web | `main()` in `src/main.rs` | `cargo build`, `tools/build-web.sh` |
| iOS | `holdfast_main()` | `tools/build-ios.sh` |
| Android | `android_main()` | `tools/build-android.sh` |

Nothing platform-specific lives in the game itself, which is the point: the
same app definition runs everywhere, and the wrappers below are the only
per-platform code there is.

## State of play

- **iOS builds and links.** `tools/build-ios.sh` produces
  `mobile/HoldfastCore.xcframework`. It has not been run on a device or in the
  simulator - that needs an Xcode project and a signing identity, neither of
  which exists here yet.
- **Android is unverified.** The entry point and the build script are written,
  but the NDK is not installed on this machine, so the toolchain has never
  been exercised. Expect the first run to surface something.

Both need the same thing before they are actually playable, and it is not a
build problem - see *Controls*.

## iOS

```sh
tools/build-ios.sh
```

Then, once:

1. New Xcode project, **App**, SwiftUI or Storyboard - it does not matter, the
   view hierarchy gets replaced.
2. Drag `mobile/HoldfastCore.xcframework` into *Frameworks, Libraries and
   Embedded Content*. Set it to **Do Not Embed** - it is a static library.
3. Add these system frameworks: `Metal`, `MetalKit`, `QuartzCore`,
   `AudioToolbox`, `AVFoundation`, `UIKit`, `GameController`.
4. Delete the generated `AppDelegate`/`main` and replace with a C `main.m`:

   ```objc
   extern void holdfast_main(void);
   int main(int argc, char *argv[]) { holdfast_main(); return 0; }
   ```

5. In *Info.plist*: `UIRequiresFullScreen = YES`, and restrict
   `UISupportedInterfaceOrientations` to landscape - the overlook camera is
   framed for it.

Deployment target 14.0 or later; `wgpu` needs Metal.

## Android

```sh
export ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/<version>
cargo install cargo-ndk
tools/build-android.sh
```

Wrap it with a `GameActivity` project. `AndroidManifest.xml` needs:

```xml
<application android:hasCode="false">
  <activity
      android:name="com.google.androidgamesdk.GameActivity"
      android:exported="true"
      android:screenOrientation="landscape"
      android:configChanges="orientation|keyboardHidden|screenSize">
    <meta-data android:name="android.app.lib_name" android:value="holdfast" />
    <intent-filter>
      <action android:name="android.intent.action.MAIN" />
      <category android:name="android.intent.category.LAUNCHER" />
    </intent-filter>
  </activity>
</application>
```

Build the crate with `--features bevy/android-game-activity` to match.

## Controls: the actual blocker

HOLDFAST is keyboard-only by design, and that design does not survive contact
with a touchscreen. A phone build that compiles is not a phone build that
plays.

The input layer is already shaped for this. `player::Intent` is a plain
`{ move_dir, dash }` component written by `read_move_input` and consumed by
movement, specifically so a second input source can write it without touching
the simulation. What a touch build needs:

- a virtual stick writing `Intent::move_dir`,
- on-screen buttons for the four verbs that matter - plan, dash, call wave,
  threat - rather than all fourteen keys,
- plan mode driven by tapping the ground instead of nudging a cursor with the
  arrow keys, which is the one interaction that does not translate at all.

That is a design job, not a port, and it is deliberately not done here. Getting
the build green first means the design work can be checked on a device instead
of guessed at.

## The device's own model

`src/tactician.rs` will let a language model retune the enemy AI while you
play. On a desktop it finds Ollama or LM Studio over a socket. On a phone the
model lives behind a Swift or Kotlin framework that Rust cannot call, so the
wrapper makes the call and hands the text back through one function pointer:

```c
void holdfast_set_model_bridge(
    char *(*ask)(const char *prompt),
    void  (*free_reply)(char *reply));
```

Call it once, before `holdfast_main` or `android_main`. Rust owns the prompt
and the parsing; the platform owns the model. Return null when unavailable and
the game quietly falls back to its own director - which is also what happens on
every device that has no such model, so the fallback is the well-tested path.

### iOS 26 and later

```swift
import FoundationModels

@_cdecl("holdfast_ask_model")
func holdfastAskModel(_ prompt: UnsafePointer<CChar>) -> UnsafeMutablePointer<CChar>? {
    guard SystemLanguageModel.default.isAvailable else { return nil }
    let text = String(cString: prompt)
    // The bridge is synchronous and is called from a background thread, so
    // blocking here is correct; do not hop to the main actor.
    let reply = /* await session.respond(to: text) */ ""
    return strdup(reply)
}

@_cdecl("holdfast_free_reply")
func holdfastFreeReply(_ p: UnsafeMutablePointer<CChar>) { free(p) }
```

Then in `main.m`, before `holdfast_main()`:

```objc
extern void holdfast_set_model_bridge(char *(*)(const char *), void (*)(char *));
extern char *holdfast_ask_model(const char *);
extern void holdfast_free_reply(char *);

holdfast_set_model_bridge(holdfast_ask_model, holdfast_free_reply);
```

On anything older than iOS 26, or any device where `isAvailable` is false,
return null and nothing else changes.

### Android

Same shape through Gemini Nano. The JNI side calls
`GenerativeModel.generateContent` from AICore, `strdup`s the result, and
registers the pair before `android_main`. Devices without AICore return null.

Neither of these is written yet - the wrapper projects do not exist. The Rust
half is done and tested, and `HOLDFAST_LLM` reaching Ollama is how it is
exercised during development.
