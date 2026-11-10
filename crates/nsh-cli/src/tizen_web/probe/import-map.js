import { mapped } from "nsh-probe-mapped";

nshProbe.done("import-map", mapped === "mapped", "nsh-probe-mapped: " + mapped);
