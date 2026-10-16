use super::*;
use crate::{build::module::Context, test_support::traced};
use metis_core::embedded_assets::{ProviderIdentity, ProviderSource, ToolKind};
use serde_json::Value;

#[test]
fn actual_workspace_metadata_keeps_worktree_root_in_the_workspace_namespace() {
    traced(|| {
        let (context, cargo, workspace) = {
            let _phase = tracing::info_span!("arrange").entered();
            let workspace = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .find(|directory| directory.join("Cargo.lock").is_file())
                .expect("real owning workspace")
                .to_path_buf();
            let context =
                Context::new(&workspace.join("Cargo.toml")).expect("real neutral Cargo context");
            let cargo = metis_core::embedded_assets::resolve_tool(
                ToolKind::Cargo,
                Path::new("cargo"),
                &workspace,
            )
            .expect("real selected Cargo");
            (context, cargo, workspace)
        };
        let (input, expected_owners, expected_files) = {
            let _phase = tracing::info_span!("act").entered();
            let mut arguments = [
                "metadata",
                "--format-version=1",
                "--no-deps",
                "--locked",
                "--offline",
                "--manifest-path",
            ]
            .map(std::ffi::OsString::from)
            .to_vec();
            arguments.push(workspace.join("Cargo.toml").into_os_string());
            let bytes = metis_core::embedded_assets::capture_command(
                &cargo,
                &arguments,
                std::time::Duration::from_secs(30),
                &context.environment,
                Some(&context.directory),
            )
            .expect("actual locked workspace metadata");
            let metadata: Value = serde_json::from_slice(&bytes).expect("actual metadata JSON");
            let (graph, expected_files) = graph_from_actual_metadata(&metadata);
            let mut input = InputSet {
                files: BTreeSet::new(),
                declared_directories: BTreeSet::new(),
                owners: graph.owners.clone(),
                environment: BTreeSet::new(),
                features: BTreeSet::new(),
            };
            package_manifests(
                &graph,
                &crate::build::module::cargo_home(&graph.workspace).expect("real Cargo home"),
                &mut input,
            )
            .expect("actual manifest ownership traversal");
            (input, graph.owners, expected_files)
        };
        {
            let _phase = tracing::info_span!("assert").entered();
            assert_eq!(input.owners, expected_owners);
            assert_eq!(input.files, expected_files);
            assert!(
                input
                    .owners
                    .iter()
                    .all(|owner| !owner.owner_id.starts_with("repository:workspace:"))
            );
        }
    });
}

fn graph_from_actual_metadata(metadata: &Value) -> (Graph, BTreeSet<PathBuf>) {
    let workspace = PathBuf::from(
        metadata["workspace_root"]
            .as_str()
            .expect("Cargo workspace root"),
    )
    .canonicalize()
    .expect("canonical workspace");
    let mut graph = Graph {
        workspace: workspace.clone(),
        providers: Vec::new(),
        owners: vec![OwnerBinding {
            owner_id: "workspace".into(),
            root: workspace,
        }],
        packages: Vec::new(),
    };
    let mut expected_files = BTreeSet::new();
    for package in metadata["packages"]
        .as_array()
        .expect("real workspace packages")
    {
        let name = package["name"].as_str().expect("actual package name");
        let version = package["version"].as_str().expect("actual package version");
        let provider = ProviderIdentity::new(
            name.to_owned(),
            version.to_owned(),
            ProviderSource::Workspace,
        );
        let manifest = PathBuf::from(package["manifest_path"].as_str().expect("actual manifest"))
            .canonicalize()
            .expect("canonical actual manifest");
        let root = manifest.parent().expect("manifest owner").to_path_buf();
        expected_files.insert(manifest);
        graph.packages.push((
            package["id"].as_str().expect("actual Cargo ID").to_owned(),
            provider.package_id.clone(),
            root.clone(),
        ));
        graph.owners.push(OwnerBinding {
            owner_id: provider.package_id.clone(),
            root,
        });
        graph.providers.push(provider);
    }
    (graph, expected_files)
}
