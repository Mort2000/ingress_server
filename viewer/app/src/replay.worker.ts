/// <reference lib="webworker" />
import init, { parse_replay } from "./wasm/replay_web";
self.onmessage = async (event: MessageEvent<ArrayBuffer>) => {
  try {
    await init();
    const replay = JSON.parse(parse_replay(new Uint8Array(event.data)));
    self.postMessage({ replay });
  } catch (error) {
    self.postMessage({ error: String(error) });
  }
};
