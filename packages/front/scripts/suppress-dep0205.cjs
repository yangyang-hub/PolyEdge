const emitWarning = process.emitWarning;

process.emitWarning = function suppressTailwindRegisterWarning(warning, type, code, ...rest) {
  const warningCode =
    code ??
    (typeof warning === "object" && warning !== null && "code" in warning ? warning.code : undefined);

  if (warningCode === "DEP0205") {
    return;
  }

  return emitWarning.call(this, warning, type, code, ...rest);
};
