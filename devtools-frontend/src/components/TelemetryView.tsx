import React, { useCallback, useEffect, useMemo, useState } from "react";
import { TreeNamespace, addToNamespace, mapNamespace } from "../tree";

import { Tmiv, TmivField } from "../proto/tco_tmiv";
import { useClient } from "./Layout";
import { useParams } from "react-router-dom";
import { Helmet } from "react-helmet-async";
import { TelemetrySchema } from "../proto/tmtc_generic_c2a";
import type { RecordingStatus } from "../worker";

const buildTelemetryFieldTreeBlueprintFromSchema = (
  tlm: TelemetrySchema,
): TreeNamespace<undefined> => {
  const fieldNames = tlm.fields.map((f) => f.name);
  const root: TreeNamespace<undefined> = new Map();
  for (const fieldName of fieldNames) {
    const path = fieldName.split(".");
    addToNamespace(root, path, undefined);
  }
  return root;
};

type TelemetryValuePair = {
  converted: TmivField["value"] | null;
  raw: TmivField["value"] | null;
};

const buildTelemetryFieldTree = (
  blueprint: TreeNamespace<undefined>,
  fields: TmivField[],
): TreeNamespace<TelemetryValuePair> => {
  const convertedFieldMap = new Map<string, TmivField["value"]>();
  const rawFieldMap = new Map<string, TmivField["value"]>();
  for (const field of fields) {
    if (field.name.endsWith("@RAW")) {
      const strippedName = field.name.slice(0, -4);
      rawFieldMap.set(strippedName, field.value);
    } else {
      convertedFieldMap.set(field.name, field.value);
    }
  }
  return mapNamespace(blueprint, (path, _key) => {
    const key = path.join(".");
    const converted = convertedFieldMap.get(key) ?? null;
    const raw = rawFieldMap.get(key) ?? null;
    return { converted, raw };
  });
};

const prettyprintValue = (value: TmivField["value"] | null) => {
  if (value === null) {
    return "****";
  }
  switch (value.oneofKind) {
    case "integer":
      return `${value.integer}`;
    case "bytes":
      return [...value.bytes]
        .map((x) => x.toString(16).padStart(2, "0"))
        .join("");
    case "enum":
      return value.enum;
    case "double":
      return value.double.toFixed(3);
    case "string":
      return value.string;
  }
};

type ValueCellProps = {
  name: string;
  value: TelemetryValuePair;
};
const LeafCell: React.FC<ValueCellProps> = ({ name, value }) => {
  return (
    <div className="px-0.5 flex flex-row justify-between highlight-domain">
      <span className="text-slate-300">{name}</span>
      <span className="min-w-[2ch]" />
      <span className="font-bold text-right">
        {prettyprintValue(value.converted)}
      </span>
    </div>
  );
};

type NamespaceCellProps = {
  name: string;
  ns: TreeNamespace<TelemetryValuePair>;
};
const NamespaceCell: React.FC<NamespaceCellProps> = ({ name, ns }) => {
  const [isOpen, setIsOpen] = useState(true);
  const handleClickHeading = useCallback(() => {
    setIsOpen(!isOpen);
  }, [isOpen]);
  return (
    <div className="flex flex-col highlight-domain">
      <div className="px-0.5 flex flex-row">
        <button
          className="flex flex-row w-full text-left outline-none"
          onClick={handleClickHeading}
        >
          <span className="text-orange-500">{name}</span>
          {!isOpen && (
            <>
              <span className="opacity-70 flex-1 truncate">
                <InlineNamespaceContentCell ns={ns} />
              </span>
              <span className="text-orange-500 font-bold">+</span>
            </>
          )}
        </button>
      </div>
      <div className={`ml-[2ch] ${!isOpen ? "hidden" : ""}`}>
        <NamespaceContentCell ns={ns} />
      </div>
    </div>
  );
};

type NamespaceContentCellProps = {
  ns: TreeNamespace<TelemetryValuePair>;
};
const NamespaceContentCell: React.FC<NamespaceContentCellProps> = ({ ns }) => {
  return (
    <div className="flex flex-col">
      {[...ns.entries()].map(([name, v]) => {
        switch (v.type) {
          case "leaf":
            return <LeafCell key={name} name={name} value={v.value} />;
          case "ns":
            return <NamespaceCell key={name} name={name} ns={v.ns} />;
        }
      })}
    </div>
  );
};

type InlineNamespaceContentCellProps = {
  ns: TreeNamespace<TelemetryValuePair>;
};
const InlineNamespaceContentCell: React.FC<InlineNamespaceContentCellProps> = ({
  ns,
}) => {
  return (
    <>
      {[...ns.entries()].map(([name, v]) => {
        switch (v.type) {
          case "leaf":
            return (
              <span className="ml-[0.5ch]" key={name}>
                <span className="text-slate-300">{name}:</span>
                <span className="font-bold">
                  {prettyprintValue(v.value.converted)}
                </span>
              </span>
            );
          case "ns":
            return null;
        }
      })}
    </>
  );
};

export const TelemetryView: React.FC = () => {
  const { client } = useClient();
  const [recorderStatus, setRecordingStatus] = useState<RecordingStatus | null>(
    null,
  );
  const params = useParams();
  const tmivName = params["tmivName"]!;
  useEffect(() => {
    const readerP = client
      .openRecordingStatusStream()
      .then((stream) => stream.getReader());
    let cancel;
    const cancelP = new Promise((resolve) => (cancel = resolve));
    Promise.all([readerP, cancelP]).then(([reader]) => reader.cancel());
    readerP.then(async (reader) => {
      // eslint-disable-next-line no-constant-condition
      while (true) {
        const next = await reader.read();
        if (next.done) {
          break;
        }
        setRecordingStatus(next.value);
      }
    });
    return cancel;
  }, [client]);

  const toggleRecordingStatus = async () => {
    if (!recorderStatus?.directoryIsSet) {
      const directoryHandle = await window.showDirectoryPicker({
        mode: "readwrite",
      });
      client.setRootRecordDirectory(directoryHandle);
    }
    if (recorderStatus?.recordingTelemetries.has(tmivName)) {
      client.disableRecording(tmivName);
    } else {
      client.enableRecording(tmivName);
    }
  };



    const recording =
    recorderStatus?.recordingTelemetries?.has(tmivName) ?? false;

  const recordingMenuItemsWhenRecording = (
    <>
      <li>Recording: ON</li>
      <li onClick={toggleRecordingStatus}>Stop Recording</li>
    </>
  );
  const recordingMenuItemsWhenNotRecording = (
    <>
      <li>Recording: OFF</li>
      <li onClick={toggleRecordingStatus}>Start Recording</li>
    </>
  );
  return (
    <>
      <nav>
        {recording
          ? recordingMenuItemsWhenRecording
          : recordingMenuItemsWhenNotRecording}
      </nav>
      <TelemetryViewBody />
    </>
  );
};

const TelemetryViewBody: React.FC = () => {
  const params = useParams();
  const tmivName = params["tmivName"]!;
  const {
    client,
    satelliteSchema: { telemetryComponents },
  } = useClient();
  const [tmiv, setTmiv] = useState<Tmiv | null>(null);
  useEffect(() => {
    setTmiv(null);
    const readerP = client
      .openTelemetryStream(tmivName)
      .then((stream) => stream.getReader());
    let cancel;
    const cancelP = new Promise((resolve) => (cancel = resolve));
    Promise.all([readerP, cancelP]).then(([reader]) => reader.cancel());
    readerP.then(async (reader) => {
      // eslint-disable-next-line no-constant-condition
      while (true) {
        const next = await reader.read();
        if (next.done) {
          break;
        }
        const tmiv = next.value;
        setTmiv(tmiv);
      }
    });
    return cancel;
  }, [client, tmivName]);
  const telemetryDef = useMemo(() => {
    const [_channel, componentName, telemetryName] = tmivName.split(".");
    const [_c, componentDef] = Object.entries(telemetryComponents).find(
      ([name, _]) => name === componentName,
    )!;
    const [_t, telemetryDef] = Object.entries(componentDef.telemetries).find(
      ([name, _]) => name === telemetryName,
    )!;
    return telemetryDef;
  }, [telemetryComponents, tmivName]);
  const treeBlueprint = useMemo(() => {
    return buildTelemetryFieldTreeBlueprintFromSchema(telemetryDef!);
  }, [telemetryDef]);
  const tree = buildTelemetryFieldTree(treeBlueprint, tmiv?.fields ?? []);

  return (
    <>
      <Helmet>
        <title>{tmivName}</title>
      </Helmet>
      <div className="h-full p-2 columns-xs overflow-x-auto flex-1 font-mono leading-4 cursor-default column-fill-auto">
        <NamespaceContentCell ns={tree} />
      </div>
    </>
  );
};
