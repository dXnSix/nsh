// The app whose active document `nsh theme check --patch` checks patches
// against: its own theme (`nsh.json`'s `"theme"`).
import { mount, Text, token } from "nsh";

mount(() => <Text>{token("brand")}</Text>);
