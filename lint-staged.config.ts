import type { Configuration } from "lint-staged";

const config: Configuration = {
  "src/**/*.rs": () => "cargo fmt",
};

export default config;
