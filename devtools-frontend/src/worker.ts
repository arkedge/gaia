import { GrpcWebFetchTransport } from "@protobuf-ts/grpcweb-transport";
import {
  PostCommandRequest,
  PostCommandResponse,
  TelemetryStreamResponse,
} from "./proto/broker";
import { BrokerClient } from "./proto/broker.client";
import { TmtcGenericC2aClient } from "./proto/tmtc_generic_c2a.client";
import { GetSateliteSchemaResponse } from "./proto/tmtc_generic_c2a";
import { Tmiv } from "./proto/tco_tmiv";

export default null;
// eslint-disable-next-line no-var
declare var self: SharedWorkerGlobalScope;

export type GrpcClientService = {
  getSatelliteSchema(): Promise<GetSateliteSchemaResponse>;
  postCommand(input: PostCommandRequest): Promise<PostCommandResponse>;
  openTelemetryStream(tmivName: string): Promise<ReadableStream<Tmiv>>;
  lastTelemetryValue(tmivName: string): Promise<Tmiv | undefined>;
  setRootRecordDirectory(directory: FileSystemDirectoryHandle): Promise<void>;
  hasRecordDirectory(): Promise<boolean>;
  enableRecording(telemetryName: string): Promise<void>;
  disableRecording(telemetryName: string): Promise<void>;
  openRecordingStatusStream(): Promise<ReadableStream<RecordingStatus>>;
};

export type WorkerRpcService = {
  [proc: string]: (...args: any) => Promise<any>;
};

type Values<T> = T[keyof T];

export type WorkerRequest<S extends WorkerRpcService> = Values<{
  [Proc in keyof S]: {
    callback: MessagePort;
    proc: Proc;
    args: Parameters<S[Proc]>;
  };
}>;
export type WorkerResponse<S extends WorkerRpcService> = {
  [Proc in keyof S]:
    | {
        value: Awaited<ReturnType<S[Proc]>>;
      }
    | {
        error: string;
      };
};

const transport = new GrpcWebFetchTransport({
  baseUrl: self.name,
});
const brokerClient = new BrokerClient(transport);
const tmtcGenericC2a = new TmtcGenericC2aClient(transport);

const telemetryLastValues = new Map<string, Tmiv>();
const telemetryBus = new EventTarget();

const startTelemetryStream = async () => {
  const { responses } = brokerClient.openTelemetryStream({});
  for await (const { tmiv } of responses) {
    if (typeof tmiv === "undefined") {
      continue;
    }
    telemetryLastValues.set(tmiv.name, tmiv);
    telemetryBus.dispatchEvent(new CustomEvent(tmiv.name, { detail: tmiv }));
  }
};

export type RecordingStatus = {
  directoryIsSet: boolean;
  recordingTelemetries: Set<string>;
};
type RecordingStatusListener = (status: RecordingStatus) => void;
let rootRecordDirectory: FileSystemDirectoryHandle | undefined;
const recorders: Map<string, TelemetryRecorder> = new Map();
let nextRecordingStatusListenerId = 0;
const recordStatusListeners: Map<number, RecordingStatusListener> = new Map();

const currentRecordingStatus = (): RecordingStatus => {
  return {
    directoryIsSet: rootRecordDirectory !== undefined,
    recordingTelemetries: new Set(recorders.keys()),
  };
};

const addRecordingStatusListener = (
  listener: RecordingStatusListener,
): number => {
  listener(currentRecordingStatus());
  const id = nextRecordingStatusListenerId++;
  recordStatusListeners.set(id, listener);
  return id;
};

const removeRecordingStatusListener = (id: number): void => {
  recordStatusListeners.delete(id);
};

const notifyRecordingStatusListener = (): void => {
  const status = currentRecordingStatus();
  for (const listener of recordStatusListeners.values()) {
    listener(status);
  }
};

// TODO: flush on worker termination
class TelemetryRecorder {
  telemetryName: string;
  cancel: () => void = () => {};
  cancelled: Promise<null>;
  onStop: () => void;
  constructor(
    rootRecordDirectory: FileSystemDirectoryHandle,
    telemetryName: string,
    onStop: () => void,
  ) {
    this.telemetryName = telemetryName;
    this.cancelled = new Promise((resolve) => {
      this.cancel = () => resolve(null);
    });
    this.onStop = onStop;
    this.run(rootRecordDirectory);
  }

  private async write(
    recordDirectory: FileSystemDirectoryHandle,
    writable: FileSystemWritableFileStream,
    tmiv: Tmiv,
  ) {
    // if tmiv has @blob field, save it to a file
    let blobFileName = "";
    const blob = tmiv.fields.find((field) => {
      return field.name === "@blob";
    });
    if (blob !== undefined) {
      if (blob.value.oneofKind == "bytes") {
        const blobBytes = blob.value.bytes;
        //FIXME: safer name consrtuction
        const blobDirectory = await recordDirectory.getDirectoryHandle(
          "blob_data",
          {
            create: true,
          },
        );

        // FIXME: readable time format?
        // FIXME: avoid name collision
        blobFileName = `${Date.now()}.dat`;
        const recordFile = await blobDirectory.getFileHandle(blobFileName, {
          create: true,
        });
        const blobWritable = await recordFile.createWritable();
        await blobWritable.write(blobBytes);
        await blobWritable.close();
      }
    }

    const fields: { [key: string]: any } = {};
    for (const field of tmiv.fields) {
      if (field.name.includes("@RAW")) {
        continue;
      }
      const name = field.name;
      let value = undefined;
      if (field.name == "@blob") {
        value = blobFileName;
      } else if (field.value.oneofKind == "integer") {
        value = Number(field.value.integer);
      } else if (field.value.oneofKind == "string") {
        value = field.value.string;
      } else if (field.value.oneofKind == "double") {
        value = field.value.double;
      } else if (field.value.oneofKind == "enum") {
        value = field.value.enum;
      } else if (field.value.oneofKind == "bytes") {
        value = field.value.bytes;
      }
      fields[name] = value;
    }
    await writable.write(JSON.stringify(fields));
    await writable.write("\n");
  }

  private async run(rootRecordDirectory: FileSystemDirectoryHandle) {
    // FIXME: safer name construction
    const recordDirectoryName = this.telemetryName;
    const recordDirectory = await rootRecordDirectory.getDirectoryHandle(
      recordDirectoryName,
      { create: true },
    );
    // FIXME: readable time format?
    const recordFile = await recordDirectory.getFileHandle(
      `${Date.now()}.log`,
      {
        create: true,
      },
    );

    const telemetryStream = await server.openTelemetryStream(
      this.telemetryName,
    );
    const reader = telemetryStream.getReader();
    let writable = await recordFile.createWritable();
    let lastFlushTime = Date.now();

    // eslint-disable-next-line no-constant-condition
    while (true) {
      const next = await Promise.race([reader.read(), this.cancelled]);
      if (next === null) {
        // cancelled
        break;
      }
      if (next.done) {
        break;
      }
      const tmiv = next.value;

      await this.write(recordDirectory, writable, tmiv);
      if (Date.now() - lastFlushTime > 10000) {
        lastFlushTime = Date.now();
        await writable.close();
        writable = await recordFile.createWritable({ keepExistingData: true });
        const size = (await recordFile.getFile()).size;
        await writable.seek(size);
      }
    }

    await writable.close();
    this.onStop();
  }

  stop() {
    // do not call onStop here
    this.cancel();
  }
}

const server = {
  async getSatelliteSchema(): Promise<GetSateliteSchemaResponse> {
    const { response } = await tmtcGenericC2a.getSatelliteSchema({});
    return response;
  },
  async postCommand(input: PostCommandRequest): Promise<PostCommandResponse> {
    const { response } = brokerClient.postCommand(input);
    return response;
  },
  async openTelemetryStream(tmivName: string): Promise<ReadableStream<Tmiv>> {
    let handler: any;
    return new ReadableStream({
      start(controller) {
        handler = (e: CustomEvent<Tmiv>) => {
          controller.enqueue(e.detail);
        };
        telemetryBus.addEventListener(tmivName, handler as any);
        const lastValue = telemetryLastValues.get(tmivName);
        if (typeof lastValue !== "undefined") {
          controller.enqueue(lastValue);
        }
      },
      cancel() {
        telemetryBus.removeEventListener(tmivName, handler as any);
      },
    });
  },

  async lastTelemetryValue(tmivName: string): Promise<Tmiv | undefined> {
    return telemetryLastValues.get(tmivName);
  },

  async setRootRecordDirectory(
    directory: FileSystemDirectoryHandle,
  ): Promise<void> {
    if (rootRecordDirectory === undefined) {
      rootRecordDirectory = directory;
    }
  },

  async hasRecordDirectory(): Promise<boolean> {
    return rootRecordDirectory !== undefined;
  },

  async enableRecording(telemetryName: string): Promise<void> {
    if (rootRecordDirectory === undefined) {
      return;
    }
    if (recorders.has(telemetryName)) {
      return;
    }

    const recorder = new TelemetryRecorder(
      rootRecordDirectory,
      telemetryName,
      () => {
        recorders.delete(telemetryName);
        notifyRecordingStatusListener();
      },
    );
    recorders.set(telemetryName, recorder);
    notifyRecordingStatusListener();
  },

  async disableRecording(telemetryName: string): Promise<void> {
    const recorder = recorders.get(telemetryName);
    if (recorder === undefined) {
      return;
    }
    recorder.stop();
  },

  async openRecordingStatusStream(): Promise<ReadableStream<RecordingStatus>> {
    let id: number | undefined;
    return new ReadableStream({
      start(controller) {
        id = addRecordingStatusListener((status) => controller.enqueue(status));
      },
      cancel() {
        removeRecordingStatusListener(id!);
      },
    });
  },
};

self.addEventListener("connect", (e) => {
  for (const port of e.ports) {
    port.addEventListener(
      "message",
      (e: MessageEvent<WorkerRequest<GrpcClientService>>) => {
        // eslint-disable-next-line prefer-spread
        const promise = (server[e.data.proc] as any).apply(
          server,
          e.data.args,
        ) as Promise<any>;
        const resolve = (value: any) => {
          if (value instanceof ReadableStream) {
            e.data.callback.postMessage(
              {
                value,
              },
              [value],
            );
          } else {
            e.data.callback.postMessage({
              value,
            });
          }
        };
        const reject = (error: any) => {
          e.data.callback.postMessage({
            error,
          });
        };
        promise.then(resolve, reject);
      },
    );
    port.start();
  }
});
(async () => {
  // eslint-disable-next-line no-constant-condition
  while (true) {
    try {
      await startTelemetryStream();
    } catch (e) {
      console.error(e);
    }
    await new Promise((resolve) => setTimeout(resolve, 1000));
  }
})();
