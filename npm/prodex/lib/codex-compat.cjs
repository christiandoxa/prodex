"use strict";

const version = "0.145.0";

module.exports = Object.freeze({
  version,
  packageSpecifier: `@openai/codex@${version}`,
  platformSpecifier(distTag) {
    return `npm:@openai/codex@${version}-${distTag}`;
  },
});
