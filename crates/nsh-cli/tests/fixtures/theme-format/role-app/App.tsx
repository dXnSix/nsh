// An app whose Rust crate registers a required role, `banner`, which
// `nsh theme check --app` checks themes against.
import { mount, role, Text } from "nsh";

mount(() => <Text>{role("banner")}</Text>);
