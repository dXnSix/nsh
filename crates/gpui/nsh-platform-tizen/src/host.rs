//! The C# host (`host/`): the project MSBuild loads (`Nsh.Tizen.Host.csproj`
//! on `Tizen.NET.Sdk`) and its program, one P/Invoke into `libnsh.so`'s
//! `nsh_tizen_main`. Its build needs a .NET SDK with Tizen's workload; these
//! tests check, without one, what a build would first refuse: a project that
//! is not well-formed XML (MSB4025), or one that no longer names the
//! assembly the runtime files ship.

#[cfg(test)]
mod tests {
    const PROJECT: &str = include_str!("../host/Nsh.Tizen.Host.csproj");
    const PROGRAM: &str = include_str!("../host/Program.cs");

    /// The text of the project's first element `name`.
    fn property<'a>(project: &'a roxmltree::Document<'_>, name: &str) -> Option<&'a str> {
        project
            .descendants()
            .find(|node| node.has_tag_name(name))
            .and_then(|node| node.text())
    }

    #[test]
    fn the_project_is_well_formed_xml_on_tizens_sdk() {
        let project = roxmltree::Document::parse(PROJECT).expect("Nsh.Tizen.Host.csproj parses");
        let root = project.root_element();
        assert!(root.has_tag_name("Project"));
        let sdk = root.attribute("Sdk").unwrap_or_default();
        assert!(sdk.starts_with("Tizen.NET.Sdk/"), "{sdk}");
        assert_eq!(property(&project, "AssemblyName"), Some("Nsh.Tizen.Host"));
        assert_eq!(property(&project, "OutputType"), Some("Exe"));
        assert_eq!(property(&project, "TargetFramework"), Some("tizen80"));
        // The SDK's own TPK step stays off: `nsh package` makes the TPK.
        assert_eq!(property(&project, "TizenCreateTpkOnBuild"), Some("false"));
    }

    /// What MSBuild refused before (a command line in the top comment).
    #[test]
    fn a_double_hyphen_in_a_comment_is_refused() {
        let comment = "<Project>\n  <!-- `nsh package --target tizen` -->\n</Project>";
        assert!(roxmltree::Document::parse(comment).is_err());
    }

    #[test]
    fn the_program_calls_nsh_tizen_main_once() {
        assert_eq!(PROGRAM.matches("[DllImport(").count(), 1);
        assert!(PROGRAM.contains("[DllImport(\"libnsh.so\""));
        assert!(PROGRAM.contains("static extern unsafe int nsh_tizen_main("));
    }
}
