import React, { useCallback, useRef, useEffect, useMemo } from "react";
import Editor, { Monaco } from "@monaco-editor/react";
import * as monaco from "monaco-editor";
import {
  CommandComponentSchema,
  CommandParameterDataType,
  CommandPrefixSchema,
  TelemetryComponentSchema,
} from "../proto/tmtc_generic_c2a";
import { Tco, TcoParam, TmivField } from "../proto/tco_tmiv";
import { useClient } from "./Layout";
import { GrpcClientService } from "../worker";
import initOpslang, * as opslang from "@crate/wasm-opslang/pkg";

type ParameterValue =
  | { type: "bytes"; bytes: Uint8Array; bigint: bigint }
  | { type: "double"; double: number }
  | { type: "integer"; integer: number };

type CommandLine = {
  command: {
    prefix: string;
    receiverComponent: string;
    executorComponent: string | undefined;
    command: string;
  };
  parameters: ParameterValue[];
};

const buildTco = (
  commandPrefixes: { [key: string]: CommandPrefixSchema },
  commandComponents: { [key: string]: CommandComponentSchema },
  commandLine: CommandLine,
): Tco => {
  let componentName = commandLine.command.receiverComponent;
  let commandPrefixName = commandLine.command.prefix;

  // FIXME: アドホックな変換
  // Gaia側の改修が必要?
  if (commandLine.command.executorComponent !== undefined) {
    if (commandLine.command.receiverComponent !== "MOBC") {
      throw new Error(
        `Executor component is only allowed when receiverComponent is MOBC`,
      );
    }

    const executionType =
      commandPrefixes[commandLine.command.prefix]?.subsystems["MOBC"]?.metadata
        ?.executionType;
    if (executionType === undefined) {
      throw new Error(`Couldn't find executionType`);
    }
    const convertedCommandPrefix = Object.entries(commandPrefixes).find(
      ([_, prefix]) => {
        const schema =
          prefix.subsystems[commandLine.command.executorComponent!];
        if (schema === undefined) {
          return false;
        }

        // destinationType must be "TO_ME"
        if (schema.metadata?.destinationType !== 0) {
          return false;
        }

        // must have the same executionType in MOBC
        if (schema.metadata?.executionType !== executionType) {
          return false;
        }

        return true;
      },
    );

    if (convertedCommandPrefix === undefined) {
      throw new Error(`Couldn't find matching command prefix`);
    }

    componentName = commandLine.command.executorComponent;
    commandPrefixName = convertedCommandPrefix[0];
  }
  if (!Object.hasOwn(commandPrefixes, commandPrefixName)) {
    throw new Error(`no such command prefix: ${commandPrefixName}`);
  }
  const commandPrefix = commandPrefixes[commandPrefixName];
  if (!Object.hasOwn(commandPrefix.subsystems, componentName)) {
    throw new Error(`prefix is not defined for component: ${componentName}`);
  }
  const commandSubsystem = commandPrefix.subsystems[componentName];
  if (!Object.hasOwn(commandComponents, componentName)) {
    throw new Error(`no such component: ${componentName}`);
  }
  const componentSchema = commandComponents[componentName];
  if (!Object.hasOwn(componentSchema.commands, commandLine.command.command)) {
    throw new Error(
      `no such command in ${componentName}: ${commandLine.command.command}`,
    );
  }
  const commandSchema = componentSchema.commands[commandLine.command.command];
  const extraParams = commandSubsystem.hasTimeIndicator ? 1 : 0;
  if (
    commandLine.parameters.length !==
    commandSchema.parameters.length + extraParams
  ) {
    throw new Error(
      `the number of parameters is wrong: expected ${commandSchema.parameters.length}, but got ${commandLine.parameters.length}`,
    );
  }
  const tcoParams: TcoParam[] = [];
  if (commandSubsystem.hasTimeIndicator) {
    const parameter = commandLine.parameters.pop()!;
    switch (parameter.type) {
      case "integer":
        tcoParams.push({
          name: "time_indicator",
          value: {
            oneofKind: "integer",
            integer: BigInt(parameter.integer),
          },
        });
        break;
      case "bytes":
        tcoParams.push({
          name: "time_indicator",
          value: {
            oneofKind: "integer",
            integer: parameter.bigint,
          },
        });
        break;
      default:
        throw new Error(`time indicator must be an integer`);
    }
  }
  for (let i = 0; i < commandSchema.parameters.length; i++) {
    const parameterSchema = commandSchema.parameters[i];
    const parameter = commandLine.parameters[i];
    const name = `param${i + 1}`;
    switch (parameterSchema.dataType) {
      case CommandParameterDataType.CMD_PARAMETER_BYTES:
        switch (parameter.type) {
          case "bytes":
            tcoParams.push({
              name,
              value: {
                oneofKind: "bytes",
                bytes: parameter.bytes,
              },
            });
            break;
          default:
            throw new Error(`value of ${name} must be bytes`);
        }
        break;
      case CommandParameterDataType.CMD_PARAMETER_INTEGER:
        switch (parameter.type) {
          case "integer":
            tcoParams.push({
              name,
              value: {
                oneofKind: "integer",
                integer: BigInt(parameter.integer),
              },
            });
            break;
          case "bytes":
            tcoParams.push({
              name,
              value: {
                oneofKind: "integer",
                integer: parameter.bigint,
              },
            });
            break;
          default:
            throw new Error(`value of ${name} must be an integer`);
        }
        break;
      case CommandParameterDataType.CMD_PARAMETER_DOUBLE:
        switch (parameter.type) {
          case "double":
            tcoParams.push({
              name,
              value: {
                oneofKind: "double",
                double: parameter.double,
              },
            });
            break;
          case "integer":
            tcoParams.push({
              name,
              value: {
                oneofKind: "double",
                double: parameter.integer,
              },
            });
            break;
          case "bytes":
            tcoParams.push({
              name,
              value: {
                oneofKind: "double",
                // FIXME: check overflow
                double: Number(parameter.bigint),
              },
            });
            break;
        }
        break;
    }
  }
  const name = `${commandPrefixName}.${componentName}.${commandLine.command.command}`;
  return {
    name,
    params: tcoParams,
  };
};

class Driver implements opslang.Driver {
  commandPrefixes: { [key: string]: CommandPrefixSchema };
  commandComponents: { [key: string]: CommandComponentSchema };
  telemetryComponents: { [key: string]: TelemetryComponentSchema };
  client: GrpcClientService;
  localVariables: Map<string, opslang.Value> = new Map();
  tlmVariables: Map<string, opslang.Value> = new Map();
  params: Map<string, opslang.Value> = new Map();
  datetimeOrigin: Map<string, bigint> = new Map();

  constructor(
    commandPrefixes: { [key: string]: CommandPrefixSchema },
    commandComponents: { [key: string]: CommandComponentSchema },
    telemetryComponents: { [key: string]: TelemetryComponentSchema },
    client: GrpcClientService,
  ) {
    this.commandPrefixes = commandPrefixes;
    this.commandComponents = commandComponents;
    this.telemetryComponents = telemetryComponents;
    this.client = client;
  }
  setDatetimeOrigin(component: string, origin: bigint) {
    this.datetimeOrigin.set(component, origin);
  }
  async sendCommand(
    prefix: string,
    receiverComponent: string,
    executorComponent: string | undefined,
    timeIndicator: opslang.Value | undefined,
    command: string,
    params: opslang.Value[],
  ): Promise<void> {
    const tiParam = [];
    if (timeIndicator !== undefined) {
      tiParam.push(timeIndicator);
    }
    const fullParams = params.concat(tiParam);
    const commandLine: CommandLine = {
      command: {
        prefix,
        receiverComponent,
        executorComponent,
        command,
      },
      parameters: fullParams.map((arg): ParameterValue => {
        if (arg.kind === "integer") {
          return {
            type: "integer",
            integer: Number(arg.value),
          };
        } else if (arg.kind === "double") {
          return {
            type: "double",
            double: arg.value,
          };
        } else if (arg.kind === "datetime") {
          const datetimeOrigin = this.datetimeOrigin.get(receiverComponent);
          if (datetimeOrigin === undefined) {
            throw new Error(
              `datetime origin is not set for ${receiverComponent}`,
            );
          }
          const millis_since_origin = arg.value - datetimeOrigin;
          const ti = Number(millis_since_origin / BigInt(100));
          return {
            type: "integer",
            integer: ti,
          };
        } else {
          throw new Error(`cannot convert ${arg.kind}`);
        }
      }),
    };

    const tco = buildTco(
      this.commandPrefixes,
      this.commandComponents,
      commandLine,
    );
    await this.client.postCommand({
      tco,
    });
  }

  resolveVariable(name: string): opslang.Value | undefined {
    return this.localVariables.get(name);
  }

  resolveTelemetryVariable(name: string): opslang.Value | undefined {
    return this.tlmVariables.get(name);
  }

  conv(field: TmivField): opslang.Value | undefined {
    const value = field.value;
    switch (value.oneofKind) {
      case "string":
        return { kind: "string", value: value.string };
      case "bytes":
        return undefined;
      case "double":
        return { kind: "double", value: value.double };
      case "integer":
        return { kind: "integer", value: value.integer };
      case "enum":
        return { kind: "string", value: value.enum };
    }
  }

  setLocalVariable(ident: string, value: opslang.Value) {
    this.localVariables.set(ident, value);
  }

  getTelemetryId(name: string): bigint | undefined {
    const re = new RegExp("([^.]*)\\.([^.]*)");
    const matches = re.exec(name);
    if (matches === null) {
      return undefined;
    }
    const componentName = matches[1] as string;
    const telemetryName = matches[2] as string;
    const component = this.telemetryComponents[componentName];
    const telemetry = component?.telemetries[telemetryName];
    const id = telemetry?.metadata?.id;
    if (id === undefined) {
      return undefined;
    } else {
      return BigInt(id);
    }
  }

  async print(value: opslang.Value): Promise<void> {
    const toStr = (value: opslang.Value): string => {
      if (value.kind === "integer") {
        return String(value.value);
      } else if (value.kind === "array") {
        return `[${value.value.map(toStr).join(", ")}]`;
      } else if (value.kind === "duration") {
        return `${value.value}ms`;
      } else if (value.kind === "datetime") {
        const d = new Date(Number(value.value));
        return d.toISOString();
      }
      return JSON.stringify(value.value);
    };
    // TODO: better output
    console.log(`print: ${toStr(value)}`);
  }

  async prepareVariables(
    _variables: string[],
    tlmVariables: string[],
  ): Promise<void> {
    this.tlmVariables.clear();

    for (const name of tlmVariables) {
      //とりあえずnameは $RT.MOBC.HK.PH.VER 形式と仮定

      const re = new RegExp("([^.]*\\.[^.]*\\.[^.]*)\\.(.*)");
      const matches = re.exec(name);
      if (matches === null) {
        continue;
      }
      const tlmName = matches[1];
      const fieldName = matches[2];
      const tmiv = await this.client.lastTelemetryValue(tlmName);
      const field = tmiv?.fields?.find((f) => f.name == fieldName);
      if (field === undefined) {
        continue;
      }
      const value = this.conv(field);
      if (value === undefined) {
        continue;
      }
      this.tlmVariables.set(name, value);
    }
  }
}

export const CommandView: React.FC = () => {
  useEffect(() => {
    (async () => {
      // init opslang
      await initOpslang();
      opslang.set_panic_hook();
    })();
  }, []);
  const {
    client,
    satelliteSchema: {
      commandPrefixes,
      commandComponents,
      telemetryComponents,
    },
  } = useClient();
  const editorRef = useRef<monaco.editor.IStandaloneCodeEditor | null>(null);
  const driver = useMemo(
    () =>
      new Driver(
        commandPrefixes,
        commandComponents,
        telemetryComponents,
        client,
      ),
    [commandPrefixes, commandComponents, telemetryComponents, client],
  );

  const validate = useCallback(
    (monaco: Monaco, model: monaco.editor.ITextModel) => {
      const markers: monaco.editor.IMarkerData[] = [];
      for (let lineno = 1; lineno <= model.getLineCount(); lineno++) {
        try {
          // TODO: rewrite validation
          // line-wise validation may not be possible
          // const line = model.getLineContent(lineno);
          // opslang.validateLine(line, lineno);
        } catch (e) {
          const lineLength = model.getLineLength(lineno);
          markers.push({
            message: `${e}`,
            severity: monaco.MarkerSeverity.Error,
            startLineNumber: lineno,
            startColumn: 1,
            endLineNumber: lineno,
            endColumn: lineLength + 1,
          });
        }
      }
      monaco.editor.setModelMarkers(model, "owner", markers);
      return markers;
    },
    [commandComponents, commandPrefixes],
  );

  const handleEditorDidMount = useCallback(
    (editor: monaco.editor.IStandaloneCodeEditor, monacoInstance: Monaco) => {
      editorRef.current = editor;

      const defaultValue = localStorage.getItem("c2a-devtools-ops-v2");
      if (defaultValue !== null) {
        editor.setValue(defaultValue);
      }

      editor.addCommand(monaco.KeyCode.Escape, () => {
        const ids =
          editor
            .getModel()
            ?.getAllDecorations()
            .map((d) => d.id) ?? [];
        editor.removeDecorations(ids);
        //validate(monacoInstance, editor.getModel()!);
      });

      const clearDecoration = (range: monaco.Range) => {
        //多分あまりよくないやり方
        //DecorationsColectionを一つ作ってすべての行のステータス表示用decorationを突っ込むのが正しい?
        const decorations = editor.getDecorationsInRange(range);
        if (decorations !== null) {
          editor.removeDecorations(decorations.map((d) => d.id));
        }
      };

      type ExecuteLineResult =
        | {
            success: true;
            status: opslang.ControlStatus;
            requestedDelay: number;
            executionContext: opslang.StatementExecutionContext | undefined;
          }
        | { success: false; error: unknown };

      const executeLineParsed = async (
        parsed: opslang.ParsedCode,
        context: opslang.StatementExecutionContext | undefined,
        lineNum: number,
        isFirstLine: boolean,
      ): Promise<ExecuteLineResult> => {
        try {
          const vs = parsed.freeVariables(lineNum);
          const vars = vs.variables;
          const tlmVars = vs.telemetry_variables;
          vs.free();
          await driver.prepareVariables(vars, tlmVars);
          const result = await parsed.executeLine(
            driver,
            context,
            !isFirstLine,
            lineNum,
            Date.now(),
          );
          const status = result.status;
          const executionContext = result.execution_context;
          const requestedDelay = result.requestedDelay;
          result.free();
          return { success: true, status, requestedDelay, executionContext };
        } catch (error) {
          return { success: false, error };
        }
      };

      const processLine = async (
        initialLine: number,
        parsed: opslang.ParsedCode,
        executionContext: opslang.StatementExecutionContext | undefined,
      ): Promise<[boolean, opslang.StatementExecutionContext | undefined]> => {
        const model = editor.getModel();
        if (model === null) {
          return [false, executionContext];
        }
        localStorage.setItem("c2a-devtools-ops-v2", editor.getValue());

        const position = editor.getPosition();
        if (position === null) {
          return [false, executionContext];
        }
        const lineno = position.lineNumber;

        const range = new monaco.Range(lineno, 1, lineno, 1);

        clearDecoration(range);
        const decoration = editor.createDecorationsCollection([
          {
            range,
            options: {
              linesDecorationsClassName: "ml-1 border-l-4 border-slate-600",
              stickiness:
                monaco.editor.TrackedRangeStickiness
                  .NeverGrowsWhenTypingAtEdges,
            },
          },
        ]);

        // executionContext will be "moved" here
        // executionContext cannot be used after this call
        const result = await executeLineParsed(
          parsed,
          executionContext,
          lineno,
          lineno == initialLine,
        );
        if (!result.success) {
          decoration.clear();
          editor.createDecorationsCollection([
            {
              range,
              options: {
                linesDecorationsClassName: "ml-1 border-l-4 border-red-600",
                stickiness:
                  monaco.editor.TrackedRangeStickiness
                    .NeverGrowsWhenTypingAtEdges,
              },
            },
          ]);
          monacoInstance.editor.setModelMarkers(model, "owner", [
            {
              message: `${result.error}`,
              severity: monaco.MarkerSeverity.Error,
              startLineNumber: lineno,
              startColumn: 1,
              endLineNumber: lineno,
              endColumn: model.getLineLength(lineno) + 1,
            },
          ]);
          return [false, undefined];
        }

        const delayLength = Math.max(result.requestedDelay, 0);
        const delay = new Promise((resolve) =>
          setTimeout(resolve, delayLength),
        );
        await delay;

        if (result.status === opslang.ControlStatus.Breaked) {
          return [false, result.executionContext];
        }
        if (result.status === opslang.ControlStatus.Executed) {
          decoration.clear();
          editor.createDecorationsCollection([
            {
              range,
              options: {
                linesDecorationsClassName: "ml-1 border-l-4 border-sky-600",
                stickiness:
                  monaco.editor.TrackedRangeStickiness
                    .NeverGrowsWhenTypingAtEdges,
              },
            },
          ]);
          if (lineno >= model.getLineCount()) {
            return [false, result.executionContext];
          }
          const nextPosition = new monaco.Position(lineno + 1, 1);
          editor.setPosition(nextPosition);
          editor.revealLine(lineno + 1);
        }
        return [true, result.executionContext];
      };

      const tryParse = (): opslang.ParsedCode | undefined => {
        try {
          const parsed = opslang.ParsedCode.fromCode(editor.getValue());
          return parsed;
        } catch (e) {
          const lines = model.getLineCount();
          monacoInstance.editor.setModelMarkers(model, "owner", [
            {
              message: `${e}`,
              severity: monaco.MarkerSeverity.Error,
              startLineNumber: 1,
              startColumn: 1,
              endLineNumber: lines,
              endColumn: model.getLineLength(lines) + 1,
            },
          ]);

          return undefined;
        }
      };

      editor.addCommand(
        monaco.KeyMod.Shift | monaco.KeyCode.Enter,
        async () => {
          const position = editor.getPosition();
          if (position === null) {
            return;
          }
          const parsed = tryParse();
          if (!parsed) {
            return;
          }

          const initialLine = position.lineNumber;
          let executionContext: opslang.StatementExecutionContext | undefined =
            undefined;

          // eslint-disable-next-line no-constant-condition
          while (true) {
            const [cont, nextStatementExecutionContext] = await processLine(
              initialLine,
              parsed,
              executionContext,
            );
            executionContext = nextStatementExecutionContext;
            if (!cont) {
              executionContext?.free();
              break;
            }
          }
          parsed.free();
        },
      );
      const model = editor.getModel()!;
      model.onDidChangeContent(() => {
        validate(monacoInstance, editor.getModel()!);
      });
    },
    [validate, driver],
  );

  return (
    <Editor
      height="100%"
      options={{ fontSize: 16, renderValidationDecorations: "on" }}
      theme="vs-dark"
      onMount={handleEditorDidMount}
    />
  );
};
