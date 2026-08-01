# 鏇存柊鏃ュ織锛圱ask 浜や粯璁板綍锛?
鏈枃浠惰褰曟瘡涓?Task 鐨勪氦浠樺唴瀹广€侀獙璇佺粨鏋溿€佹彁浜や俊鎭笌鏋勫缓鐜璇存槑銆傛瘡瀹屾垚涓€涓?Task 浜や粯鍚庡湪姝よ拷鍔犺褰曘€?
---

## Task 4锛氬疄鐜版ā鍨嬬鐞嗐€丼ilero VAD 鍜屾湰鍦?Whisper 杞啓

**鏃ユ湡锛?* 2026-08-01

### 浜や粯鍐呭

- `src-tauri/models/models.json`锛氬畼鏂规ā鍨嬫竻鍗曪紙id銆佷笅杞?URL銆丼HA-256銆佸ぇ灏忋€佽瑷€鑼冨洿銆佽繍琛屾。浣嶏級锛屽唴宓屼簬绋嬪簭
- `src-tauri/src/asr/model_manager.rs`锛氭竻鍗曟牎楠岋紙蹇呴渶瀛楁锛夈€?*鏈湴妯″瀷瀵煎叆**锛堣绠楀苟鐧昏 SHA-256銆佹寜澶у皬鍖归厤娓呭崟銆佷复鏃舵枃浠?+ 鍘熷瓙閲嶅懡鍚嶃€佸け璐ュ嵆鍒狅級銆佹湰鍦版敞鍐岃〃鎸佷箙鍖栥€乣download_with_resume` 鏂偣缁紶锛圧ange 璇锋眰 + 澶у皬鏍￠獙锛?- `src-tauri/src/vad/segmenter.rs`锛歏AD 鍒嗘鐘舵€佹満锛?0ms 甯с€?00ms 鍓嶇疆缂撳瓨銆?80ms 璇煶璧锋銆?00ms 闈欓煶鏀舵銆?5s 寮哄埗鍒囧垎锛涙涓嶅惈灏鹃儴闈欓煶锛夛紝涓庡垎绫诲櫒瑙ｈ€?- `src-tauri/src/vad/silero.rs`锛歋ilero VAD v5 ONNX 鍒嗙被鍣紙ort 杩愯锛岀淮鎶?h/c 鐘舵€侊級
- `src-tauri/src/asr/whisper_worker.rs`锛歐hisper 杞啓鍘熻锛圴ulkan 鍔犺浇澶辫触鑷姩闄嶇骇 CPU銆乣transcribe`/`transcribe_text`銆?6kHz 鍗曞０閬?i16 WAV 璇诲彇锛?- `tests/fixtures/audio/`锛歋API TTS 鐢熸垚鐨勪腑鏂?鑻辨枃闂闊抽涓庨潤闊虫祴璇曢煶棰戯紙16kHz 鍗曞０閬擄級
- 渚濊禆锛氭柊澧?`sha2`銆乣ndarray`锛坥rt 杈撳叆鏍囬噺锛?
### 楠岃瘉缁撴灉

| 妫€鏌ラ」 | 缁撴灉 |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml` | PASS锛?7 閫氳繃 + 2 蹇界暐锛?|
| asr::model_manager::tests | PASS锛氶敊璇搱甯屾嫆缁濄€佹纭搱甯岄€氳繃銆佸鍏ョ櫥璁颁笌瑙ｆ瀽銆佹寜澶у皬鍖归厤娓呭崟銆佸師瀛愭浛鎹㈠け璐ユ竻鐞嗕复鏃舵枃浠躲€佹柇鐐圭画浼狅紙鏈湴 mock HTTP 鏈嶅姟鍣級銆佸ぇ灏忎笉鍖归厤妫€娴嬨€佹敞鍐岃〃鎸佷箙鍖?|
| vad::segmenter::tests | PASS锛氶潤闊充笉浜у嚭銆佷袱娈佃闊虫纭垎寮€锛堝惈鍓嶇疆缂撳瓨銆佷笉鍚熬閮ㄩ潤闊筹級銆佺煭鍣０涓嶈捣娈点€佽秴闀胯闊?25s 鍒囧垎 |
| asr::whisper_worker::tests | PASS锛氱己澶辨ā鍨嬫姤閿欍€乄AV 璇诲彇銆乸cm鈫抐32 鏄犲皠 |
| 妯″瀷渚濊禆闆嗘垚娴嬭瘯锛坺h/en/silence fixtures 杞啓锛?| 鏍囪 `#[ignore]`锛?*闇€瑕佺敤鎴峰厛鏈湴瀵煎叆 Whisper 妯″瀷**锛涘鍏ュ悗杩愯 `cargo test -- --ignored asr::whisper_worker` 楠岃瘉 |

### 妯″瀷鏉ユ簮璇存槑锛堟寜鐢ㄦ埛瑕佹眰璋冩暣锛?
- v0.1.0 **涓嶅仛妯″瀷鑷姩涓嬭浇**锛圚uggingFace 鐨?whisper.cpp 妯″瀷浠撳簱闇€瑕佺櫥褰曪紝GitHub Release 鏃犳ā鍨嬭祫浜э級
- 鐢ㄦ埛鍦ㄨ缃?瀵煎叆鐣岄潰閫夋嫨鏈湴妯″瀷鏂囦欢 鈫?`import_model` 璁＄畻骞剁櫥璁?SHA-256锛岀粡鏍￠獙鍚庝娇鐢?- 瀹樻柟娓呭崟锛坢odels.json锛変繚鐣欎笅杞藉厓鏁版嵁涓庣粨鏋勶紝渚涘悗缁増鏈惎鐢ㄨ嚜鍔ㄤ笅杞?
### 妯″瀷楠岃瘉锛?026-08-01锛岀敤鎴峰鍏?ggml-large-v3-turbo-q5_0.bin锛?
- 瀹炴祴杞啓锛圴ulkan 鍚庣锛孯TX 3060 Ti锛夛細zh `璇蜂粙缁嶄竴涓嬩綘璐熻矗鐨勯」鐩甡銆乪n `What was the hardest problem you solved?`銆乻ilence `''`锛堢┖锛?- 宸茬敤鐪熷疄 SHA-256 `3942217...a7e2` 鍥炲～ models.json 鐨?large-v3-turbo 鏉＄洰
- **淇 1 鈥?澶氭樉鍗?Vulkan 宕╂簝**锛氭満鍣ㄥ惈铏氭嫙鏄剧ず閫傞厤鍣紝ggml-vulkan 鏋氫妇鍏ㄩ儴璁惧瀵艰嚧璁块棶杩濊锛泈orker 榛樿璁?`GGML_VK_VISIBLE_DEVICES=0`锛堢敤鎴锋樉寮忚缃椂涓嶈鐩栵級
- **淇 2 鈥?骞惰 GPU 涓婁笅鏂囧穿婧?*锛歡gml-vulkan 涓嶅厑璁稿悓杩涚▼骞惰澶氫釜 GPU 涓婁笅鏂囷紱WhisperWorker 澧炲姞鍏ㄥ眬浜掓枼閿佷覆琛屽寲
- **淇 3 鈥?闈欓煶骞昏**锛歸hisper 瀵圭函闈欓煶杈撳嚭銆孴hank you.銆嶏紱澧炲姞 RMS 闈欓煶闂ㄦ帶锛?0.005 鐩存帴璺宠繃锛? no_speech_probability 鈮?.6 娈佃繃婊?- 妯″瀷渚濊禆娴嬭瘯锛坄#[ignore]`锛夊湪瀵煎叆妯″瀷鍚庡叏閮?PASS锛歚cargo test -- --ignored asr::whisper_worker`

### 鎻愪氦淇℃伅

- 鍒嗘敮锛歚feat/task-4-local-asr`
- commit锛歚85225ba`锛堝姛鑳斤級銆乣367e1f0`锛圕HANGELOG锛夈€乣寰呰ˉ鍏卄锛堟ā鍨嬮獙璇佷慨澶嶏級
- 鐘舵€侊細宸叉帹閫侊紝寰呯‘璁ゅ悗鍚堝苟 `main`

---

## Task 3锛氬疄鐜?WASAPI 绯荤粺闊抽鍜屽彲閫夐害鍏嬮閲囬泦

**鏃ユ湡锛?* 2026-08-01

### 浜や粯鍐呭

- `src-tauri/src/audio/resample.rs`锛氫氦閿欏澹伴亾娴偣 鈫?16kHz 鍗曞０閬?i16锛堝潎鍊?+ 閽充綅闃叉孩鍑猴級銆佷竴娆℃€х嚎鎬ф彃鍊奸噸閲囨牱銆佹祦寮?`Resampler`
- `src-tauri/src/audio/level.rs`锛歊MS / 宄板€?/ RMS 鍒嗚礉璁＄畻
- `src-tauri/src/audio/wasapi.rs`锛歐ASAPI 閲囬泦锛堢Щ妞嶈嚜 onetruedutchie-windows锛孧IT锛夛細榛樿 render endpoint loopback锛堢郴缁熼煶棰戯級銆侀粯璁?capture endpoint锛堥害鍏嬮锛夈€佹牸寮忔娴嬶紙IEEE float / PCM / extensible锛夈€侀煶閲忚绠椼€佸仠姝俊鍙?- `src-tauri/src/audio/mod.rs`锛歚AudioSource`锛圫ystem/Microphone锛夈€乣AudioFrame`锛堟潵婧愭爣璁?+ 16kHz 鍗曞０閬?i16 + 閲囬泦鏃跺埢锛夈€侀噰闆嗙嚎绋嬪惎鍔ㄥ嚱鏁帮紱绯荤粺涓庨害鍏嬮鏁版嵁鍐欏叆涓嶅悓 channel锛屼笉鍋?sample-by-sample 娣峰悎
- `src-tauri/examples/loopback_probe.rs`锛氱湡瀹炶澶囨帰閽?- `src-tauri/Cargo.toml`锛歸indows crate 澧炲姞 `Win32_System_Com_StructuredStorage`銆乣Win32_System_Variant` feature锛坄IMMDevice::Activate` 闇€瑕侊級

### 楠岃瘉缁撴灉

| 妫€鏌ラ」 | 缁撴灉 |
|---|---|
| `cargo test --manifest-path src-tauri/Cargo.toml audio::resample::tests` | 鍏?FAIL锛? 椤规湭瀹炵幇锛夆啋 瀹炵幇鍚?PASS |
| `cargo test --manifest-path src-tauri/Cargo.toml audio::` | PASS锛?1/11锛?|
| `cargo build --example loopback_probe` | PASS |
| 鐪熷疄璁惧鎺㈤拡锛堟挱鏀?1kHz 娴嬭瘯闊筹級 | `LOOPBACK_OK frames=200 rms=0.0123 peak=0.0513`锛堥潪闆?RMS锛夈€乣MIC_OK frames=291`銆乣PROBE_PASS` |
| 楹﹀厠椋庝笉鍙敤鏃?| 鎺㈤拡杈撳嚭 `MIC_UNAVAILABLE` 涓旂郴缁熼煶棰戜笉鍙楀奖鍝嶏紙浠ｇ爜璺緞宸查獙璇侊級 |

### 鎻愪氦淇℃伅

- 鍒嗘敮锛歚feat/task-3-audio-capture`
- commit锛歚3617634 feat: capture separate system and microphone audio`
- 鐘舵€侊細宸叉帹閫侊紝寰呯‘璁ゅ悗鍚堝苟 `main` 骞舵墦 `v0.1.0-m1` 閲岀▼纰?tag

### 璇存槑

- 鎺㈤拡杩愯鏈熼棿闇€瑕佹挱鏀炬祴璇曢煶棰戯紙濡?1kHz 姝ｅ鸡锛夐獙璇侀潪闆?RMS
- 閲囬泦杈撳嚭濂戠害锛歚AudioFrame { source, samples_16khz_mono: Vec<i16>, captured_at_ms }`

---

## Task 2锛氬缓绔嬮鍩熺被鍨嬨€乀auri 鍛戒护鍜屽弻绐楀彛 UI 楠ㄦ灦

**鏃ユ湡锛?* 2026-08-01

### 浜や粯鍐呭

- `src/types/domain.ts`锛氬墠鍚庣棰嗗煙濂戠害锛坄Speaker`銆乣PipelineState`銆乣SessionState`銆乣CaptureSource`銆乣TranscriptSegment`銆乣DetectedQuestion`銆乣AnswerDraft`锛?- `src/lib/events.ts`锛氱ǔ瀹氫簨浠跺绾︼紙capture-state銆乤udio-level銆乼ranscript-pending/final銆乹uestion-detected銆乤nswer-started/delta/completed 鍙婅浇鑽风被鍨嬶級
- `src/lib/tauri.ts`锛歍auri invoke/浜嬩欢鐩戝惉灏佽锛堥潪 Tauri 鐜瀹夊叏闄嶇骇锛?- `src/features/meeting/OverlayPage.tsx` + `OverlayPage.test.tsx`锛氱疆椤朵細璁潰鏉匡紙鏍囬鏍忔寔缁樉绀?AI 涓庨噰闆嗙姸鎬侊級
- `src/components/CaptureIndicator.tsx`锛氶噰闆嗙姸鎬佹寚绀猴紙绯荤粺/楹﹀厠椋?鍙岃矾/鏈噰闆嗭級
- `src-tauri/src/state.rs`锛氫細璇濈姸鎬佹満 `SessionState`锛圛dle鈫扴tarting鈫扖apturing鈫扴topping鈫扞dle锛孎ailed 鍙洖 Idle锛? 7 椤瑰崟鍏冩祴璇?- `src-tauri/src/commands.rs`锛歚start_session` / `stop_session` / `session_state` 鍛戒护
- `src-tauri/src/lib.rs`锛氭敞鍏?`SessionManager` 骞舵敞鍐屽懡浠?- `src-tauri/tauri.conf.json`锛氭柊澧?`overlay` 绐楀彛锛堝缁堢疆椤躲€佸彲缂╂斁銆佹渶灏忓搴?360px銆侀粯璁や笉閫忔槑搴?1.0 鈮?70%锛?- `src-tauri/icons/`锛歵auri-build 鎵€闇€鐨勭獥鍙ｅ浘鏍囷紙鍗犱綅鍥炬爣锛孴ask 11 缁嗗寲锛?
### 楠岃瘉缁撴灉

| 妫€鏌ラ」 | 缁撴灉 |
|---|---|
| `npm test -- --run src/features/meeting/OverlayPage.test.tsx` | PASS锛堝厛 red 鍚?green锛?|
| `cargo test --manifest-path src-tauri/Cargo.toml state::tests` | PASS锛?/7锛?|
| `npm test -- --run`锛堝叏閲忓墠绔級 | PASS |
| `npx tsc --noEmit` | PASS |
| `npm run build` | PASS |
| `cargo test`锛堝畬鏁翠緷璧栨爲锛?| PASS |

### 鎻愪氦淇℃伅

- 鍒嗘敮锛歚feat/task-2-ui-skeleton`
- commit锛歚668863b feat: add visible meeting overlay and session state`
- 鐘舵€侊細宸叉帹閫侊紝寰呯‘璁ゅ悗鍚堝苟 `main`

### 鏋勫缓鐜璇存槑锛堥噸瑕侊級

- 棣栨瀹屾暣缂栬瘧渚濊禆鏍戯紙tauri + whisper.cpp Vulkan + ONNX Runtime锛夛紝鍏?566 涓?crate
- **260 瀛楃璺緞闄愬埗**锛氶」鐩矾寰勬繁宓屽瀵艰嚧 MSVC 鏃犳硶鍐欏叆涓棿鏂囦欢锛坈l.exe 涓嶆敮鎸侀暱璺緞锛屽嵆浣垮紑鍚郴缁?LongPathsEnabled锛夈€傝В鍐虫柟妗堬細cargo 鏋勫缓鐩綍杩佺Щ鑷崇煭璺緞 `G:\t`锛堢敤鎴风幆澧冨彉閲?`CARGO_TARGET_DIR=G:\t`锛宻etx 鎸佷箙鍖栵級
- 缂栬瘧鎵€闇€鐜鍙橀噺锛歚VULKAN_SDK`锛?.4.350.0锛夈€乣LIBCLANG_PATH=C:\Program Files\LLVM\bin`锛坆indgen 闇€瑕侊級銆乣CMAKE_GENERATOR=Ninja`锛坵hisper.cpp 鏋勫缓锛岄伩鍏?MSBuild 闀胯矾寰勯棶棰橈級
- 蹇呴』鍦?VS 寮€鍙戣€呯幆澧冿紙`vcvars64.bat`锛変笅鎵ц cargo 鍛戒护
- tauri 2.11.5 宸茬Щ闄ょ獥鍙?opacity API锛岀獥鍙ｄ笉閫忔槑搴︿负榛樿 1.0锛屾弧瓒炽€屼笉寰椾綆浜?70%銆?
---

## Task 1锛氬垵濮嬪寲宸ョ▼銆佹祴璇曟鏋朵笌绗笁鏂圭櫥璁?
**鏃ユ湡锛?* 2026-08-01

### 浜や粯鍐呭

- `.gitignore`锛氭帓闄?API Key銆乣.env`銆丼QLite銆佸綍闊?杞啓銆佹ā鍨嬫枃浠躲€乣target/`銆乣node_modules/`銆佸畨瑁呭寘杈撳嚭
- `package.json`锛歊eact 19.2.8銆乂ite 8.2.0銆乂itest 4.1.10銆乀ypeScript 7.0.2銆丂tauri-apps/api 2.11.1銆丂tauri-apps/cli 2.11.4
- `vite.config.ts`銆乣tsconfig.json`銆乣index.html`銆乣src/main.tsx`銆乣src/test/setup.ts`
- `src-tauri/`锛欳argo.toml锛?3 涓洿鎺ヤ緷璧栵紝鍚?whisper-rs 0.16.0[vulkan]銆乷rt 2.0.0-rc.13銆乲eyring 4.1.6 绛夛級銆乼auri.conf.json銆乧apabilities/default.json銆佹渶灏忓彲缂栬瘧 lib.rs/main.rs銆丆argo.lock锛?66 鍖咃級
- `THIRD_PARTY_NOTICES.md`锛?6 椤圭涓夋柟鐧昏锛圲RL/璁稿彲璇?鍥哄畾鐗堟湰/澶嶇敤鏉垮潡/淇敼璇存槑锛夛紝鍚弬鑰冨疄鐜?commit `f3dca22`
- `scripts/verify-third-party.ps1`锛氭牎楠岀櫥璁拌〃瀛楁瀹屾暣鎬?+ 浜ゅ弶鏍￠獙 package.json/Cargo.toml 鍏ㄩ儴鐩存帴渚濊禆

### 楠岃瘉缁撴灉

| 妫€鏌ラ」 | 缁撴灉 |
|---|---|
| `npm ls --depth=0` | 鏃?missing 渚濊禆 |
| `cargo metadata --manifest-path src-tauri/Cargo.toml --no-deps` | PASS锛坋xit 0锛?|
| `npm run build` | PASS锛坋xit 0锛?|
| `npx tsc --noEmit` | PASS |
| `powershell -ExecutionPolicy Bypass -File scripts/verify-third-party.ps1` | 杈撳嚭 `Third-party manifest OK`锛?6 椤癸級 |

### 鎻愪氦淇℃伅

- 鍒嗘敮锛歚main`
- commit锛歚5dfbeec chore: initialize tauri meeting assistant`锛圱ask 1 鍦ㄥ垎鏀瓥鐣ョ‘瀹氬墠瀹屾垚锛屼繚鐣欏湪 main锛?
### 鐜瀹夎璁板綍

- Node.js 24.18.1锛堝畼鏂瑰畨瑁咃紱opencode 鑷甫 npm 鎹熷潖锛屾棤娉曟墽琛屼换浣曞懡浠わ級
- Rust 1.97.1 MSVC锛坮ustup锛?- Vulkan SDK 1.4.350.0锛坵hisper-rs vulkan feature 缂栬瘧鏈熼渶瑕佸ご鏂囦欢锛?- GitHub CLI 2.97.0锛涜繙绋嬩粨搴?https://github.com/1suTik/Interview-Assistant---Deepseek.git锛堢鏈夛紝榛樿鍒嗘敮 main锛?
### Git 绛栫暐鍙樻洿

- 2026-08-01 璧凤細姣忎釜 Task 鍦ㄧ嫭绔嬪姛鑳藉垎鏀?`feat/task-N-鎻忚堪` 寮€鍙戯紝娴嬭瘯閫氳繃鍚?push 鍒嗘敮锛岀‘璁ゅ悗鍚堝苟 `main`锛堣瑙?PROJECT_PLAN.md 3.1锛?
