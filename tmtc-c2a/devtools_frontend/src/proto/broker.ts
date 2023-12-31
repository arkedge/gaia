// @generated by protobuf-ts 2.9.1
// @generated from protobuf file "broker.proto" (package "broker", syntax proto3)
// tslint:disable
import { ServiceType } from "@protobuf-ts/runtime-rpc";
import type { BinaryWriteOptions } from "@protobuf-ts/runtime";
import type { IBinaryWriter } from "@protobuf-ts/runtime";
import { WireType } from "@protobuf-ts/runtime";
import type { BinaryReadOptions } from "@protobuf-ts/runtime";
import type { IBinaryReader } from "@protobuf-ts/runtime";
import { UnknownFieldHandler } from "@protobuf-ts/runtime";
import type { PartialMessage } from "@protobuf-ts/runtime";
import { reflectionMergePartial } from "@protobuf-ts/runtime";
import { MESSAGE_TYPE } from "@protobuf-ts/runtime";
import { MessageType } from "@protobuf-ts/runtime";
import { Tmiv } from "./tco_tmiv";
import { Tco } from "./tco_tmiv";
/**
 * @generated from protobuf message broker.PostCommandRequest
 */
export interface PostCommandRequest {
    /**
     * @generated from protobuf field: tco_tmiv.Tco tco = 3;
     */
    tco?: Tco;
}
/**
 * TODO:
 *
 * @generated from protobuf message broker.PostCommandResponse
 */
export interface PostCommandResponse {
}
/**
 * @generated from protobuf message broker.CommandStreamRequest
 */
export interface CommandStreamRequest {
}
/**
 * @generated from protobuf message broker.CommandStreamResponse
 */
export interface CommandStreamResponse {
    /**
     * @generated from protobuf field: string tco_json = 1;
     */
    tcoJson: string;
    /**
     * @generated from protobuf field: tco_tmiv.Tco tco = 2;
     */
    tco?: Tco;
}
/**
 * @generated from protobuf message broker.PostTelemetryRequest
 */
export interface PostTelemetryRequest {
    /**
     * @generated from protobuf field: string tmiv_json = 1;
     */
    tmivJson: string;
    /**
     * @generated from protobuf field: tco_tmiv.Tmiv tmiv = 2;
     */
    tmiv?: Tmiv;
}
/**
 * TODO:
 *
 * @generated from protobuf message broker.PostTelemetryResponse
 */
export interface PostTelemetryResponse {
}
/**
 * @generated from protobuf message broker.TelemetryStreamRequest
 */
export interface TelemetryStreamRequest {
}
/**
 * @generated from protobuf message broker.TelemetryStreamResponse
 */
export interface TelemetryStreamResponse {
    /**
     * @generated from protobuf field: tco_tmiv.Tmiv tmiv = 3;
     */
    tmiv?: Tmiv;
}
/**
 * @generated from protobuf message broker.GetLastReceivedTelemetryRequest
 */
export interface GetLastReceivedTelemetryRequest {
    /**
     * @generated from protobuf field: string telemetry_name = 1;
     */
    telemetryName: string;
}
/**
 * @generated from protobuf message broker.GetLastReceivedTelemetryResponse
 */
export interface GetLastReceivedTelemetryResponse {
    /**
     * @generated from protobuf field: tco_tmiv.Tmiv tmiv = 1;
     */
    tmiv?: Tmiv;
}
// @generated message type with reflection information, may provide speed optimized methods
class PostCommandRequest$Type extends MessageType<PostCommandRequest> {
    constructor() {
        super("broker.PostCommandRequest", [
            { no: 3, name: "tco", kind: "message", T: () => Tco }
        ]);
    }
    create(value?: PartialMessage<PostCommandRequest>): PostCommandRequest {
        const message = {};
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<PostCommandRequest>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: PostCommandRequest): PostCommandRequest {
        let message = target ?? this.create(), end = reader.pos + length;
        while (reader.pos < end) {
            let [fieldNo, wireType] = reader.tag();
            switch (fieldNo) {
                case /* tco_tmiv.Tco tco */ 3:
                    message.tco = Tco.internalBinaryRead(reader, reader.uint32(), options, message.tco);
                    break;
                default:
                    let u = options.readUnknownField;
                    if (u === "throw")
                        throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
                    let d = reader.skip(wireType);
                    if (u !== false)
                        (u === true ? UnknownFieldHandler.onRead : u)(this.typeName, message, fieldNo, wireType, d);
            }
        }
        return message;
    }
    internalBinaryWrite(message: PostCommandRequest, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        /* tco_tmiv.Tco tco = 3; */
        if (message.tco)
            Tco.internalBinaryWrite(message.tco, writer.tag(3, WireType.LengthDelimited).fork(), options).join();
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.PostCommandRequest
 */
export const PostCommandRequest = new PostCommandRequest$Type();
// @generated message type with reflection information, may provide speed optimized methods
class PostCommandResponse$Type extends MessageType<PostCommandResponse> {
    constructor() {
        super("broker.PostCommandResponse", []);
    }
    create(value?: PartialMessage<PostCommandResponse>): PostCommandResponse {
        const message = {};
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<PostCommandResponse>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: PostCommandResponse): PostCommandResponse {
        return target ?? this.create();
    }
    internalBinaryWrite(message: PostCommandResponse, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.PostCommandResponse
 */
export const PostCommandResponse = new PostCommandResponse$Type();
// @generated message type with reflection information, may provide speed optimized methods
class CommandStreamRequest$Type extends MessageType<CommandStreamRequest> {
    constructor() {
        super("broker.CommandStreamRequest", []);
    }
    create(value?: PartialMessage<CommandStreamRequest>): CommandStreamRequest {
        const message = {};
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<CommandStreamRequest>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: CommandStreamRequest): CommandStreamRequest {
        return target ?? this.create();
    }
    internalBinaryWrite(message: CommandStreamRequest, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.CommandStreamRequest
 */
export const CommandStreamRequest = new CommandStreamRequest$Type();
// @generated message type with reflection information, may provide speed optimized methods
class CommandStreamResponse$Type extends MessageType<CommandStreamResponse> {
    constructor() {
        super("broker.CommandStreamResponse", [
            { no: 1, name: "tco_json", kind: "scalar", T: 9 /*ScalarType.STRING*/ },
            { no: 2, name: "tco", kind: "message", T: () => Tco }
        ]);
    }
    create(value?: PartialMessage<CommandStreamResponse>): CommandStreamResponse {
        const message = { tcoJson: "" };
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<CommandStreamResponse>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: CommandStreamResponse): CommandStreamResponse {
        let message = target ?? this.create(), end = reader.pos + length;
        while (reader.pos < end) {
            let [fieldNo, wireType] = reader.tag();
            switch (fieldNo) {
                case /* string tco_json */ 1:
                    message.tcoJson = reader.string();
                    break;
                case /* tco_tmiv.Tco tco */ 2:
                    message.tco = Tco.internalBinaryRead(reader, reader.uint32(), options, message.tco);
                    break;
                default:
                    let u = options.readUnknownField;
                    if (u === "throw")
                        throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
                    let d = reader.skip(wireType);
                    if (u !== false)
                        (u === true ? UnknownFieldHandler.onRead : u)(this.typeName, message, fieldNo, wireType, d);
            }
        }
        return message;
    }
    internalBinaryWrite(message: CommandStreamResponse, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        /* string tco_json = 1; */
        if (message.tcoJson !== "")
            writer.tag(1, WireType.LengthDelimited).string(message.tcoJson);
        /* tco_tmiv.Tco tco = 2; */
        if (message.tco)
            Tco.internalBinaryWrite(message.tco, writer.tag(2, WireType.LengthDelimited).fork(), options).join();
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.CommandStreamResponse
 */
export const CommandStreamResponse = new CommandStreamResponse$Type();
// @generated message type with reflection information, may provide speed optimized methods
class PostTelemetryRequest$Type extends MessageType<PostTelemetryRequest> {
    constructor() {
        super("broker.PostTelemetryRequest", [
            { no: 1, name: "tmiv_json", kind: "scalar", T: 9 /*ScalarType.STRING*/ },
            { no: 2, name: "tmiv", kind: "message", T: () => Tmiv }
        ]);
    }
    create(value?: PartialMessage<PostTelemetryRequest>): PostTelemetryRequest {
        const message = { tmivJson: "" };
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<PostTelemetryRequest>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: PostTelemetryRequest): PostTelemetryRequest {
        let message = target ?? this.create(), end = reader.pos + length;
        while (reader.pos < end) {
            let [fieldNo, wireType] = reader.tag();
            switch (fieldNo) {
                case /* string tmiv_json */ 1:
                    message.tmivJson = reader.string();
                    break;
                case /* tco_tmiv.Tmiv tmiv */ 2:
                    message.tmiv = Tmiv.internalBinaryRead(reader, reader.uint32(), options, message.tmiv);
                    break;
                default:
                    let u = options.readUnknownField;
                    if (u === "throw")
                        throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
                    let d = reader.skip(wireType);
                    if (u !== false)
                        (u === true ? UnknownFieldHandler.onRead : u)(this.typeName, message, fieldNo, wireType, d);
            }
        }
        return message;
    }
    internalBinaryWrite(message: PostTelemetryRequest, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        /* string tmiv_json = 1; */
        if (message.tmivJson !== "")
            writer.tag(1, WireType.LengthDelimited).string(message.tmivJson);
        /* tco_tmiv.Tmiv tmiv = 2; */
        if (message.tmiv)
            Tmiv.internalBinaryWrite(message.tmiv, writer.tag(2, WireType.LengthDelimited).fork(), options).join();
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.PostTelemetryRequest
 */
export const PostTelemetryRequest = new PostTelemetryRequest$Type();
// @generated message type with reflection information, may provide speed optimized methods
class PostTelemetryResponse$Type extends MessageType<PostTelemetryResponse> {
    constructor() {
        super("broker.PostTelemetryResponse", []);
    }
    create(value?: PartialMessage<PostTelemetryResponse>): PostTelemetryResponse {
        const message = {};
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<PostTelemetryResponse>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: PostTelemetryResponse): PostTelemetryResponse {
        return target ?? this.create();
    }
    internalBinaryWrite(message: PostTelemetryResponse, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.PostTelemetryResponse
 */
export const PostTelemetryResponse = new PostTelemetryResponse$Type();
// @generated message type with reflection information, may provide speed optimized methods
class TelemetryStreamRequest$Type extends MessageType<TelemetryStreamRequest> {
    constructor() {
        super("broker.TelemetryStreamRequest", []);
    }
    create(value?: PartialMessage<TelemetryStreamRequest>): TelemetryStreamRequest {
        const message = {};
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<TelemetryStreamRequest>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: TelemetryStreamRequest): TelemetryStreamRequest {
        return target ?? this.create();
    }
    internalBinaryWrite(message: TelemetryStreamRequest, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.TelemetryStreamRequest
 */
export const TelemetryStreamRequest = new TelemetryStreamRequest$Type();
// @generated message type with reflection information, may provide speed optimized methods
class TelemetryStreamResponse$Type extends MessageType<TelemetryStreamResponse> {
    constructor() {
        super("broker.TelemetryStreamResponse", [
            { no: 3, name: "tmiv", kind: "message", T: () => Tmiv }
        ]);
    }
    create(value?: PartialMessage<TelemetryStreamResponse>): TelemetryStreamResponse {
        const message = {};
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<TelemetryStreamResponse>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: TelemetryStreamResponse): TelemetryStreamResponse {
        let message = target ?? this.create(), end = reader.pos + length;
        while (reader.pos < end) {
            let [fieldNo, wireType] = reader.tag();
            switch (fieldNo) {
                case /* tco_tmiv.Tmiv tmiv */ 3:
                    message.tmiv = Tmiv.internalBinaryRead(reader, reader.uint32(), options, message.tmiv);
                    break;
                default:
                    let u = options.readUnknownField;
                    if (u === "throw")
                        throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
                    let d = reader.skip(wireType);
                    if (u !== false)
                        (u === true ? UnknownFieldHandler.onRead : u)(this.typeName, message, fieldNo, wireType, d);
            }
        }
        return message;
    }
    internalBinaryWrite(message: TelemetryStreamResponse, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        /* tco_tmiv.Tmiv tmiv = 3; */
        if (message.tmiv)
            Tmiv.internalBinaryWrite(message.tmiv, writer.tag(3, WireType.LengthDelimited).fork(), options).join();
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.TelemetryStreamResponse
 */
export const TelemetryStreamResponse = new TelemetryStreamResponse$Type();
// @generated message type with reflection information, may provide speed optimized methods
class GetLastReceivedTelemetryRequest$Type extends MessageType<GetLastReceivedTelemetryRequest> {
    constructor() {
        super("broker.GetLastReceivedTelemetryRequest", [
            { no: 1, name: "telemetry_name", kind: "scalar", T: 9 /*ScalarType.STRING*/ }
        ]);
    }
    create(value?: PartialMessage<GetLastReceivedTelemetryRequest>): GetLastReceivedTelemetryRequest {
        const message = { telemetryName: "" };
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<GetLastReceivedTelemetryRequest>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: GetLastReceivedTelemetryRequest): GetLastReceivedTelemetryRequest {
        let message = target ?? this.create(), end = reader.pos + length;
        while (reader.pos < end) {
            let [fieldNo, wireType] = reader.tag();
            switch (fieldNo) {
                case /* string telemetry_name */ 1:
                    message.telemetryName = reader.string();
                    break;
                default:
                    let u = options.readUnknownField;
                    if (u === "throw")
                        throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
                    let d = reader.skip(wireType);
                    if (u !== false)
                        (u === true ? UnknownFieldHandler.onRead : u)(this.typeName, message, fieldNo, wireType, d);
            }
        }
        return message;
    }
    internalBinaryWrite(message: GetLastReceivedTelemetryRequest, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        /* string telemetry_name = 1; */
        if (message.telemetryName !== "")
            writer.tag(1, WireType.LengthDelimited).string(message.telemetryName);
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.GetLastReceivedTelemetryRequest
 */
export const GetLastReceivedTelemetryRequest = new GetLastReceivedTelemetryRequest$Type();
// @generated message type with reflection information, may provide speed optimized methods
class GetLastReceivedTelemetryResponse$Type extends MessageType<GetLastReceivedTelemetryResponse> {
    constructor() {
        super("broker.GetLastReceivedTelemetryResponse", [
            { no: 1, name: "tmiv", kind: "message", T: () => Tmiv }
        ]);
    }
    create(value?: PartialMessage<GetLastReceivedTelemetryResponse>): GetLastReceivedTelemetryResponse {
        const message = {};
        globalThis.Object.defineProperty(message, MESSAGE_TYPE, { enumerable: false, value: this });
        if (value !== undefined)
            reflectionMergePartial<GetLastReceivedTelemetryResponse>(this, message, value);
        return message;
    }
    internalBinaryRead(reader: IBinaryReader, length: number, options: BinaryReadOptions, target?: GetLastReceivedTelemetryResponse): GetLastReceivedTelemetryResponse {
        let message = target ?? this.create(), end = reader.pos + length;
        while (reader.pos < end) {
            let [fieldNo, wireType] = reader.tag();
            switch (fieldNo) {
                case /* tco_tmiv.Tmiv tmiv */ 1:
                    message.tmiv = Tmiv.internalBinaryRead(reader, reader.uint32(), options, message.tmiv);
                    break;
                default:
                    let u = options.readUnknownField;
                    if (u === "throw")
                        throw new globalThis.Error(`Unknown field ${fieldNo} (wire type ${wireType}) for ${this.typeName}`);
                    let d = reader.skip(wireType);
                    if (u !== false)
                        (u === true ? UnknownFieldHandler.onRead : u)(this.typeName, message, fieldNo, wireType, d);
            }
        }
        return message;
    }
    internalBinaryWrite(message: GetLastReceivedTelemetryResponse, writer: IBinaryWriter, options: BinaryWriteOptions): IBinaryWriter {
        /* tco_tmiv.Tmiv tmiv = 1; */
        if (message.tmiv)
            Tmiv.internalBinaryWrite(message.tmiv, writer.tag(1, WireType.LengthDelimited).fork(), options).join();
        let u = options.writeUnknownFields;
        if (u !== false)
            (u == true ? UnknownFieldHandler.onWrite : u)(this.typeName, message, writer);
        return writer;
    }
}
/**
 * @generated MessageType for protobuf message broker.GetLastReceivedTelemetryResponse
 */
export const GetLastReceivedTelemetryResponse = new GetLastReceivedTelemetryResponse$Type();
/**
 * @generated ServiceType for protobuf service broker.Broker
 */
export const Broker = new ServiceType("broker.Broker", [
    { name: "PostCommand", options: {}, I: PostCommandRequest, O: PostCommandResponse },
    { name: "OpenTelemetryStream", serverStreaming: true, options: {}, I: TelemetryStreamRequest, O: TelemetryStreamResponse },
    { name: "GetLastReceivedTelemetry", options: {}, I: GetLastReceivedTelemetryRequest, O: GetLastReceivedTelemetryResponse },
    { name: "OpenCommandStream", serverStreaming: true, clientStreaming: true, options: {}, I: CommandStreamRequest, O: CommandStreamResponse },
    { name: "PostTelemetry", options: {}, I: PostTelemetryRequest, O: PostTelemetryResponse }
]);
