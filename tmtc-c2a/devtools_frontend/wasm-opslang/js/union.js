const asKind = (kind) => {
  return (obj) => {
    if (obj.kind == kind) {
      return obj.value;
    } else {
      return undefined;
    }
  };
};

export const asInt = asKind("integer");
export const asDouble = asKind("double");
export const asBool = asKind("bool");
export const asArray = asKind("array");
export const asString = asKind("string");

const make = (kind) => {
  return (value) => {
    return {
      kind,
      value,
    };
  };
};

export const makeInt = make("integer");
export const makeDouble = make("double");
export const makeBool = make("bool");
export const makeArray = make("array");
export const makeString = make("string");
