// The files a UI set ships, imported by their URLs: what importing such a
// file of the app gives.
import { Text, View } from "nsh";
import logo from "asset:/@compat-rust-native/images/logo.png";
import mark from "asset:/@compat-rust-native/icons/mark.svg";
import pacifico from "asset:/@compat-rust-native/fonts/Pacifico-Regular.woff2";
import cities from "asset:/@compat-rust-native/data/cities.json";
import logoUrl from "asset:/@compat-rust-native/images/logo.png?url";

export default () => (
  <View style={{ flex: 1, paddingTop: 64 }}>
    <Text>{`Image: ${logo.url} ${logo.size.width}x${logo.size.height} at ${(logo.scales ?? []).join(",")}`}</Text>
    <Text>{`SVG: ${mark.url} ${mark.size.width}x${mark.size.height}`}</Text>
    <Text>{`Font: ${pacifico}`}</Text>
    <Text>{`File: ${cities}`}</Text>
    <Text>{`URL: ${logoUrl}`}</Text>
  </View>
);
