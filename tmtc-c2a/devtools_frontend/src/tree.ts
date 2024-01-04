export type TreeNamespace<T> = Map<string, TreeNode<T>>;
export type TreeNode<T> =
  | { type: "leaf"; value: T }
  | { type: "ns"; ns: TreeNamespace<T> };

const makeLeaf = <T>(value: T): TreeNode<T> => ({ type: "leaf", value });
const makeNs = <T>(ns: TreeNamespace<T>): TreeNode<T> => ({ type: "ns", ns });

const mapMapWithKey = <K, T, U>(
  map: Map<K, T>,
  f: (key: K, value: T) => U,
): Map<K, U> => {
  const result = new Map();
  for (const [key, value] of map) {
    result.set(key, f(key, value));
  }
  return result;
};

const mapTreeRec = <T, U>(
  tree: TreeNode<T>,
  path: string[],
  f: (path: string[], node: T) => U,
): TreeNode<U> => {
  switch (tree.type) {
    case "leaf":
      return makeLeaf(f(path, tree.value));
    case "ns":
      return makeNs(
        mapMapWithKey(tree.ns, (key, child) =>
          mapTreeRec(child, [...path, key], f),
        ),
      );
  }
};

export const mapTree = <T, U>(
  tree: TreeNode<T>,
  f: (path: string[], node: T) => U,
): TreeNode<U> => {
  return mapTreeRec(tree, [], f);
};

export const mapNamespace = <T, U>(
  ns: TreeNamespace<T>,
  f: (path: string[], node: T) => U,
): TreeNamespace<U> => {
  const tree = makeNs(ns);
  const mappedTree = mapTreeRec(tree, [], f);
  if (mappedTree.type === "ns") {
    return mappedTree.ns;
  } else {
    throw new Error("Impossible");
  }
};

export const singleton = <T>(path: string[], value: T): TreeNode<T> => {
  let tree: TreeNode<T> = { type: "leaf", value };
  for (const key of path.toReversed()) {
    const ns = new Map();
    ns.set(key, tree);
    tree = { type: "ns", ns };
  }
  return tree;
};

export const add = <T>(
  tree: TreeNode<T>,
  path: string[],
  value: T,
): TreeNode<T> => {
  switch (tree.type) {
    case "leaf":
      if (path.length === 0) {
        tree.value = value;
        return tree;
      } else {
        const [pathHead, ...pathTail] = path;
        const ns = new Map();
        ns.set("", tree);
        ns.set(pathHead, singleton(pathTail, value));
        return {
          type: "ns",
          ns,
        };
      }
    case "ns":
      addToNamespace(tree.ns, path, value);
      return tree;
  }
};

export const addToNamespace = <T>(
  ns: TreeNamespace<T>,
  path: string[],
  value: T,
): void => {
  if (path.length === 0) {
    ns.set("", { type: "leaf", value });
  } else {
    const [pathHead, ...pathTail] = path;
    const child = ns.get(pathHead);
    if (child === undefined) {
      ns.set(pathHead, singleton(pathTail, value));
    } else {
      ns.set(pathHead, add(child, pathTail, value));
    }
  }
};
