//! Source namespaces resolve against current workspace and Cargo-owned roots.
use super::super::{
    EmbeddedModuleInventory, OwnerBinding, ProviderIdentity, ProviderSource, inventory::invalid,
    source,
};
use std::{io, path::Path};

pub(super) fn owners(
    inventory: &EmbeddedModuleInventory,
    bindings: &[OwnerBinding],
    workspace: &Path,
    cargo_home: &Path,
) -> io::Result<()> {
    for binding in bindings {
        if super::canonical_directory(&binding.root)? != binding.root {
            return Err(invalid("owner root is not canonical"));
        }
        if binding.owner_id == "cargo-home" {
            if binding.root != cargo_home {
                return Err(invalid(
                    "Cargo home namespace differs from current Cargo home",
                ));
            }
            continue;
        }
        if binding.owner_id == "workspace" {
            if binding.root != workspace {
                return Err(invalid(
                    "workspace namespace root differs from current workspace",
                ));
            }
            continue;
        }
        if let Some(provider_id) = binding.owner_id.strip_prefix("repository:") {
            repository(inventory, binding, provider_id, cargo_home)?;
            continue;
        }
        if let Some((generated, provider_id)) = binding
            .owner_id
            .strip_prefix("generated:")
            .map(|id| (true, id))
            .or_else(|| {
                binding
                    .owner_id
                    .strip_prefix("build:")
                    .map(|id| (false, id))
            })
        {
            generated_root(inventory, binding, provider_id, generated, workspace)?;
            continue;
        }
        package_root(inventory, binding, workspace, cargo_home)?;
    }
    Ok(())
}
fn document(path: &Path) -> io::Result<toml::Value> {
    let bytes = super::FileIdentity::read(path, super::super::MANIFEST_LIMIT)?
        .verified_bytes(path, super::super::MANIFEST_LIMIT)?;
    toml::from_str(std::str::from_utf8(&bytes).map_err(|error| invalid(error.to_string()))?)
        .map_err(|error| invalid(format!("Cargo manifest: {error}")))
}
fn workspace_owner(provider: &ProviderIdentity, root: &Path, workspace: &Path) -> io::Result<()> {
    let document = document(&source(workspace, "Cargo.toml")?)?;
    let members = document
        .get("workspace")
        .and_then(|v| v.get("members"))
        .and_then(toml::Value::as_array)
        .ok_or_else(|| invalid("workspace manifest omits explicit members"))?;
    let admitted = members
        .iter()
        .filter_map(toml::Value::as_str)
        .any(|member| {
            workspace
                .join(member)
                .canonicalize()
                .is_ok_and(|member_root| member_root == root)
        });
    if !admitted {
        return Err(invalid(
            "provider root is not an explicit current workspace member",
        ));
    }
    package(provider, root)
}
fn package(provider: &ProviderIdentity, root: &Path) -> io::Result<()> {
    let manifest = document(&source(root, "Cargo.toml")?)?;
    let package = manifest
        .get("package")
        .ok_or_else(|| invalid("provider manifest omits package"))?;
    if package.get("name").and_then(toml::Value::as_str) != Some(provider.package.as_str()) {
        return Err(invalid("provider source package name differs from lock"));
    }
    let version = match package.get("version") {
        Some(toml::Value::String(value)) => value.clone(),
        Some(toml::Value::Table(table))
            if table.get("workspace").and_then(toml::Value::as_bool) == Some(true) =>
        {
            let mut parent = root.parent();
            let mut found = None;
            while let Some(directory) = parent {
                let path = directory.join("Cargo.toml");
                if path.is_file() {
                    let owner = document(&path)?;
                    if let Some(value) = owner
                        .get("workspace")
                        .and_then(|v| v.get("package"))
                        .and_then(|v| v.get("version"))
                        .and_then(toml::Value::as_str)
                    {
                        found = Some(value.to_owned());
                        break;
                    }
                }
                parent = directory.parent();
            }
            found.ok_or_else(|| invalid("provider inherited version has no workspace owner"))?
        }
        _ => return Err(invalid("provider package version is absent or invalid")),
    };
    if version != provider.version {
        return Err(invalid("provider source version differs from lock"));
    }
    Ok(())
}
fn git_revision(checkout: &Path) -> io::Result<String> {
    let head = bounded_text(&source(checkout, ".git/HEAD")?)?;
    if let Some(reference) = head.trim().strip_prefix("ref: ") {
        super::super::relative(reference)?;
        let git = checkout.join(".git");
        match source(&git, reference) {
            Ok(path) => Ok(bounded_text(&path)?.trim().to_owned()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let packed = bounded_text(&source(&git, "packed-refs")?)?;
                packed
                    .lines()
                    .filter(|line| !line.starts_with(['#', '^']))
                    .find_map(|line| {
                        line.split_once(' ')
                            .filter(|(_, name)| *name == reference)
                            .map(|(revision, _)| revision.to_owned())
                    })
                    .ok_or_else(|| invalid("Git HEAD reference is absent"))
            }
            Err(error) => Err(error),
        }
    } else {
        Ok(head.trim().to_owned())
    }
}

fn repository(
    inventory: &EmbeddedModuleInventory,
    binding: &OwnerBinding,
    provider_id: &str,
    cargo_home: &Path,
) -> io::Result<()> {
    let provider = inventory
        .spec()
        .providers
        .iter()
        .find(|p| p.package_id == provider_id)
        .ok_or_else(|| invalid("repository namespace has no locked provider"))?;
    let super::super::ProviderSource::Git { revision, .. } = &provider.source else {
        return Err(invalid(
            "repository namespace requires a locked Git provider",
        ));
    };
    let relative = binding
        .root
        .strip_prefix(cargo_home.join("git/checkouts"))
        .map_err(|_| invalid("repository namespace escapes Cargo home"))?;
    if relative.components().count() != 2 || git_revision(&binding.root)? != *revision {
        return Err(invalid("repository namespace differs from locked checkout"));
    }
    Ok(())
}
fn generated_root(
    inventory: &EmbeddedModuleInventory,
    binding: &OwnerBinding,
    provider_id: &str,
    generated: bool,
    workspace: &Path,
) -> io::Result<()> {
    let (provider_id, expected_instance) = provider_id
        .rsplit_once("::")
        .ok_or_else(|| invalid("generated owner omits canonical build instance"))?;
    let provider = inventory
        .spec()
        .providers
        .iter()
        .find(|p| p.package_id == provider_id)
        .ok_or_else(|| invalid("generated output has no locked provider"))?;
    let target = super::CargoLocations::current(workspace)?
        .target_directory()
        .canonicalize()?;
    let relative = binding
        .root
        .strip_prefix(&target)
        .map_err(|_| invalid("generated output escapes current Cargo target directory"))?;
    let components = relative
        .iter()
        .map(|part| {
            part.to_str()
                .ok_or_else(|| invalid("generated output path is not UTF-8"))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let instance = match (generated, components.as_slice()) {
        (true, [.., "build", instance, "out"]) | (false, [.., "build", instance]) => *instance,
        _ => {
            return Err(invalid(
                "generated owner is not a Cargo build-script output instance",
            ));
        }
    };
    if instance != expected_instance {
        return Err(invalid(
            "generated owner build instance differs from local binding",
        ));
    }
    if !instance
        .strip_prefix(&format!("{}-", provider.package))
        .is_some_and(|suffix| {
            !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
    {
        return Err(invalid(
            "generated output build instance differs from locked provider",
        ));
    }
    Ok(())
}
fn package_root(
    inventory: &EmbeddedModuleInventory,
    binding: &OwnerBinding,
    workspace: &Path,
    cargo_home: &Path,
) -> io::Result<()> {
    let provider = inventory
        .spec()
        .providers
        .iter()
        .find(|p| p.package_id == binding.owner_id)
        .ok_or_else(|| invalid("binding owner has no locked provider"))?;
    match &provider.source {
        ProviderSource::Workspace => workspace_owner(provider, &binding.root, workspace)?,
        ProviderSource::Registry { .. } => {
            let relative = binding
                .root
                .strip_prefix(cargo_home.join("registry/src"))
                .map_err(|_| invalid("registry owner escapes current Cargo home"))?;
            let components = relative.iter().collect::<Vec<_>>();
            if components.len() != 2
                || components[1]
                    != std::ffi::OsStr::new(&format!("{}-{}", provider.package, provider.version))
            {
                return Err(invalid(
                    "registry source directory differs from locked package",
                ));
            }
            package(provider, &binding.root)?;
        }
        ProviderSource::Git { revision, .. } => {
            let relative = binding
                .root
                .strip_prefix(cargo_home.join("git/checkouts"))
                .map_err(|_| invalid("Git owner escapes current Cargo home"))?;
            let components = relative.iter().collect::<Vec<_>>();
            if components.len() < 2 {
                return Err(invalid("Git source lacks checkout identity"));
            }
            let checkout = cargo_home
                .join("git/checkouts")
                .join(components[0])
                .join(components[1]);
            if git_revision(&checkout)? != *revision {
                return Err(invalid(
                    "Git checkout revision differs from locked provider",
                ));
            }
            package(provider, &binding.root)?;
        }
    }
    Ok(())
}

fn bounded_text(path: &Path) -> io::Result<String> {
    let identity = super::FileIdentity::read(path, super::super::MANIFEST_LIMIT)?;
    String::from_utf8(identity.verified_bytes(path, super::super::MANIFEST_LIMIT)?)
        .map_err(|error| invalid(format!("source identity UTF-8: {error}")))
}
